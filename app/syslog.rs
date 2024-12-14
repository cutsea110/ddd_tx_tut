use log::{error, trace};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use syslog::{Formatter3164, Logger, LoggerBackend};

use crate::{
    location::Location,
    reporter::{self, Level, Observer},
};

pub struct Client {
    writer: RefCell<Logger<LoggerBackend, Formatter3164>>,
}
impl Client {
    pub fn new(program_name: &str, pid: u32) -> Result<Self, reporter::ReporterError> {
        let formatter = Formatter3164 {
            facility: syslog::Facility::LOG_USER,
            hostname: None,
            process: program_name.into(),
            pid,
        };
        trace!("connecting to syslog: {:?}", formatter);
        match syslog::unix(formatter) {
            Err(e) => {
                error!("impossible to connect to syslog: {:?}", e);
                Err(reporter::ReporterError::Unavailable(format!(
                    "impossible to connect to syslog: {:?}",
                    e
                )))
            }
            Ok(writer) => {
                trace!("connected to syslog.");
                Ok(Self {
                    writer: RefCell::new(writer),
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Payload<'a> {
    level: Level,
    message: &'a str,
    location: Location<'a>,
}

impl Observer for Client {
    fn handle_notification(
        &self,
        level: Level,
        _to: &str,
        message: &str,
        loc: crate::location::Location,
    ) -> Result<(), crate::reporter::ReporterError> {
        let payload = serde_json::to_string(&Payload {
            level,
            message,
            location: loc,
        })
        .unwrap_or_default();
        self.writer
            .borrow_mut()
            .notice(payload.as_str())
            .map_err(|e| crate::reporter::ReporterError::Unavailable(e.to_string()))
    }
}
