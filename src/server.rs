use crate::command::{Command, Del, Get, HSet, MultipleSet, Set};
use crate::connection::Connection;
use bytes::Bytes;
use crossbeam_utils::CachePadded;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::str;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use rand::seq::IteratorRandom;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::interval;

static EXPIRE_MAP: LazyLock<Arc<Mutex<HashMap<String, (u64, u64)>>>> = LazyLock::new(|| {
    // key: key value: (created time, ttl_ms)
    Arc::new(Mutex::new(HashMap::new()))
});

pub enum DbValue {
    String(Bytes),
    HashMap(HashMap<String, Bytes>),
}

type ShardedDb = Arc<Vec<CachePadded<Mutex<HashMap<String, DbValue>>>>>;

pub struct Server {
    addr: SocketAddr,
    db: ShardedDb,
}

impl Server {
    pub fn new(addr: SocketAddr, num_shards: usize) -> Self {
        let db = new_sharded_db(num_shards);
        Server { addr, db }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(self.addr).await?;
        let db = self.db.clone();

        // active deletion for keys with TTL
        active_cleanup_task(db).await;

        loop {
            let (socket, _) = listener.accept().await?;
            let db = self.db.clone();
            tokio::spawn(async move {
                process(socket, db).await;
            });
        }
    }
}

/// Active deletion executed every 100 ms for 20 random keys
async fn active_cleanup_task(db: ShardedDb) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_millis(100));
        loop {
            interval.tick().await;

            // amount of a bunch: 20
            let sample_keys= {
                let expire_map = EXPIRE_MAP.lock().unwrap();
                expire_map
                    .iter()
                    .map(|(key, _)| key.clone())
                    .choose_multiple(&mut rand::rng(), 20)
                    .into_iter()
                    .collect::<Vec<String>>()
            };

            let now = current_unix_timestamp();
            for key in sample_keys {
                let mut expire_map = EXPIRE_MAP.lock().unwrap();
                if let Some(&(created_time, ttl_ms)) = expire_map.get(&key) {
                    if ttl_ms != u64::MAX && created_time + ttl_ms < now {
                        expire_map.remove(&key);
                        drop(expire_map); // prevent nested lock

                        let idx = hash_key(&key) % db.len();
                        let mut db = db[idx].lock().unwrap();
                        db.remove(&key);
                    }
                }
            }
        }
    });
}

fn new_sharded_db(num_shards: usize) -> ShardedDb {
    let mut shards = Vec::with_capacity(num_shards);
    for _ in 0..num_shards {
        shards.push(CachePadded::new(Mutex::new(HashMap::new())));
    }
    Arc::new(shards)
}

fn hash_key(key: &str) -> usize {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish() as usize
}

async fn process(socket: TcpStream, db: ShardedDb) {
    let mut connection = match Connection::new(socket).await {
        Ok(connection) => connection,
        Err(_e) => {
            return;
        }
    };

    loop {
        let buff = match connection.read_stream().await {
            Ok(buff) => buff,
            Err(_e) => {
                return;
            }
        };

        let response: Bytes = match Command::from_bytes(&buff) {
            Command::Get(cmd) => handle_get(cmd, &db),
            Command::Set(cmd) => handle_set(cmd, &db),
            Command::MultipleSet(cmd) => handle_multiple_set(cmd, &db),
            Command::Del(cmd) => handle_del(cmd, &db),
            Command::GetDel(get_cmd, del_cmd) => handle_getdel(get_cmd, del_cmd, &db),
            Command::HSet(cmd) => handle_hset(cmd, &db),
            Command::Invalid => Bytes::copy_from_slice(b"{}"),
        };

        if let Err(e) = connection.write_stream(&response).await {
            println!("write stream error: {e}");
            return;
        }
    }
}

fn handle_get(cmd: Get, db: &ShardedDb) -> Bytes {
    if cmd.is_valid() {
        // lazy deletion from EXPIRE_MAP & db if expired
        // fixed lock order: lock EXPIRE_MAP first, then lock sharded db
        let now = current_unix_timestamp();
        let idx = hash_key(cmd.key()) % db.len();

        {
            let mut expire_map = EXPIRE_MAP.lock().unwrap();
            if let Some(&(created_time, ttl_ms)) = expire_map.get(cmd.key()) {
                if ttl_ms != u64::MAX && created_time + ttl_ms < now {
                    expire_map.remove(cmd.key());
                    drop(expire_map); // prevent nested lock

                    let mut db = db[idx].lock().unwrap();
                    db.remove(cmd.key());
                }
            }
        }

        let db = db[idx].lock().unwrap();
        if let Some(DbValue::String(value)) = db.get(cmd.key()) {
            let value_string = std::str::from_utf8(value).unwrap();
            Bytes::from(format!("{{\"{}\":\"{}\"}}", cmd.key(), value_string))
        } else {
            Bytes::copy_from_slice(b"{}")
        }
    } else {
        Bytes::copy_from_slice(b"{\"GET\": \"Invalid \"}")
    }
}

fn handle_set(cmd: Set, db: &ShardedDb) -> Bytes {
    if !cmd.is_valid() {
        return Bytes::copy_from_slice(b"{\"SET\": \"Invalid \"}");
    }

    let idx = hash_key(cmd.key()) % db.len();
    let mut shard = db[idx].lock().unwrap();
    let db_value = DbValue::String(Bytes::copy_from_slice(cmd.val().as_bytes()));

    // insert into expire_map
    shard.insert(cmd.key().to_string(), db_value);

    {
        let mut expire_map = EXPIRE_MAP.lock().unwrap();
        expire_map.insert(cmd.key().to_string(), (current_unix_timestamp(), cmd.ttl_ms()));
    }

    Bytes::copy_from_slice(b"{\"SET\": \"OK\"}")
}

fn handle_multiple_set(cmd: MultipleSet, db: &ShardedDb) -> Bytes {
    if cmd.is_valid() {
        for (key, val) in cmd.kv().iter() {
            let idx: usize = hash_key(key) % db.len();
            let mut db = db[idx].lock().unwrap();
            let db_value = DbValue::String(Bytes::copy_from_slice(val.as_str().unwrap().to_string().as_bytes()));

            db.insert(key.to_string(), db_value);

            {
                let mut expire_map = EXPIRE_MAP.lock().unwrap();
                expire_map.insert(key.clone(), (current_unix_timestamp(), cmd.ttl_ms()));
            }
        }
        Bytes::copy_from_slice(b"{\"SET\": \"OK\"}")
    } else {
        Bytes::copy_from_slice(b"{\"SET\": \"Invalid \"}")
    }
}

fn handle_del(cmd: Del, db: &ShardedDb) -> Bytes {
    if cmd.is_valid() {
        if cmd.key_list().len() >= 1 {
            let mut deleted_count = 0;

            {
                let mut expire_map = EXPIRE_MAP.lock().unwrap();
                expire_map.retain(|key, _| {
                    cmd.key_list().contains(key)
                });
            }

            for key in cmd.key_list() {
                let idx = hash_key(&*key) % db.len();
                let mut db = db[idx].lock().unwrap();
                if db.remove(&*key).is_some() {
                    deleted_count += 1;
                }
            }

            let response = format!("DEL: {} REMOVED", deleted_count);
            Bytes::copy_from_slice(response.as_bytes())
        } else {
            Bytes::copy_from_slice(b"{\"DEL\": \"0 REMOVED\"}")
        }
    } else {
        Bytes::copy_from_slice(b"{\"DEL\": \"Invalid \"}")
    }
}

fn handle_getdel(get_cmd: Get, del_cmd: Del, db: &ShardedDb) -> Bytes {
    if get_cmd.is_valid() && del_cmd.is_valid() {
        let now = current_unix_timestamp();
        let idx = hash_key(get_cmd.key()) % db.len();
        {
            let mut expire_map = EXPIRE_MAP.lock().unwrap();
            if let Some(&(created_time, ttl_ms)) = expire_map.get(get_cmd.key()) {
                if ttl_ms != u64::MAX && created_time + ttl_ms < now {
                    expire_map.remove(get_cmd.key());
                    drop(expire_map); // prevent nested lock

                    let mut db = db[idx].lock().unwrap();
                    db.remove(get_cmd.key());
                }
            }
        }

        // get value from db
        let mut db = db[idx].lock().unwrap();
        let response = if let Some(DbValue::String(value)) = db.get(get_cmd.key()) {
            let value_string = str::from_utf8(value).unwrap();
            format!("{{\"{}\":\"{}\"}} : REMOVED", get_cmd.key(), value_string)
        } else {
            String::from("{}")
        };

        // delete keys in del_cmd
        for key in del_cmd.key_list() {
            {
                let mut expire_map = EXPIRE_MAP.lock().unwrap();
                expire_map.remove(&*key);
            }

            db.remove(&*key);
        }

        Bytes::copy_from_slice(response.as_bytes())
    } else {
        Bytes::copy_from_slice(b"{\"GETDEL\": \"Invalid \"}")
    }
}

fn handle_hset(cmd: HSet, db: &ShardedDb) -> Bytes {
    todo!()
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
