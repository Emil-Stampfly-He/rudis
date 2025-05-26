use rudis_http::command::Command;

fn generate_buff(arg_string: &str) -> Vec<u8> {
    format!(
        "GET {} HTTP/1.1\r\nHost: localhost:6379\r\nAccept-Encoding: gzip, deflate, br\r\nConnection: keep-alive\r\n\r\n",
        arg_string
    )
    .as_bytes()
    .to_vec()
}

#[test]
fn from_bytes_invalid() {
    match Command::from_bytes(&generate_buff("/foo/bar")) {
        Command::Invalid => assert!(true),
        _ => assert!(false),
    }
}

#[test]
fn from_bytes_get() {
    match Command::from_bytes(&generate_buff("/get/foo")) {
        Command::Get(cmd) => assert_eq!(cmd.key(), "foo"),
        _ => assert!(false),
    }
}

#[test]
fn from_bytes_set() {
    match Command::from_bytes(&generate_buff("/set/foo/bar")) {
        Command::Set(cmd) => {
            assert_eq!(cmd.key(), "foo");
            assert_eq!(cmd.val(), "bar");
            assert_eq!(cmd.ttl_ms(), u64::MAX);
        }
        _ => assert!(false),
    }
}

#[test]
fn from_bytes_set_with_ttl() {
    match Command::from_bytes(&generate_buff("/set/foo/bar/2000")) {
        Command::Set(cmd) => {
            assert_eq!(cmd.key(), "foo");
            assert_eq!(cmd.val(), "bar");
            assert_eq!(cmd.ttl_ms(), 2000);
        }
        _ => assert!(false),
    }
}

#[test]
fn from_bytes_set_invalid_less_than_three_args() {
    match Command::from_bytes(&generate_buff("/set/foo")) {
        Command::Set(cmd) => assert!(!cmd.is_valid()),
        _ => assert!(false),
    }
}

#[test]
fn from_bytes_set_invalid_more_than_four_args() {
    match Command::from_bytes(&generate_buff("/set/foo/bar/2000/1000")) {
        Command::Set(cmd) => assert!(!cmd.is_valid()),
        _ => assert!(false),
    }
}

#[test]
fn from_bytes_get_invalid() {
    match Command::from_bytes(&generate_buff("/get")) {
        Command::Get(cmd) => assert!(!cmd.is_valid()),
        _ => assert!(false),
    }
}