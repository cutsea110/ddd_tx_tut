use aws_sdk_dynamodb::Client;
use chrono::NaiveDate;
use log::trace;

use crate::dao::{DaoError, PersonDao};
use crate::domain::{PersonId, Revision};
use crate::dto::PersonDto;

#[derive(Debug, Clone)]
pub struct DynamoDbPersonDao {
    client: Client,
}
impl DynamoDbPersonDao {
    pub async fn new() -> Self {
        let config = aws_config::load_from_env().await;
        let client = Client::new(&config);
        Self { client }
    }
}
impl PersonDao<()> for DynamoDbPersonDao {
    fn insert(&self, person: PersonDto) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
        tx_rs::with_tx(move |tx: &mut ()| Ok(42))
    }
    fn fetch(&self, id: PersonId) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = DaoError> {
        tx_rs::with_tx(move |tx: &mut ()| Ok(None))
    }
    fn select(&self) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = DaoError> {
        tx_rs::with_tx(move |tx: &mut ()| Ok(vec![]))
    }
    fn save(
        &self,
        id: PersonId,
        revision: Revision,
        person: PersonDto,
    ) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
        tx_rs::with_tx(move |tx: &mut ()| Ok(()))
    }
    fn delete(&self, id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
        tx_rs::with_tx(move |tx: &mut ()| Ok(()))
    }
}
