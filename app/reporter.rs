use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use crate::location::Location;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReporterError {
    #[error("reporter unavailable: {0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    Trace,
    Info,
    Warn,
    Error,
}

pub trait Reporter {
    fn send_report(
        &self,
        level: Level,
        to: &str,
        message: &str,
        loc: Location,
    ) -> Result<(), ReporterError>;
}
