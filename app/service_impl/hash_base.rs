use log::{error, trace};
use std::{cell::RefCell, cell::RefMut, collections::HashMap, process, rc::Rc, time::Duration};

use crate::cached_service::PersonCachedService;
use crate::dao::{self, HavePersonDao};
use crate::domain::PersonId;
use crate::dto::PersonDto;
use crate::hs_db::HashDB;
use crate::rabbitmq;
use crate::redis_cache;
use crate::reporter::{DefaultReporter, Reporter};
use crate::service::{PersonOutputBoundary, PersonService, ServiceError};
use crate::usecase::{PersonUsecase, UsecaseError};

#[derive(Debug, Clone)]
pub struct PersonUsecaseImpl {
    dao: HashDB,
}
impl PersonUsecaseImpl {
    #[cfg_attr(any(feature = "use_pq", feature = "use_dynamo"), allow(unused))]
    pub fn new(dao: HashDB) -> Self {
        Self { dao }
    }
}
impl<'a> PersonUsecase<RefMut<'a, HashMap<PersonId, PersonDto>>> for PersonUsecaseImpl {}
impl<'a> HavePersonDao<RefMut<'a, HashMap<PersonId, PersonDto>>> for PersonUsecaseImpl {
    fn get_dao(&self) -> &impl dao::PersonDao<RefMut<'a, HashMap<PersonId, PersonDto>>> {
        &self.dao
    }
}

pub struct PersonServiceImpl {
    hs_db: HashDB,
    cache_client: redis::Client,
    reporter: DefaultReporter<'static>,
    usecase: RefCell<PersonUsecaseImpl>,
}
impl PersonServiceImpl {
    #[cfg_attr(any(feature = "use_pq", feature = "use_dynamo"), allow(unused))]
    pub fn new(runtime: Rc<tokio::runtime::Runtime>, cache_uri: &str, mq_uri: &str) -> Self {
        let pid = process::id();
        trace!("pid: {}", pid);
        let cache_client = redis::Client::open(cache_uri).expect("create cache client");
        let mq_client = rabbitmq::Client::open(runtime, mq_uri).expect("create mq client");
        let syslog_client =
            crate::syslog::Client::new("ddd_tx_tut", pid).expect("crate syslog client");
        let mut reporter = DefaultReporter::new();
        reporter
            .register(mq_client)
            .expect("register observer: rabbitmq");
        reporter
            .register(syslog_client)
            .expect("register observer: syslog");

        let dao = HashDB::new();
        let usecase = RefCell::new(PersonUsecaseImpl::new(dao.clone()));

        Self {
            hs_db: dao,
            cache_client,
            reporter,
            usecase,
        }
    }
}
impl<'a> PersonService<'a, RefMut<'a, HashMap<PersonId, PersonDto>>> for PersonServiceImpl {
    type U = PersonUsecaseImpl;
    type N = DefaultReporter<'a>;

    // service is responsible for transaction management
    fn run_tx<T, F>(&'a mut self, f: F) -> Result<T, ServiceError>
    where
        F: FnOnce(
            &mut PersonUsecaseImpl,
            &mut RefMut<'a, HashMap<PersonId, PersonDto>>,
        ) -> Result<T, UsecaseError>,
    {
        let mut ctx = self.hs_db.persons.borrow_mut();
        trace!("transaction started");

        let mut usecase = self.usecase.borrow_mut();
        let res = f(&mut usecase, &mut ctx);

        match res {
            Ok(v) => {
                trace!("transaction committed");
                Ok(v)
            }
            Err(e) => {
                error!("transaction aborted: {}", e);
                Err(ServiceError::TransactionFailed(e))
            }
        }
    }

    fn get_reporter(&self) -> Self::N {
        self.reporter.clone()
    }
}

impl<'a> PersonCachedService<'a, redis::Connection, RefMut<'a, HashMap<PersonId, PersonDto>>>
    for PersonServiceImpl
{
    type C = redis_cache::RedisPersonCao;

    fn get_cao(&self) -> Self::C {
        redis_cache::RedisPersonCao::new(self.cache_client.clone(), Duration::from_secs(2))
    }
}

// a crude presenter
#[cfg_attr(any(feature = "use_pq", feature = "use_dynamo"), allow(unused))]
pub struct PersonBatchImportPresenterImpl;
impl PersonOutputBoundary<(u64, u64), ServiceError> for PersonBatchImportPresenterImpl {
    fn started(&self) {
        println!("service started");
    }
    fn in_progress(&self, progress: (u64, u64)) {
        println!("{} of {} done", progress.0, progress.1);
    }
    fn completed(&self) {
        println!("service completed");
    }
    fn aborted(&self, err: ServiceError) {
        println!("service aborted: {}", err);
    }
}
