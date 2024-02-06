mod domain;
mod pg_db;

use domain::{Person, PersonRepository};
use pg_db::PgPersonRepository as db;
use tx_rs::tx::{with_tx, Tx};

const DB_URL: &str = "postgresql://admin:adminpass@localhost:15432/sampledb";

fn main() {
    // test insert and fetch
    {
        let person = Person::new("cutsea", 53, None);

        let result = db::new(DB_URL)
            .run_tx(db::insert_person(&person).and_then(|id| db::fetch_person(id)))
            .expect("run tx");

        println!("{:?}", result);
    }

    // test insert persons
    {
        let ids = db::new(DB_URL)
            .run_tx(with_tx(|tx| {
                let mut ids = vec![];
                let ps = [
                    Person::new("Gauss", 21, Some(b"Number theory".as_ref())),
                    Person::new("Galois", 16, Some(b"Group theory".as_ref())),
                    Person::new("Abel", 26, Some(b"Group theory".as_ref())),
                    Person::new("Euler", 23, Some(b"Mathematical analysis".as_ref())),
                ];
                for p in ps {
                    let id = db::insert_person(&p).run(tx)?;
                    ids.push(id);
                }

                Ok(ids)
            }))
            .expect("run tx");

        println!("{:?}", ids);
    }

    // test collect and delete
    {
        let _ = db::new(DB_URL)
            .run_tx(with_tx(|tx| {
                let ps = db::collect_persons().run(tx)?;
                for (id, p) in ps {
                    println!("{}: {}", id, p);
                    db::delete_person(id).run(tx)?;
                }
                Ok(())
            }))
            .expect("run tx");
    }
}
