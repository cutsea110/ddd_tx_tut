use core::fmt;
use thiserror::Error;

use tx_rs;

#[derive(Debug, Error)]
pub enum PersonRepositoryError {
    #[error("connect failed")]
    ConnectFailed,
    #[error("transaction failed")]
    TransactionFailed,
    #[error("commit failed")]
    CommitFailed,
    #[error("rollback failed")]
    RollbackFailed,
    #[error("create failed")]
    CreateFailed,
    #[error("fetch failed")]
    FetchFailed,
    #[error("collect failed")]
    CollectFailed,
    #[error("update failed")]
    UpdateFailed,
    #[error("delete failed")]
    DeleteFailed,
}

pub type Result<T> = std::result::Result<T, PersonRepositoryError>;

pub type PersonId = i32;
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Person {
    pub name: String,
    pub age: i32,
    pub data: Option<Vec<u8>>,
}
impl fmt::Display for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Person {{ name: {}, age: {}, data: '{}' }}",
            self.name,
            self.age,
            std::str::from_utf8(&self.data.as_ref().unwrap_or(&vec![])).unwrap_or("N/A")
        )
    }
}

impl Person {
    pub fn new(name: &str, age: i32, data: Option<&[u8]>) -> Self {
        Self {
            name: name.to_string(),
            age,
            data: data.map(|d| d.to_vec()),
        }
    }
}

pub trait PersonRepository<'a> {
    type Ctx;

    fn run_tx<Tx, T>(&'a mut self, tx: Tx) -> Result<T>
    where
        Tx: tx_rs::Tx<Self::Ctx, Item = T, Err = PersonRepositoryError>;

    fn create(person: &Person) -> impl tx_rs::Tx<Self::Ctx, Item = PersonId>;
    fn fetch(id: PersonId) -> impl tx_rs::Tx<Self::Ctx, Item = Option<Person>>;
    fn collect() -> impl tx_rs::Tx<Self::Ctx, Item = Vec<(PersonId, Person)>>;
    fn update(id: PersonId, person: &Person) -> impl tx_rs::Tx<Self::Ctx, Item = ()>;
    fn delete(id: PersonId) -> impl tx_rs::Tx<Self::Ctx, Item = ()>;
}
