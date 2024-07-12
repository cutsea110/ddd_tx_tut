use serde::{Deserialize, Serialize};
use std::rc::Rc;
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

pub trait Observer {
    fn notify(
        &self,
        level: Level,
        to: &str,
        message: &str,
        loc: Location,
    ) -> Result<(), ReporterError>;
}

pub trait Reporter<'a> {
    fn register(&mut self, observer: impl Observer + 'a) -> Result<(), ReporterError>;
    fn get_observers(&self) -> Vec<&dyn Observer>;
    fn send_report(
        &self,
        level: Level,
        to: &str,
        message: &str,
        loc: Location,
    ) -> Result<(), ReporterError> {
        for observer in self.get_observers() {
            observer
                .notify(level.clone(), to, message, loc.clone())
                .or_else(|e| {
                    eprintln!("reporter error: {}", e);
                    Ok(())
                })?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct DefaultReporter<'a> {
    observers: Vec<Rc<dyn Observer + 'a>>,
}
impl<'a> DefaultReporter<'a> {
    pub fn new() -> Self {
        Self {
            observers: Vec::new(),
        }
    }
}
impl<'a> Reporter<'a> for DefaultReporter<'a> {
    fn register(&mut self, observer: impl Observer + 'a) -> Result<(), ReporterError> {
        self.observers.push(Rc::new(observer));
        Ok(())
    }
    fn get_observers(&self) -> Vec<&dyn Observer> {
        self.observers.iter().map(|o| o.as_ref()).collect()
    }
}
