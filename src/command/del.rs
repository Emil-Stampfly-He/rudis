#[derive(Debug)]
pub struct Del {
    key: String,
    valid: bool,
}

impl Del {
    pub fn from_key(key: impl ToString) -> Self {
        Del {
            key: key.to_string(),
            valid: true,
        }
    }
    
    pub fn new_invalid() -> Self {
        Del {
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