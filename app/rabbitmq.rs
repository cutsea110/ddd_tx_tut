pub use log::error;
pub use thiserror::Error;

#[derive(Error, Debug)]
pub enum RabbitmqError {
    #[error("failed to open rabbitmq client")]
    OpenError,
}

pub struct Client {
    addr: String,
}
impl Client {
    pub fn open(addr: &str) -> Result<Self, RabbitmqError> {
        Ok(Self {
            addr: addr.to_string(),
        })
    }
    pub async fn connect(&self) -> Result<lapin::Connection, RabbitmqError> {
        lapin::Connection::connect(&self.addr, lapin::ConnectionProperties::default())
            .await
            .map_err(|e| {
                error!("failed to connect to rabbitmq: {}", e);
                RabbitmqError::OpenError
            })
    }
}
