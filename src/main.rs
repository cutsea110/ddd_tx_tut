use postgres::{Client, NoTls, Transaction};

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

trait PersonRepository {
    type Tx;

    fn with_tx<F, T, E>(&mut self, q: F) -> Result<T, E>
    where
        F: FnOnce(&mut Self::Tx) -> Result<T, E>;

    fn insert_person(tx: &mut Self::Tx, person: &Person) -> PersonId;
    fn fetch_person(tx: &mut Self::Tx, id: PersonId) -> Option<Person>;
}

fn insert_person(tx: &mut Transaction<'_>, person: &Person) -> PersonId {
    // execute ではなく query を使うことで id を取得できる
    let row = tx
        .query_one(
            "INSERT INTO person (name, age, data) VALUES ($1, $2, $3) RETURNING id",
            &[&person.name, &person.age, &person.data],
        )
        .unwrap();

    row.get(0)
}

fn fetch_person(tx: &mut Transaction<'_>, id: PersonId) -> Option<Person> {
    match tx.query_one("SELECT name, age, data FROM person WHERE id = $1", &[&id]) {
        Ok(row) => Some(Person::new(row.get(0), row.get(1), row.get(2))),
        Err(e) => {
            eprintln!("error fetching person: {}", e);
            None
        }
    }
}

fn with_tx<F, T, E>(client: &mut Client, q: F) -> Result<T, E>
where
    F: FnOnce(&mut Transaction<'_>) -> Result<T, E>,
{
    let mut tx = client.transaction().unwrap();

    match q(&mut tx) {
        Ok(ret) => {
            tx.commit().unwrap();
            Ok(ret)
        }
        Err(e) => {
            tx.rollback().unwrap();
            Err(e)
        }
    }
}

fn main() {
    let mut client = Client::connect(
        "postgresql://admin:adminpass@localhost:15432/sampledb",
        NoTls,
    )
    .unwrap();

    let person = with_tx(&mut client, |tx| {
        let person = Person::new("Ferris", 42, None);
        let id = insert_person(tx, &person);
        fetch_person(tx, id).ok_or(())
    });

    match person {
        Ok(p) => println!("found person {:?}", p),
        Err(e) => println!("no person found: {:?}", e),
    }
}
