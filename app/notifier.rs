use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NotifierError {
    #[error("notifier unavailable: {0}")]
    Unavailable(String),
}

pub trait Notifier {
    fn notify(&self, to: &str, message: &str) -> Result<(), NotifierError>;
}

pub trait HaveNotifier {
    type T: Notifier;

    fn get_notifier(&self) -> Self::T;
}
