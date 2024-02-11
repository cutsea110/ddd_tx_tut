mod dao;
mod domain;
mod pg_db;
mod service;
mod usecase;

pub use dao::{DaoError, HavePersonDao, PersonDao};
pub use domain::{Person, PersonId};
pub use pg_db::PgPersonDao;
pub use service::PersonApi;
pub use usecase::{PersonUsecase, ServiceError};

fn main() {
    let mut api = PersonApi::new("postgres://admin:adminpass@localhost:15432/sampledb");

    // call api
    let (id, person) = api.register("cutsea", 53, "rustacean").unwrap();
    println!("id:{} {}", id, person);

    api.batch_import(vec![
        Person::new("Abel", 26, Some("Abel's theorem")),
        Person::new("Euler", 76, Some("Euler's identity")),
        Person::new("Galois", 20, Some("Group Theory")),
        Person::new("Gauss", 34, Some("King of Math")),
    ])
    .unwrap();
    println!("batch import done");

    let persons = api.list_all().expect("list all");
    for (id, person) in persons {
        println!("id:{} {}", id, person);
    }
}
