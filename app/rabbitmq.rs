pub use log::{error, trace};
pub use std::rc::Rc;

use crate::reporter::{self, Level, Location};

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

impl reporter::Reporter for Client {
    // to: queue name
    // message: message to send
    fn send_report(
        &self,
        _level: Level, // TODO: use level
        to: &str,
        message: &str,
        _loc: Location, // TODO: use loc
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
            chan.basic_publish(
                "",
                to,
                lapin::options::BasicPublishOptions::default(),
                message.as_bytes(),
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
