mod get;

pub use get::Get;

mod set;
mod del;
mod hset;
mod hget;

use httparse::Request;
use serde_json::{Map, Result, Value};
pub use set::{MultipleSet, Set};
pub use del::Del;
pub use hset::HSet;
pub use hget::HGet;

pub enum Command {
    Set(Set),
    Get(Get),
    MultipleSet(MultipleSet),
    Del(Del),
    GetDel(Get, Del),
    HSet(HSet),
    HGet(HGet),
    Invalid,
}

#[derive(Debug, Eq, PartialEq, Clone)]
struct Args {
    valid: bool,
    command: String,
    key: String,
    field: String,
    val: Option<String>,
    ttl_ms: Option<u64>,
    kv: Option<Map<String, Value>>,
    key_list: Option<Vec<String>>,
}

impl Args {
    pub fn new_invalid(command_type: &str) -> Self {
        Args {
            valid: false,
            command: String::from(command_type),
            key: String::from(""),
            field: String::from(""),
            val: None,
            ttl_ms: None,
            kv: None,
            key_list: None,
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
                Command::Set(Set::from_key_val(arg.key, arg.val.unwrap(), arg.ttl_ms.unwrap()))
            }
            "MULTIPLE_SET" => {
                if !arg.valid {
                    return Command::MultipleSet(MultipleSet::new_invalid());
                }
                let json_kv = arg.kv.unwrap();
                if let Some(arg) = MultipleSet::from_json_kv(json_kv, arg.ttl_ms.unwrap()) {
                    Command::MultipleSet(arg)
                } else {
                    Command::MultipleSet(MultipleSet::new_invalid())
                }
            }
            "DEL" => {
                if !arg.valid {
                    return Command::Del(Del::new_invalid());
                }
                Command::Del(Del::from_key_list(arg.key_list.unwrap()))
            }
            "GETDEL" => {
                if !arg.valid {
                    return Command::GetDel(Get::new_invalid(), Del::new_invalid());
                }
                let get = Get::from_key(arg.key);
                let del = Del::from_key_list(arg.key_list.unwrap());
                Command::GetDel(get, del)
            }
            "HSET" => {
                if !arg.valid {
                    return Command::HSet(HSet::new_invalid());
                }
                let kv = arg.kv.unwrap();
                let key = arg.key;
                if let Some(arg) = HSet::from_json_kv(key, kv, arg.ttl_ms.unwrap()) {
                    Command::HSet(arg)
                } else { 
                    Command::HSet(HSet::new_invalid())
                }
            }
            "HGET" => {
                if !arg.valid {
                    return Command::HGet(HGet::new_invalid());
                }
                Command::HGet(HGet::from_key_field(arg.key, arg.field))
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
                    field: String::from(""),
                    val: None,
                    ttl_ms: None,
                    kv: None,
                    key_list: None,
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
                            field: String::from(""),
                            val: Some(String::from(all_path_vec[2])),
                            ttl_ms: Some(u64::MAX),
                            kv: None,
                            key_list: None,
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
                        let ttl_ms = all_path_vec[3];

                        return Args {
                            valid: true,
                            command: String::from("SET"),
                            key: String::from(key),
                            field: String::from(""),
                            val: Some(String::from(val)),
                            ttl_ms: Some(ttl_ms.parse().unwrap()),
                            kv: None,
                            key_list: None,
                        };
                    }
                }
            }
            // DEL key
            // case 1: curl 'localhost:6379/del/hello'
            // case 2: curl 'localhost:6379/del/hello/foo/...'
            "DEL" => {
                if all_path_vec.len() < 2 {
                    return Args::new_invalid("DEL");
                } else {
                    let key_list: Vec<String>= all_path_vec
                        .iter()
                        .map(|key| { key.to_string() })
                        .skip(1)
                        .collect();
                    return Args {
                        valid: true,
                        command: String::from("DEL"),
                        key: String::from(""),
                        field: String::from(""),
                        val: None,
                        ttl_ms: None,
                        kv: None,
                        key_list: Some(key_list),
                    }
                }
            }
            // GETDEL key
            // curl 'localhost:6379/getdel/hello'
            "GETDEL" => {
                if all_path_vec.len() != 2 {
                    return Args::new_invalid("GETDEL");
                } else {
                    let key_list: Vec<String>= all_path_vec
                        .iter()
                        .map(|key| { key.to_string() })
                        .skip(1)
                        .collect();
                    let key = &key_list[0];
                    return Args {
                        valid: true,
                        command: String::from("GETDEL"),
                        key: key.clone(),
                        field: String::from(""),
                        val: None,
                        ttl_ms: None,
                        kv: None,
                        key_list: Some(key_list),
                    }
                }
            }
            // HSET key field1 value1 [field2 value2 ...] [EX]
            // case 1: curl 'localhost:6379/hset/key/field1/value1'
            // case 2: curl 'localhost:6379/hset/key/field1/value1/field2/value2'
            // case 3: curl 'localhost:6379/hset/key/field1/value1/field2/value2/1000'
            "HSET" => {
                if all_path_vec.len() < 4 {
                    return Args::new_invalid("HSET");
                }

                let rest_path_vec: Vec<String> = all_path_vec.iter()
                    .map(|key| { key.to_string() })
                    .skip(1)
                    .collect();
                // no expiration time specified, case 1 & 2
                if rest_path_vec.len() % 2 != 0 {
                    let key = rest_path_vec[0].clone();
                    let mut field_vec: Vec<String> = vec![];
                    let mut value_vec: Vec<Value> = vec![];

                    for (idx, item) in rest_path_vec[1..].iter().enumerate() {
                        if idx % 2 == 0 {
                            field_vec.push(item.clone());
                        } else {
                            value_vec.push(item.as_str().into());
                        }
                    }
                    assert_eq!(field_vec.len(), value_vec.len());

                    let mut fv_map = Map::new();
                    let len = field_vec.len();
                    for idx in 0..len {
                        fv_map.insert(field_vec[idx].clone(), value_vec[idx].clone());
                    }

                    return Args {
                        valid: true,
                        command: String::from("HSET"),
                        key,
                        field: String::from(""),
                        val: None,
                        ttl_ms: Option::from(u64::MAX),
                        kv: Option::from(fv_map),
                        key_list: None,
                    }
                // expiration time specified, case 3 
                } else {
                    let key = rest_path_vec[0].clone();
                    let mut field_vec: Vec<String> = vec![];
                    let mut value_vec: Vec<Value> = vec![];
                    let ttl_ms: u64;

                    let ttl_str = rest_path_vec.last().unwrap();
                    match ttl_str.parse::<u64>() {
                        Ok(val) => ttl_ms = val,
                        Err(_) => return Args::new_invalid("HSET"),
                    }

                    for (idx, item) in rest_path_vec[1..rest_path_vec.len() - 1].iter().enumerate() {
                        if idx % 2 == 0 {
                            field_vec.push(item.clone());
                        } else {
                            value_vec.push(item.as_str().into());
                        }
                    }

                    assert_eq!(field_vec.len(), value_vec.len());

                    let mut fv_map = Map::new();
                    let len = field_vec.len();
                    for idx in 0..len {
                        fv_map.insert(field_vec[idx].clone(), value_vec[idx].clone());
                    }

                    return Args {
                        valid: true,
                        command: String::from("HSET"),
                        key,
                        field: String::from(""),
                        val: None,
                        ttl_ms: Option::from(ttl_ms),
                        kv: Option::from(fv_map),
                        key_list: None,
                    }
                }
            }
            // HGET key field
            // curl 'localhost:6379/hget/key/field'
            "HGET" => {
                if all_path_vec.len() != 3 {
                    return Args::new_invalid("HGET");
                }
                
                let key = all_path_vec[1].to_string();
                let field = all_path_vec[2].to_string();
                
                return Args {
                    valid: true,
                    command: String::from("HGET"),
                    key,
                    field,
                    val: None,
                    ttl_ms: None,
                    kv: None,
                    key_list: None,
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
        let ttl_ms = if all_path_vec.len() == 2 { all_path_vec[1].parse::<u64>().unwrap() } else { u64::MAX };
        match parse_json(&body) {
            Ok(value) => {
                if let Some(obj) = value.as_object() {
                    return Args {
                        valid: true,
                        command: String::from("MULTIPLE_SET"),
                        key: String::from(""),
                        field: String::from(""),
                        val: None,
                        ttl_ms: Some(ttl_ms),
                        kv: Some(obj.clone()),
                        key_list: None,
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
    match serde_json::from_slice(bytes) {
        Ok(v) => Ok(v),
        Err(e) => {
            eprintln!("JSON parse error: {}", e);
            Err(e)
        }
    }
}
