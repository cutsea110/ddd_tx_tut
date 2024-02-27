use thiserror::Error;

use crate::domain::{Person, PersonId};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CaoError {
    #[error("cache unavailable: {0}")]
    Unavailable(String),
}

pub trait PersonCao<Ctx> {
    fn get_conn(&self) -> Result<Ctx, CaoError>;

    fn run_tx<T, F>(&self, f: F) -> Result<T, CaoError>
    where
        F: tx_rs::Tx<Ctx, Item = T, Err = CaoError>;

    fn exists(&self, id: PersonId) -> impl tx_rs::Tx<Ctx, Item = bool, Err = CaoError>;
    fn find(&self, id: PersonId) -> impl tx_rs::Tx<Ctx, Item = Option<Person>, Err = CaoError>;
    fn load(&self, id: PersonId, person: &Person)
        -> impl tx_rs::Tx<Ctx, Item = (), Err = CaoError>;
    fn unload(&self, id: PersonId) -> impl tx_rs::Tx<Ctx, Item = (), Err = CaoError>;
}
