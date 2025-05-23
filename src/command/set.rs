use serde_json::{Map, Value};

#[derive(Debug)]
pub struct Set {
    key: String,
    val: String,
    ttl: u64, // in milliseconds
    valid: bool,
}

impl Set {
    pub fn from_key_val(key: impl ToString, value: impl ToString, ttl: u64) -> Self {
        Set {
            key: key.to_string(),
            val: value.to_string(),
            ttl,
            valid: true,
        }
    }

    pub fn new_invalid() -> Self {
        Set {
            key: String::from(""),
            val: String::from(""),
            ttl: 0,
            valid: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn val(&self) -> &str {
        &self.val
    }
    
    pub fn ttl(&self) -> u64 { self.ttl }
}

pub struct MultipleSet {
    kv: Map<String, Value>,
    valid: bool,
}

impl MultipleSet {
    pub fn from_json_kv(obj: Map<String, Value>) -> Option<Self> {
        Some(MultipleSet {
            kv: obj,
            valid: true,
        })
    }

    pub fn new_invalid() -> Self {
        MultipleSet {
            kv: Map::new(),
            valid: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn kv(&self) -> &Map<String, Value> {
        &self.kv
    }
}
