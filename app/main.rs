use tx_rs::tx::{self, Tx};

pub mod pg_db;
use pg_db::PgPersonRepository as db;

type PersonId = i32;
#[derive(Debug, Clone, Eq, PartialEq)]
struct Person {
    name: String,
    age: i32,
    data: Option<Vec<u8>>,
}
impl Person {
    fn new(name: &str, age: i32, data: Option<&[u8]>) -> Self {
        Self {
            name: name.to_string(),
            age,
            data: data.map(|d| d.to_vec()),
        }
    }
}

trait PersonRepository<'a> {
    type Ctx;
    type Err;

    fn run_tx<Tx, T>(&'a mut self, tx: Tx) -> Result<T, Self::Err>
    where
        Tx: tx::Tx<Self::Ctx, Item = T, Err = Self::Err>;

    fn insert_person(person: &Person) -> impl tx::Tx<Self::Ctx, Item = PersonId, Err = Self::Err>;
    fn fetch_person(id: PersonId)
        -> impl tx::Tx<Self::Ctx, Item = Option<Person>, Err = Self::Err>;
}

fn main() {
    let mut dao =
        pg_db::PgPersonRepository::new("postgresql://admin:adminpass@localhost:15432/sampledb");

    let person = Person::new("Gauss", 21, None);

    let result = dao
        .run_tx(db::insert_person(&person).and_then(|id| db::fetch_person(id)))
        .expect("run tx");

    println!("{:?}", result);
}
