use thiserror::Error;

use crate::domain::PersonId;
use crate::dto::PersonDto;

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

    fn find(&self, id: PersonId) -> impl tx_rs::Tx<Ctx, Item = Option<PersonDto>, Err = CaoError>;
    fn load(
        &self,
        id: PersonId,
        person: &PersonDto,
    ) -> impl tx_rs::Tx<Ctx, Item = (), Err = CaoError>;
    fn unload(&self, id: PersonId) -> impl tx_rs::Tx<Ctx, Item = (), Err = CaoError>;
}
