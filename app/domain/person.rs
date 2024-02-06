use core::fmt;

use tx_rs;

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
    type Err;

    fn run_tx<Tx, T>(&'a mut self, tx: Tx) -> Result<T, Self::Err>
    where
        Tx: tx_rs::Tx<Self::Ctx, Item = T, Err = Self::Err>;

    fn insert_person(
        person: &Person,
    ) -> impl tx_rs::Tx<Self::Ctx, Item = PersonId, Err = Self::Err>;
    fn fetch_person(
        id: PersonId,
    ) -> impl tx_rs::Tx<Self::Ctx, Item = Option<Person>, Err = Self::Err>;
    fn collect_persons(
    ) -> impl tx_rs::Tx<Self::Ctx, Item = Vec<(PersonId, Person)>, Err = Self::Err>;
    fn update_person(
        id: PersonId,
        person: &Person,
    ) -> impl tx_rs::Tx<Self::Ctx, Item = (), Err = Self::Err>;
    fn delete_person(id: PersonId) -> impl tx_rs::Tx<Self::Ctx, Item = (), Err = Self::Err>;
}
