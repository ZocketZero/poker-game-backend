use regex::Regex;
use std::sync::LazyLock;

pub static USERNAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9]+$").expect("Invalid username regex"));
