mod get;

use std::hash::{Hash, Hasher};
pub use get::Get;

mod set;
use httparse::Request;
use serde_json::{Map, Result, Value};
pub use set::{MultipleSet, Set};

pub enum Command {
    Set(Set),
    Get(Get),
    MultipleSet(MultipleSet),
    Invalid,
}

#[derive(Debug, Eq, PartialEq, Clone)]
struct Args {
    valid: bool,
    command: String,
    key: String,
    val: Option<String>,
    ttl: Option<u64>,
    kv: Option<Map<String, Value>>,
}

impl Args {
    pub fn new_invalid(command_type: &str) -> Self {
        Args {
            valid: false,
            command: String::from(command_type),
            key: String::from(""),
            val: None,
            ttl: None,
            kv: None,
        }
    }
}

impl Hash for Args {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.valid.hash(state);
        self.command.hash(state);
        self.key.hash(state);
        self.val.hash(state);
        self.ttl.hash(state);
        
        if let Some(ref map) = self.kv {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));

            for (k, v) in entries {
                k.hash(state);
                v.to_string().hash(state);
            }
        }
    }
}

impl Command {
    pub fn from_bytes(http_request_buff: &[u8]) -> Command {
        if http_request_buff.is_empty() {
            return Command::Invalid;
        }

        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut req = Request::new(&mut headers);
        let result = req.parse(http_request_buff).unwrap();
        let n = result.unwrap();

        let arg = make_args(&req, http_request_buff, n);

        match arg.command.as_str() {
            "GET" => {
                if !arg.valid {
                    return Command::Get(Get::new_invalid());
                }
                Command::Get(Get::from_key(arg.key))
            }
            "SET" => {
                if !arg.valid {
                    return Command::Set(Set::new_invalid());
                }
                Command::Set(Set::from_key_val(arg.key, arg.val.unwrap(), arg.ttl.unwrap()))
            }
            "MULTIPLE_SET" => {
                if !arg.valid {
                    return Command::MultipleSet(MultipleSet::new_invalid());
                }
                let json_kv = arg.kv.unwrap();
                if let Some(arg) = MultipleSet::from_json_kv(json_kv, arg.ttl.unwrap()) {
                    Command::MultipleSet(arg)
                } else {
                    Command::MultipleSet(MultipleSet::new_invalid())
                }
            }
            _ => Command::Invalid,
        }
    }
}

fn make_args(req: &Request, request_buff: &[u8], idx_of_body: usize) -> Args {
    let method = req.method.unwrap();
    let all_path_vec = split_on_path(req.path.unwrap());

    // using regular GET request (pass data through URL)
    if method == "GET" {
        let action = all_path_vec[0];
        match action.to_uppercase().as_str() {
            "GET" => {
                if all_path_vec.len() < 2 || all_path_vec[1].is_empty() || all_path_vec.len() > 2 {
                    return Args::new_invalid("GET");
                }
                let key = all_path_vec[1];
                return Args {
                    valid: true,
                    command: String::from("GET"),
                    key: String::from(key),
                    val: None,
                    ttl: None,
                    kv: None,
                };
            }
            // SET key value [EX]
            // case 1: curl 'localhost:6379/set/hello/world/1000': EX 1000 ms
            // case 2: curl 'localhost:6379/set/hello/world': EX 2^64-1 ms (never expire)
            "SET" => {
                if all_path_vec.len() < 4 {
                    if all_path_vec.len() < 3
                        || all_path_vec[1].is_empty()
                        || all_path_vec[2].is_empty() {
                        return Args::new_invalid("SET");   
                    } else {
                        return Args {
                            valid: true,
                            command: String::from("SET"),
                            key: String::from(all_path_vec[1]),
                            val: Some(String::from(all_path_vec[2])),
                            ttl: Some(u64::MAX),
                            kv: None,
                        };   
                    }
                } else if all_path_vec.len() > 4 {
                    return Args::new_invalid("SET");   
                } else {
                    if all_path_vec[1].is_empty() 
                        || all_path_vec[2].is_empty() 
                        || all_path_vec[3].is_empty() {
                        return Args::new_invalid("SET");   
                    } else {
                        let key = all_path_vec[1];
                        let val = all_path_vec[2];
                        let ttl = all_path_vec[3];

                        return Args {
                            valid: true,
                            command: String::from("SET"),
                            key: String::from(key),
                            val: Some(String::from(val)),
                            ttl: Some(ttl.parse().unwrap()),
                            kv: None,
                        };
                    }
                }
            }
            _ => return Args::new_invalid("INVALID"),
        }
    } else if method == "POST" {
        // using POST request (SET ONLY) that passes kv-pair through body
        // case 1: curl -X POST 'localhost:6379/set/1000' -d '{"hello":"world"}' EX 1000 ms
        // case 2: curl -X POST 'localhost:6379/set' -d '{"hello":"world"}' EX 2^64-1 ms (never expire)
        if all_path_vec.len() > 2 && all_path_vec.len() < 1 {
            return Args::new_invalid("SET");
        }
        
        let body = request_buff[idx_of_body..].to_vec();
        let ttl = if all_path_vec.len() == 2 { all_path_vec[1].parse::<u64>().unwrap() } else { u64::MAX };
        match parse_json(&body) {
            Ok(value) => {
                if let Some(obj) = value.as_object() {
                    return Args {
                        valid: true,
                        command: String::from("MULTIPLE_SET"),
                        key: String::from(""),
                        val: None,
                        ttl: Some(ttl),
                        kv: Some(obj.clone()),
                    };
                }
                return Args::new_invalid("SET");
            }
            Err(_) => return Args::new_invalid("SET"),
        }
    }
    Args::new_invalid("INVALID")
}

fn split_on_path(input: &str) -> Vec<&str> {
    let all: Vec<&str> = input.split('/').collect();
    if let Some((_, rest)) = all.split_first() {
        return rest.to_vec();
    }
    all
}

fn parse_json(bytes: &[u8]) -> Result<Value> {
    eprintln!("BODY as text: {:?}", String::from_utf8_lossy(bytes));
    match serde_json::from_slice(bytes) {
        Ok(v) => Ok(v),
        Err(e) => {
            eprintln!("JSON parse error: {}", e);
            Err(e)
        }
    }
}
