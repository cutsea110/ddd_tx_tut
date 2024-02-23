use thiserror::Error;

use crate::{Person, PersonId};

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CaoError {
    #[error("dummy todo")]
    Dummy,
}

// Cao = Cache Access Object
pub trait PersonCao<Ctx> {
    fn exists(&self, key: &PersonId) -> impl tx_rs::Tx<Ctx, Item = bool, Err = CaoError>;
    fn find(&self, key: &PersonId) -> impl tx_rs::Tx<Ctx, Item = Option<Person>, Err = CaoError>;
    fn save(
        &self,
        key: &PersonId,
        value: &Person,
    ) -> impl tx_rs::Tx<Ctx, Item = (), Err = CaoError>;
    fn expire(&self, key: &PersonId) -> impl tx_rs::Tx<Ctx, Item = (), Err = CaoError>;
}

pub trait HavePersonCao<Ctx> {
    fn get_cao<'a>(&'a self) -> Box<&impl PersonCao<Ctx>>;
}
