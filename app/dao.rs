use thiserror::Error;

use crate::domain::{Person, PersonId};

#[derive(Error, Debug)]
pub enum DaoError {
    #[error("insert error: {0}")]
    InsertError(String),
    #[error("select error: {0}")]
    SelectError(String),
}
pub trait PersonDao<Ctx> {
    fn insert(&self, person: Person) -> impl tx_rs::Tx<Ctx, Item = PersonId, Err = DaoError>;
    fn fetch(&self, id: PersonId) -> impl tx_rs::Tx<Ctx, Item = Option<Person>, Err = DaoError>;
    fn select(&self) -> impl tx_rs::Tx<Ctx, Item = Vec<(PersonId, Person)>, Err = DaoError>;
}

pub trait HavePersonDao<Ctx> {
    fn get_dao(&self) -> Box<&impl PersonDao<Ctx>>;
}
