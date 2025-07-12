pub struct HGet {
    key: String,
    field: String,
    valid: bool,
}

impl HGet {
    pub fn from_key_field(key: impl ToString, field: impl ToString) -> Self {
        HGet {
            key: key.to_string(),
            field: field.to_string(),
            valid: true,
        }
    }
    
    pub fn new_invalid() -> Self {
        HGet {
            key: String::from(""),
            field: String::from(""),
            valid: false,
        }
    }
    
    pub fn is_valid(&self) -> bool {
        self.valid
    }
    
    pub fn key(&self) -> &str {
        &self.key
    }
    
    pub fn field(&self) -> &str {
        &self.field
    }
}