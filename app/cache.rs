use thiserror::Error;

use crate::{Person, PersonId};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CaoError {
    #[error("cache unavailable: {0}")]
    Unavailable(String),
}

pub trait PersonCao<Ctx> {
    fn get_conn(&self) -> Result<Ctx, CaoError>;

    fn exists(&self, id: PersonId) -> impl FnOnce(&mut Ctx) -> Result<bool, CaoError>;
    fn find(&self, id: PersonId) -> impl FnOnce(&mut Ctx) -> Result<Option<Person>, CaoError>;
    fn save(&self, id: PersonId, person: &Person) -> impl FnOnce(&mut Ctx) -> Result<(), CaoError>;
    fn discard(&self, id: PersonId) -> impl FnOnce(&mut Ctx) -> Result<(), CaoError>;
}
