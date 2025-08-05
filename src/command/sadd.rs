use std::collections::HashSet;

pub struct SAdd {
    valid: bool,
    key: String,
    val_set: HashSet<String>,

}

impl SAdd {
    pub fn from_key_val_list(key: impl ToString, val_set: HashSet<String>) -> Self {
        SAdd {
            key: key.to_string(),
            val_set,
            valid: true,
        }
    }

    pub fn new_invalid() -> Self {
        SAdd {
            key: String::from(""),
            val_set: HashSet::new(),
            valid: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn val_list(&self) -> &HashSet<String> {
        &self.val_set
    }
}