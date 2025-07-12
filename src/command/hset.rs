use serde_json::{Map, Value};

pub struct HSet {
    key: String,
    ttl_ms: u64,
    valid: bool,
    kv: Map<String, Value>,
}
impl HSet {
    pub fn from_json_kv(key: impl ToString, obj: Map<String, Value>, ttl: u64) -> Option<Self> {
        Some(HSet {
            key: key.to_string(),
            ttl_ms: ttl,
            valid: true,
            kv: obj,
        })
    }

    pub fn new_invalid() -> Self {
        HSet {
            key: String::new(),
            ttl_ms: 0,
            valid: false,
            kv: Map::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn kv(&self) -> &Map<String, Value> {
        &self.kv
    }

    pub fn ttl_ms(&self) -> u64 { self.ttl_ms }
    
    pub fn key(&self) -> &str { &self.key }
}