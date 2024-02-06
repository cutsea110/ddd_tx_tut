use tx_rs::tx::{self, Tx};

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

pub mod pg_db {
    use postgres::{Client, Transaction};
    use std::fmt;
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum PgDbError {
        #[error("query failed: {0:?}")]
        QueryFailed(#[from] postgres::Error),
        #[error("failed to connect to database")]
        ConnectionFailed,
        #[error("failed to start transaction")]
        TransactionFailed,
        #[error("failed to commit")]
        CommitFailed,
        #[error("failed to rollback")]
        RollbackFailed,
    }

    use super::PersonRepository;
    use super::{Person, PersonId};
    use tx_rs::tx;

    pub struct PgPersonRepository<'a> {
        conn_str: &'a str,
        client: Client,
    }
    impl fmt::Display for PgPersonRepository<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "PgPersonRepository {{ conn_str: {} }}", self.conn_str)
        }
    }
    impl<'a> PgPersonRepository<'a> {
        pub fn new(conn_str: &'a str) -> Self {
            let client = Client::connect(conn_str, postgres::NoTls)
                .map_err(|_| PgDbError::ConnectionFailed)
                .expect("connect to database");

            Self { conn_str, client }
        }
    }
    impl<'a> PersonRepository<'a> for PgPersonRepository<'a> {
        type Ctx = Transaction<'a>;
        type Err = PgDbError;

        fn run_tx<Tx, T>(&'a mut self, tx: Tx) -> Result<T, Self::Err>
        where
            Tx: tx::Tx<Self::Ctx, Item = T, Err = Self::Err>,
        {
            let mut ctx = self
                .client
                .transaction()
                .map_err(|_| PgDbError::TransactionFailed)?;

            let result = tx.run(&mut ctx);

            if result.is_ok() {
                ctx.commit().map_err(|_| PgDbError::CommitFailed)?;
            } else {
                ctx.rollback().map_err(|_| PgDbError::RollbackFailed)?;
            }

            result
        }

        fn insert_person(
            person: &Person,
        ) -> impl tx::Tx<Self::Ctx, Item = PersonId, Err = Self::Err> {
            tx::with_tx(move |tx: &mut Self::Ctx| {
                let row = tx
                    .query_one(
                        "INSERT INTO person (name, age, data) VALUES ($1, $2, $3) RETURNING id",
                        &[&person.name, &person.age, &person.data],
                    )
                    .map_err(|e| PgDbError::QueryFailed(e))?;

                Ok(row.get(0))
            })
        }

        fn fetch_person(
            id: PersonId,
        ) -> impl tx::Tx<Self::Ctx, Item = Option<Person>, Err = Self::Err> {
            tx::with_tx(move |tx: &mut Self::Ctx| {
                match tx.query_one("SELECT name, age, data FROM person WHERE id = $1", &[&id]) {
                    Ok(row) => Ok(Some(Person::new(row.get(0), row.get(1), row.get(2)))),
                    Err(e) => {
                        eprintln!("error fetching person: {}", e);
                        Ok(None)
                    }
                }
            })
        }
    }
}
use pg_db::PgPersonRepository as db;

fn main() {
    let mut dao =
        pg_db::PgPersonRepository::new("postgresql://admin:adminpass@localhost:15432/sampledb");

    let person = Person::new("Gauss", 21, None);

    let result = dao
        .run_tx(db::insert_person(&person).and_then(|id| db::fetch_person(id)))
        .expect("run tx");

    println!("{:?}", result);
}
