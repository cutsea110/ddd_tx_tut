use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReporterError {
    #[error("reporter unavailable: {0}")]
    Unavailable(String),
}

pub trait Reporter {
    fn send_report(&self, to: &str, message: &str) -> Result<(), ReporterError>;
}
