use postgres::{Client, NoTls};
use std::cell::{RefCell, RefMut};
use std::env;
use std::rc::Rc;

mod dao;
mod domain;
mod pg_db;
mod service;
mod usecase;

pub use dao::{DaoError, HavePersonDao, PersonDao};
pub use domain::{Person, PersonId};
pub use pg_db::PgPersonDao;
pub use service::{PersonService, ServiceError};
pub use usecase::{PersonUsecase, UsecaseError};

#[derive(Debug, Clone)]
pub struct PersonUsecaseImpl {
    dao: Rc<PgPersonDao>,
}
impl PersonUsecaseImpl {
    pub fn new(dao: Rc<PgPersonDao>) -> Self {
        Self { dao }
    }
}
impl<'a> PersonUsecase<postgres::Transaction<'a>> for PersonUsecaseImpl {}
impl<'a> HavePersonDao<postgres::Transaction<'a>> for PersonUsecaseImpl {
    fn get_dao<'b>(&'b self) -> Box<&impl PersonDao<postgres::Transaction<'a>>> {
        Box::new(&*self.dao)
    }
}

pub struct PersonServiceImpl {
    db_client: Client,
    usecase: Rc<RefCell<PersonUsecaseImpl>>,
}
impl PersonServiceImpl {
    pub fn new(db_url: &str) -> Self {
        let dao = PgPersonDao;
        let usecase = PersonUsecaseImpl::new(Rc::new(dao));
        let db_client = Client::connect(db_url, NoTls).unwrap();

        Self {
            db_client,
            usecase: Rc::new(RefCell::new(usecase)),
        }
    }
}
impl<'a> PersonService<'a, postgres::Transaction<'a>> for PersonServiceImpl {
    type U = PersonUsecaseImpl;

    // api is responsible for transaction management
    fn run_tx<T, F>(&'a mut self, f: F) -> Result<T, ServiceError>
    where
        F: FnOnce(
            &mut RefMut<'_, PersonUsecaseImpl>,
            &mut postgres::Transaction<'a>,
        ) -> Result<T, UsecaseError>,
    {
        let mut usecase = self.usecase.borrow_mut();
        let mut ctx = self.db_client.transaction().unwrap();

        let res = f(&mut usecase, &mut ctx);

        match res {
            Ok(v) => {
                ctx.commit().unwrap();
                Ok(v)
            }
            Err(e) => {
                ctx.rollback().unwrap();
                Err(ServiceError::TransactionFailed(e))
            }
        }
    }
}

fn main() {
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://admin:adminpass@localhost:15432/sampledb".to_string());
    let mut service = PersonServiceImpl::new(&db_url);

    let (id, person) = service.register("cutsea", 53, "rustacean").unwrap();
    println!("id:{} {}", id, person);

    service
        .batch_import(vec![
            Person::new("Abel", 26, Some("Abel's theorem")),
            Person::new("Euler", 76, Some("Euler's identity")),
            Person::new("Galois", 20, Some("Group Theory")),
            Person::new("Gauss", 34, Some("King of Math")),
        ])
        .unwrap();
    println!("batch import done");

    let persons = service.list_all().expect("list all");
    for (id, person) in persons {
        println!("id:{} {}", id, person);
    }
}
