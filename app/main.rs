use log::{error, trace};
use postgres::NoTls;
use std::cell::{RefCell, RefMut};
use std::env;
use std::rc::Rc;

mod cache;
mod dao;
mod domain;
mod pg_db;
mod redis_cache;
mod service;
mod usecase;

pub use cache::{CaoError, PersonCao};
pub use dao::{DaoError, HavePersonDao};
pub use domain::{Person, PersonId};
pub use pg_db::PgPersonDao;
pub use service::{PersonService, ServiceError};
pub use usecase::{PersonUsecase, UsecaseError};

use crate::domain::date;

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
    fn get_dao<'b>(&'b self) -> Box<&impl dao::PersonDao<postgres::Transaction<'a>>> {
        Box::new(&*self.dao)
    }
}

pub struct PersonServiceImpl {
    db_client: postgres::Client,
    usecase: Rc<RefCell<PersonUsecaseImpl>>,
}
impl PersonServiceImpl {
    pub fn new(db_url: &str) -> Self {
        let db_client = postgres::Client::connect(db_url, NoTls).expect("create db client");

        let usecase = PersonUsecaseImpl::new(Rc::new(PgPersonDao));

        Self {
            db_client,
            usecase: Rc::new(RefCell::new(usecase)),
        }
    }
}
impl<'a> PersonService<'a, postgres::Transaction<'a>> for PersonServiceImpl {
    type U = PersonUsecaseImpl;

    // service is responsible for transaction management
    fn run_tx<T, F>(&'a mut self, f: F) -> Result<T, ServiceError>
    where
        F: FnOnce(
            &mut RefMut<'_, PersonUsecaseImpl>,
            &mut postgres::Transaction<'a>,
        ) -> Result<T, UsecaseError>,
    {
        let mut usecase = self.usecase.borrow_mut();
        let mut ctx = match self.db_client.transaction() {
            Ok(ctx) => {
                trace!("transaction started");
                ctx
            }
            Err(e) => {
                error!("failed to start transaction: {}", e);
                return Err(ServiceError::ServiceUnavailable(format!("{}", e)));
            }
        };

        let res = f(&mut usecase, &mut ctx);

        match res {
            Ok(v) => {
                ctx.commit().expect("commit");
                trace!("transaction committed");
                Ok(v)
            }
            Err(e) => {
                ctx.rollback().expect("rollback");
                error!("transaction rollbacked");
                Err(ServiceError::TransactionFailed(e))
            }
        }
    }
}

fn main() {
    env_logger::init();

    let cache_url =
        env::var("CACHE_URL").unwrap_or_else(|_| "redis://:adminpass@localhost:16379".to_string());
    let cache_client = redis::Client::open(cache_url.as_str()).expect("create cache client");
    let mut con = cache_client.get_connection().expect("connect to cache");

    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://admin:adminpass@localhost:15432/sampledb?connect_timeout=2".to_string()
    });
    let mut service = PersonServiceImpl::new(&db_url);

    let cao = redis_cache::RedisPersonCao;

    let (id, person) = service
        .register("cutsea", date(1970, 11, 6), None, "rustacean")
        .expect("register one person");
    println!("id:{} {}", id, person);
    // save on redis cache
    let _: () = (cao.save(id, &person))(&mut con).expect("save cache");

    if (cao.exists(id))(&mut con).expect("check existence in cache") {
        if let Some(p) = (cao.find(id))(&mut con).expect("find cache") {
            println!("cache hit:{}", p);
        }
    }
    let _: () = (cao.discard(id))(&mut con).expect("discard cache");
    service.unregister(id).expect("delete person");

    let persons = vec![
        Person::new(
            "Abel",
            date(1802, 8, 5),
            date(1829, 4, 6).into(),
            Some("Abel's theorem"),
        ),
        Person::new(
            "Euler",
            date(1707, 4, 15),
            date(1783, 9, 18).into(),
            Some("Euler's identity"),
        ),
        Person::new(
            "Galois",
            date(1811, 10, 25),
            date(1832, 5, 31).into(),
            Some("Group Theory"),
        ),
        Person::new(
            "Gauss",
            date(1777, 4, 30),
            date(1855, 2, 23).into(),
            Some("King of Math"),
        ),
    ];

    let ids = service.batch_import(persons.clone()).expect("batch import");
    println!("batch import done");

    ids.iter().zip(persons.iter()).for_each(|(id, p)| {
        let _: () = (cao.save(*id, p))(&mut con).expect("save cache");
        println!("save cache: {} {}", id, p);
    });

    let persons = service.list_all().expect("list all");
    for (id, person) in &persons {
        println!("found id:{} {}", id, person);
        if let Some(p) = (cao.find(*id))(&mut con).expect("find cache") {
            println!("cache hit:{}", p);
        } else {
            println!("cache miss:{}", id);
        }
    }

    for id in ids {
        let _: () = (cao.discard(id))(&mut con).expect("delete cache");
        println!("delete cache id:{}", id);
        println!("unregister id:{}", id);
        service.unregister(id).expect("unregister");
    }

    println!("done everything!");
}
