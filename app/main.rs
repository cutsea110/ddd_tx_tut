use log::{error, trace};
use postgres::NoTls;
use std::cell::RefCell;
use std::env;
use std::time::Duration;

mod cache;
mod cached_service;
mod dao;
mod domain;
mod dto;
mod notifier;
mod pg_db;
mod rabbitmq;
mod redis_cache;
mod service;
mod usecase;

use cached_service::PersonCachedService;
use dao::HavePersonDao;
use domain::date;
use pg_db::PgPersonDao;
use service::{PersonService, ServiceError};
use usecase::{PersonUsecase, UsecaseError};

use crate::dto::PersonDto;

#[derive(Debug, Clone)]
pub struct PersonUsecaseImpl {
    dao: PgPersonDao,
}
impl PersonUsecaseImpl {
    pub fn new(dao: PgPersonDao) -> Self {
        Self { dao }
    }
}
impl<'a> PersonUsecase<postgres::Transaction<'a>> for PersonUsecaseImpl {}
impl<'a> HavePersonDao<postgres::Transaction<'a>> for PersonUsecaseImpl {
    fn get_dao<'b>(&'b self) -> Box<&impl dao::PersonDao<postgres::Transaction<'a>>> {
        Box::new(&self.dao)
    }
}

pub struct PersonServiceImpl {
    db_client: postgres::Client,
    cache_client: redis::Client,
    mq_client: rabbitmq::Client,
    usecase: RefCell<PersonUsecaseImpl>,
}
impl PersonServiceImpl {
    pub fn new(db_uri: &str, cache_uri: &str, mq_uri: &str) -> Self {
        let db_client = postgres::Client::connect(db_uri, NoTls).expect("create db client");
        let cache_client = redis::Client::open(cache_uri).expect("create cache client");
        let mq_client = rabbitmq::Client::open(mq_uri).expect("create mq client");

        let usecase = RefCell::new(PersonUsecaseImpl::new(PgPersonDao));

        Self {
            db_client,
            cache_client,
            mq_client,
            usecase,
        }
    }
}
impl<'a> PersonService<'a, postgres::Transaction<'a>> for PersonServiceImpl {
    type U = PersonUsecaseImpl;
    type N = rabbitmq::Client;

    // service is responsible for transaction management
    fn run_tx<T, F>(&'a mut self, f: F) -> Result<T, ServiceError>
    where
        F: FnOnce(
            &mut PersonUsecaseImpl,
            &mut postgres::Transaction<'a>,
        ) -> Result<T, UsecaseError>,
    {
        let mut ctx = self.db_client.transaction().map_err(|e| {
            error!("failed to start transaction: {}", e);
            ServiceError::ServiceUnavailable(format!("{}", e))
        })?;
        trace!("transaction started");

        let mut usecase = self.usecase.borrow_mut();
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

    fn get_notifier(&self) -> Self::N {
        self.mq_client.clone()
    }
}
impl<'a> PersonCachedService<'a, redis::Connection, postgres::Transaction<'a>>
    for PersonServiceImpl
{
    type C = redis_cache::RedisPersonCao;

    fn get_cao(&self) -> Self::C {
        redis_cache::RedisPersonCao::new(self.cache_client.clone(), Duration::from_secs(2))
    }
}

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let cache_uri =
        env::var("CACHE_URI").unwrap_or("redis://:adminpass@localhost:16379".to_string());
    let db_uri = env::var("DATABASE_URI").unwrap_or(
        "postgres://admin:adminpass@localhost:15432/sampledb?connect_timeout=2".to_string(),
    );
    let mq_uri = env::var("AMQP_URI").unwrap_or(
        // connection_timeout is in milliseconds
        "amqp://admin:adminpass@localhost:5672/%2f?connection_timeout=2000".to_string(),
    );
    let mut service = PersonServiceImpl::new(&db_uri, &cache_uri, &mq_uri);

    let (id, person) = service
        .cached_register("poor man", date(2001, 9, 11), None, "one person")
        .expect("register one person");
    println!("id:{} {:?}", id, person);

    if let Some(p) = service.cached_find(id).expect("find person") {
        println!("cache hit:{:?}", p);

        let death_date = date(2011, 3, 11);
        println!("death {} at:{:?}", id, death_date);
        service.cached_death(id, death_date).expect("kill person");

        if let Some(p) = service.cached_find(id).expect("find dead person") {
            println!("dead person: {:?}", p);
        }
    }
    service.cached_unregister(id).expect("delete person");

    let persons = vec![
        (
            "Abel",
            date(1802, 8, 5)..=date(1829, 4, 6),
            "Abel's theorem",
        ),
        (
            "Euler",
            date(1707, 4, 15)..=date(1783, 9, 18),
            "Euler's identity",
        ),
        (
            "Galois",
            date(1811, 10, 25)..=date(1832, 5, 31),
            "Group Theory",
        ),
        (
            "Gauss",
            date(1777, 4, 30)..=date(1855, 2, 23),
            "King of Math",
        ),
    ]
    .into_iter()
    .map(|(name, life, desc)| {
        let (from, to) = (*life.start(), *life.end());
        PersonDto::new(name, from, Some(to), Some(desc))
    })
    .collect::<Vec<_>>();

    let ids = service
        .cached_batch_import(persons.clone())
        .expect("batch import");
    println!("batch import done");

    let persons = service.cached_list_all().expect("list all");
    for (id, _) in &persons {
        if let Some(p) = service.cached_find(*id).expect("find person") {
            println!("cache hit:{} {:?}", id, p);
        }
    }

    for id in ids {
        println!("unregister id:{}", id);
        service.cached_unregister(id).expect("unregister");
    }

    println!("done everything!");
}
