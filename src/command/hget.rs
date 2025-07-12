use crate::command::Get;

pub struct HGet {
    key: String,
    field: String,
    valid: bool,
}

impl HGet {
    // TODO
    // pub fn from_key(key: impl ToString) -> Self {
    //     HGet {
    //         key: key.to_string(),
    //         valid: true,
    //     }
    // }
    //
    // pub fn new_invalid() -> Self {
    //     Get {
    //         key: String::from(""),
    //         valid: false,
    //     }
    // }
    //
    // pub fn is_valid(&self) -> bool {
    //     self.valid
    // }
    //
    // pub fn key(&self) -> &str {
    //     &self.key
    // }
}