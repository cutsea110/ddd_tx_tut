use postgres::{Client, NoTls};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("dummy error")]
    Dummy,
}

#[derive(Debug)]
struct Person {
    name: String,
    age: i32,
    data: Option<String>,
}
impl Person {
    pub fn new(name: &str, age: i32, data: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            age,
            data: data.map(|d| d.to_string()),
        }
    }
}

trait PersonUsecase {
    fn entry(&mut self, person: &Person) -> Result<i32, MyError>;
    fn collect(&mut self) -> Result<Vec<(i32, Person)>, MyError>;
}

struct PersonUsecaseImpl {
    client: Client,
}
impl PersonUsecase for PersonUsecaseImpl {
    fn entry(&mut self, person: &Person) -> Result<i32, MyError> {
        self.run_tx(|tx| {
            tx.query_one(
                "INSERT INTO person (name, age, data) VALUES ($1, $2, $3) RETURNING id",
                &[
                    &person.name,
                    &person.age,
                    &person.data.clone().map(|d| d.into_bytes()),
                ],
            )
            .map(|r| r.get(0))
            .map_err(|_| MyError::Dummy)
        })
    }

    fn collect(&mut self) -> Result<Vec<(i32, Person)>, MyError> {
        self.run_tx(|tx| {
            let mut result = vec![];

            for row in tx
                .query("SELECT id, name, age, data FROM person", &[])
                .unwrap()
            {
                let id: i32 = row.get(0);
                let name: &str = row.get(1);
                let age: i32 = row.get(2);
                let data: Option<&[u8]> = row.get(3);

                result.push((
                    id,
                    Person::new(name, age, data.map(|d| std::str::from_utf8(d).unwrap())),
                ));
            }

            Ok(result)
        })
    }
}

impl PersonUsecaseImpl {
    pub fn new(url: &str) -> Self {
        let client = Client::connect(url, NoTls).unwrap();
        Self { client }
    }

    fn run_tx<T, F>(&mut self, f: F) -> Result<T, MyError>
    where
        F: FnOnce(&mut postgres::Transaction<'_>) -> Result<T, MyError>,
    {
        let mut transaction = self.client.transaction().unwrap();
        let result = f(&mut transaction);
        match result {
            Ok(v) => {
                transaction.commit().unwrap();
                Ok(v)
            }
            Err(e) => {
                transaction.rollback().unwrap();
                Err(e)
            }
        }
    }
}

fn main() {
    let mut usecase =
        PersonUsecaseImpl::new("postgresql://admin:adminpass@localhost:15432/sampledb");

    let persons = vec![
        Person::new("Gauss", 27, Some("King of Math")),
        Person::new("Galois", 20, Some("Group Theory")),
        Person::new("Abel", 26, Some("Abelian Group")),
        Person::new("Euler", 23, Some("Euler's Formula")),
    ];
    for person in persons {
        let id = usecase.entry(&person).unwrap();
        println!("inserted person {}", id);
    }

    let rows = usecase.collect().unwrap();
    for row in rows {
        println!("found {:?}", row);
    }
}
