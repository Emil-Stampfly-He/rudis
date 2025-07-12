pub struct HGetAll {
    valid: bool,
    key: String,
}

impl HGetAll {
    pub fn from_key(key: impl ToString) -> Self {
        HGetAll {
            key: key.to_string(),
            valid: true,
        }
    }

    pub fn new_invalid() -> Self {
        HGetAll {
            key: String::from(""),
            valid: false,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}