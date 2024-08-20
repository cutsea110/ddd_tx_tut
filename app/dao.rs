use thiserror::Error;

use crate::domain::{PersonId, Revision};
use crate::dto::PersonDto;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DaoError {
    #[error("insert error: {0}")]
    InsertError(String),
    #[error("select error: {0}")]
    SelectError(String),
    #[error("update error: {0}")]
    UpdateError(String),
    #[error("delete error: {0}")]
    DeleteError(String),
}
pub trait PersonDao<Ctx> {
    fn insert(&self, person: PersonDto) -> impl tx_rs::Tx<Ctx, Item = PersonId, Err = DaoError>;
    fn fetch(&self, id: PersonId) -> impl tx_rs::Tx<Ctx, Item = Option<PersonDto>, Err = DaoError>;
    fn select(&self) -> impl tx_rs::Tx<Ctx, Item = Vec<(PersonId, PersonDto)>, Err = DaoError>;
    fn save(
        &self,
        id: PersonId,
        revision: Revision,
        person: PersonDto,
    ) -> impl tx_rs::Tx<Ctx, Item = (), Err = DaoError>;
    fn delete(&self, id: PersonId) -> impl tx_rs::Tx<Ctx, Item = (), Err = DaoError>;
}

pub trait HavePersonDao<Ctx> {
    fn get_dao(&self) -> &impl PersonDao<Ctx>;
}
