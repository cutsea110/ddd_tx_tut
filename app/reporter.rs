pub use log::Level;
use thiserror::Error;

pub use crate::location::Location;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReporterError {
    #[error("reporter unavailable: {0}")]
    Unavailable(String),
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
