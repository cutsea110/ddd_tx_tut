mod domain;
mod pg_db;

use thiserror::Error;

use domain::person::{Person, PersonId, PersonRepository};
use pg_db::PgPersonRepository as db;
use tx_rs::{with_tx, Tx};

const DB_URL: &str = "postgresql://admin:adminpass@localhost:15432/sampledb";

#[derive(Error, Debug)]
pub enum MyError {
    #[error("dummy error")]
    Dummy,
}

type Result<T> = std::result::Result<T, MyError>;

// TODO: make this trait and independent from concrete repository
fn register_person(person: &Person) -> Result<Option<Person>> {
    db::new(DB_URL)
        .run_tx(db::create(person).and_then(|id| db::fetch(id)))
        .map_err(|_| MyError::Dummy)
}

fn batch_register_persons(persons: &[Person]) -> Result<Vec<PersonId>> {
    db::new(DB_URL)
        .run_tx(with_tx(|tx| {
            let mut ids = vec![];
            for p in persons {
                let id = db::create(p).run(tx)?;
                ids.push(id);
            }
            Ok(ids)
        }))
        .map_err(|_| MyError::Dummy)
}

fn unregister_all_persons() -> Result<()> {
    db::new(DB_URL)
        .run_tx(with_tx(|tx| {
            let ps = db::collect().run(tx)?;
            for (id, p) in ps {
                println!("{} {}", id, p);
                db::delete(id).run(tx)?;
            }
            Ok(())
        }))
        .map_err(|_| MyError::Dummy)
}

fn main() {
    // test insert and fetch
    let p = register_person(&Person::new("cutsea", 53, None)).expect("register cutsea");
    println!("registered: {:?}", p);

    // test insert persons
    let ps = [
        Person::new("Gauss", 21, Some(b"Number theory".as_ref())),
        Person::new("Galois", 16, Some(b"Group theory".as_ref())),
        Person::new("Abel", 26, Some(b"Group theory".as_ref())),
        Person::new("Euler", 23, Some(b"Mathematical analysis".as_ref())),
    ];
    let ids = batch_register_persons(&ps).expect("batch register");
    println!("registered ids: {:?}", ids);

    // test collect and delete
    unregister_all_persons().expect("unregister all");
    println!("unregistered all");
}
