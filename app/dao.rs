use thiserror::Error;

use crate::domain::PersonId;
use crate::dto::PersonLayout;

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
    fn insert(&self, person: PersonLayout) -> impl tx_rs::Tx<Ctx, Item = PersonId, Err = DaoError>;
    fn fetch(
        &self,
        id: PersonId,
    ) -> impl tx_rs::Tx<Ctx, Item = Option<PersonLayout>, Err = DaoError>;
    fn select(&self) -> impl tx_rs::Tx<Ctx, Item = Vec<(PersonId, PersonLayout)>, Err = DaoError>;
    fn save(
        &self,
        id: PersonId,
        person: PersonLayout,
    ) -> impl tx_rs::Tx<Ctx, Item = (), Err = DaoError>;
    fn delete(&self, id: PersonId) -> impl tx_rs::Tx<Ctx, Item = (), Err = DaoError>;
}

pub trait HavePersonDao<Ctx> {
    fn get_dao(&self) -> Box<&impl PersonDao<Ctx>>;
}
