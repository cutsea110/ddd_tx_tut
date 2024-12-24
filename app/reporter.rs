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
    fn handle_notification(
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
                .handle_notification(level.clone(), to, message, loc.clone())
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::location::Location;
    use std::cell::RefCell;

    #[derive(Debug, Clone)]
    struct SpyObserver {
        messages: Rc<RefCell<Vec<(Level, String, String)>>>,
    }
    impl Observer for SpyObserver {
        fn handle_notification(
            &self,
            level: Level,
            to: &str,
            message: &str,
            _loc: Location,
        ) -> Result<(), ReporterError> {
            self.messages
                .borrow_mut()
                .push((level, to.to_string(), message.to_string()));
            Ok(())
        }
    }

    #[test]
    fn test_reporter_for_single_observer() {
        let observer = SpyObserver {
            messages: Rc::new(RefCell::new(Vec::new())),
        };
        let mut reporter = DefaultReporter::new();
        reporter.register(observer.clone()).unwrap();
        reporter
            .send_report(Level::Info, "to", "message", location!())
            .unwrap();
        assert_eq!(
            observer.messages.borrow().as_slice(),
            &[(Level::Info, "to".to_string(), "message".to_string())]
        );
    }

    #[test]
    fn test_reporter_for_multi_observers() {
        let observer1 = SpyObserver {
            messages: Rc::new(RefCell::new(Vec::new())),
        };
        let observer2 = SpyObserver {
            messages: Rc::new(RefCell::new(Vec::new())),
        };
        let mut reporter = DefaultReporter::new();
        reporter.register(observer1.clone()).unwrap();
        reporter.register(observer2.clone()).unwrap();
        reporter
            .send_report(Level::Info, "to", "message", location!())
            .unwrap();
        assert_eq!(
            observer1.messages.borrow().as_slice(),
            &[(Level::Info, "to".to_string(), "message".to_string())]
        );
        assert_eq!(
            observer2.messages.borrow().as_slice(),
            &[(Level::Info, "to".to_string(), "message".to_string())]
        );
    }
}
