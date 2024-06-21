use serde::{Deserialize, Serialize};

// NOTE: std::panic::Location's fields are not public, so we have to define our own Location struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location<'a> {
    pub file: &'a str,
    pub line: u32,
    pub column: u32,
}
