mod domain;
mod pg_db;

use domain::{Person, PersonRepository};
use pg_db::PgPersonRepository as db;
use tx_rs::tx::Tx;

fn main() {
    let mut dao =
        pg_db::PgPersonRepository::new("postgresql://admin:adminpass@localhost:15432/sampledb");

    let person = Person::new("Gauss", 21, None);

    let result = dao
        .run_tx(db::insert_person(&person).and_then(|id| db::fetch_person(id)))
        .expect("run tx");

    println!("{:?}", result);
}
