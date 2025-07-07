pub struct Del {
    key: String,
    valid: bool,
    key_list: Vec<String>
}

impl Del {
    pub fn from_key_list(key_list: Vec<String>) -> Self {
        Del {
            key: String::from(""),
            valid: true,
            key_list
        }
    }
    
    pub fn new_invalid() -> Self {
        Del {
            key: String::from(""),
            valid: false,
            key_list: Vec::new()
        }
    }
    
    pub fn is_valid(&self) -> bool {
        self.valid
    }
   
    pub fn key(&self) -> &str {
        &self.key
    }
    
    pub fn key_list(&self) -> Vec<String> {
        self.key_list.clone()
    }
}