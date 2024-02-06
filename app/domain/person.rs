use tx_rs::tx;

pub type PersonId = i32;
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Person {
    pub name: String,
    pub age: i32,
    pub data: Option<Vec<u8>>,
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
        Tx: tx::Tx<Self::Ctx, Item = T, Err = Self::Err>;

    fn insert_person(person: &Person) -> impl tx::Tx<Self::Ctx, Item = PersonId, Err = Self::Err>;
    fn fetch_person(id: PersonId)
        -> impl tx::Tx<Self::Ctx, Item = Option<Person>, Err = Self::Err>;
}
