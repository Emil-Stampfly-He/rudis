use crate::command::Command;
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

type ShardedDb = Arc<Vec<CachePadded<Mutex<HashMap<String, Bytes>>>>>;

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
        
        // active deletion executed every 100 ms for 20 random keys
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

        loop {
            let (socket, _) = listener.accept().await?;
            let db = self.db.clone();
            tokio::spawn(async move {
                process(socket, db).await;
            });
        }
    }
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
            Command::Get(cmd) => {
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
                    if let Some(value) = db.get(cmd.key()) {
                        let value_string = std::str::from_utf8(value).unwrap();
                        Bytes::from(format!("{{\"{}\":\"{}\"}}", cmd.key(), value_string))
                    } else {
                        Bytes::copy_from_slice(b"{}")
                    }
                } else {
                    Bytes::copy_from_slice(b"{\"GET\": \"Invalid \"}")
                }
            }
            Command::Set(cmd) => {
                if cmd.is_valid() {
                    let idx: usize = hash_key(cmd.key()) % db.len();
                    let mut db = db[idx].lock().unwrap();

                    db.insert(
                        cmd.key().to_string(),
                        Bytes::copy_from_slice(cmd.val().as_bytes()),
                    );

                    // insert into expire_map
                    {
                        let mut expire_map = EXPIRE_MAP.lock().unwrap();
                        expire_map.insert(cmd.key().to_string(), (current_unix_timestamp(), cmd.ttl_ms()));
                    }

                    Bytes::copy_from_slice(b"{\"SET\": \"OK\"}")
                } else {
                    Bytes::copy_from_slice(b"{\"SET\": \"Invalid \"}")
                }
            }
            Command::MultipleSet(cmd) => {
                if cmd.is_valid() {
                    for (key, val) in cmd.kv().iter() {
                        let idx: usize = hash_key(key) % db.len();
                        let mut db = db[idx].lock().unwrap();

                        db.insert(
                            key.to_string(),
                            Bytes::copy_from_slice(val.as_str().unwrap().to_string().as_bytes()),
                        );
                        
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
            Command::Del(cmd) => {
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
            Command::Invalid => Bytes::copy_from_slice(b"{}"),
        };

        if let Err(e) = connection.write_stream(&response).await {
            println!("write stream error: {e}");
            return;
        }
    }
}

fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
