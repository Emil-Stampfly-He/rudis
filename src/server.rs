use crate::command::Command;
use crate::connection::Connection;
use bytes::Bytes;
use crossbeam_utils::CachePadded;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::str;
use std::sync::{Arc, LazyLock, Mutex};
use tokio::net::{TcpListener, TcpStream};

static EXPIRE_MAP: LazyLock<Arc<Mutex<HashMap<(String, u64), u64>>>> = LazyLock::new(|| {
    // key: (key, ttl) value: created time
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
                    let idx = hash_key(cmd.key()) % db.len();
                    let mut db = db[idx].lock().unwrap();

                    // delete from EXPIRE_MAP & db if expired
                    let now = current_unix_timestamp();
                    let mut expire_map = EXPIRE_MAP.lock().unwrap();
                    expire_map.retain(|(k, ttl), &mut created_time| {
                        if cmd.key() == k {
                            let should_delete = created_time + ttl < now;
                            if should_delete {
                                db.remove(k);
                            }
                            return !should_delete;
                        }
                        true
                    });

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
                        expire_map.insert((cmd.key().to_string(), cmd.ttl()), current_unix_timestamp());
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
                            expire_map.insert((key.clone(), cmd.ttl()), current_unix_timestamp());
                        }
                    }
                    Bytes::copy_from_slice(b"{\"SET\": \"OK\"}")
                } else {
                    Bytes::copy_from_slice(b"{\"SET\": \"Invalid \"}")
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
