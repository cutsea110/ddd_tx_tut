use thiserror::Error;

use crate::domain::{Person, PersonId};

#[derive(Debug, Error)]
pub enum PersonUsecaseError {
    #[error("failed to save person")]
    SaveFailed,
    #[error("failed to get person")]
    GetFailed,
    #[error("failed to collect people")]
    CollectFailed,
    #[error("failed to modify person")]
    ModifyFailed,
    #[error("failed to unregister person")]
    UnregisterFailed,
}

pub type Result<T> = std::result::Result<T, PersonUsecaseError>;

pub struct PersonUsecase {
    // TODO
}

impl PersonUsecase {
    pub fn register(&self, person: &Person) -> Result<PersonId> {
        todo!()
    }

    pub fn get(&self, id: PersonId) -> Result<Option<Person>> {
        todo!()
    }

    pub fn collect(&self) -> Result<Vec<(PersonId, Person)>> {
        todo!()
    }

    pub fn modify(&self, id: PersonId, person: &Person) -> Result<()> {
        todo!()
    }

    pub fn unregister(&self, id: PersonId) -> Result<()> {
        todo!()
    }
}
