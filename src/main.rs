use postgres::{Client, NoTls, Transaction};

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

fn insert_person(tx: &mut Transaction<'_>, person: &Person) -> i32 {
    let row = tx
        .query_one(
            "INSERT INTO person (name, age, data) VALUES ($1, $2, $3) RETURNING id",
            &[&person.name, &person.age, &person.data],
        )
        .unwrap();

    row.get(0)
}

fn fetch_person(tx: &mut Transaction<'_>, id: i32) -> Option<Person> {
    match tx.query_one("SELECT name, age, data FROM person WHERE id = $1", &[&id]) {
        Ok(row) => Some(Person::new(row.get(0), row.get(1), row.get(2))),
        Err(e) => {
            eprintln!("error fetching person: {}", e);
            None
        }
    }
}

fn with_tx<F, T>(client: &mut Client, q: F) -> Option<T>
where
    F: FnOnce(&mut Transaction<'_>) -> Option<T>,
{
    let mut tx = client.transaction().unwrap();

    let ret = q(&mut tx);

    tx.commit().unwrap();

    ret
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
        fetch_person(tx, id)
    });

    match person {
        Some(person) => println!("found person {:?}", person),
        None => println!("no person found"),
    }
}
