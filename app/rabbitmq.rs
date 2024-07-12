pub use log::{error, trace};
use serde::{Deserialize, Serialize};
pub use std::rc::Rc;

use crate::reporter::{self, Level, Location, Observer};

#[derive(Debug, Clone)]
pub struct Client {
    async_runtime: Rc<tokio::runtime::Runtime>,
    conn: Rc<lapin::Connection>,
}
impl Client {
    pub fn open(addr: &str) -> Result<Self, reporter::ReporterError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        trace!("connecting to rabbitmq: {}", addr);
        let conn = runtime.block_on(async {
            lapin::Connection::connect(addr, lapin::ConnectionProperties::default())
                .await
                .map_err(|e| {
                    error!("failed to connect to rabbitmq: {}", e);
                    reporter::ReporterError::Unavailable(e.to_string())
                })
        })?;
        trace!("connected to rabbitmq with {:?}", conn.configuration());

        Ok(Self {
            async_runtime: Rc::new(runtime),
            conn: Rc::new(conn),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Payload<'a> {
    level: Level,
    message: &'a str,
    location: Location<'a>,
}

impl Observer for Client {
    // to: queue name
    // message: message to send
    fn notify(
        &self,
        level: Level,
        to: &str,
        message: &str,
        loc: Location,
    ) -> Result<(), reporter::ReporterError> {
        self.async_runtime.block_on(async {
            let chan = self.conn.create_channel().await.map_err(|e| {
                error!("failed to create channel: {}", e);
                reporter::ReporterError::Unavailable(e.to_string())
            })?;
            trace!("channel created");
            chan.queue_declare(
                to,
                lapin::options::QueueDeclareOptions::default(),
                lapin::types::FieldTable::default(),
            )
            .await
            .map_err(|e| {
                error!("failed to declare queue: {}", e);
                reporter::ReporterError::Unavailable(e.to_string())
            })?;
            trace!("queue declared: {}", to);
            let payload = serde_json::to_string(&Payload {
                level,
                message,
                location: loc,
            })
            .unwrap_or_default();
            chan.basic_publish(
                "",
                to,
                lapin::options::BasicPublishOptions::default(),
                payload.as_bytes(),
                lapin::BasicProperties::default(),
            )
            .await
            .map_err(|e| {
                error!("failed to publish message: {}", e);
                reporter::ReporterError::Unavailable(e.to_string())
            })?;
            trace!("published: {} to {}", message, to);

            Ok(())
        })
    }
}
