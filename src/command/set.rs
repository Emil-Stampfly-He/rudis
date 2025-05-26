use serde_json::{Map, Value};

#[derive(Debug)]
pub struct Set {
    key: String,
    val: String,
    ttl_ms: u64,
    valid: bool,
}

impl Set {
    pub fn from_key_val(key: impl ToString, value: impl ToString, ttl_ms: u64) -> Self {
        Set {
            key: key.to_string(),
            val: value.to_string(),
            ttl_ms,
            valid: true,
        }
    }

    pub fn new_invalid() -> Self {
        Set {
            key: String::from(""),
            val: String::from(""),
            ttl_ms: 0,
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

    pub fn ttl_ms(&self) -> u64 { self.ttl_ms }
}

pub struct MultipleSet {
    kv: Map<String, Value>,
    ttl_ms: u64,
    valid: bool,
}

impl MultipleSet {
    pub fn from_json_kv(obj: Map<String, Value>, ttl: u64) -> Option<Self> {
        Some(MultipleSet {
            kv: obj,
            ttl_ms: ttl,
            valid: true,
        })
    }

    pub fn new_invalid() -> Self {
        MultipleSet {
            kv: Map::new(),
            ttl_ms: 0,
            valid: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn kv(&self) -> &Map<String, Value> {
        &self.kv
    }

    pub fn ttl_ms(&self) -> u64 { self.ttl_ms }
}