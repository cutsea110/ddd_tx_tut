use log::trace;
use std::{cell::RefCell, process, rc::Rc, time::Duration};

use crate::cached_service::PersonCachedService;
use crate::dao::{self, HavePersonDao};
use crate::dynamodb::DynamoDbPersonDao;
use crate::rabbitmq;
use crate::redis_cache;
use crate::reporter::{DefaultReporter, Reporter};
use crate::service::{PersonOutputBoundary, PersonService, ServiceError};
use crate::usecase::{PersonUsecase, UsecaseError};

#[derive(Debug, Clone)]
pub struct PersonUsecaseImpl {
    dao: DynamoDbPersonDao,
}
impl PersonUsecaseImpl {
    #[cfg_attr(any(feature = "use_pq", feature = "use_hash"), allow(unused))]
    pub fn new(dao: DynamoDbPersonDao) -> Self {
        Self { dao }
    }
}
impl PersonUsecase<Rc<tokio::runtime::Runtime>> for PersonUsecaseImpl {}

impl HavePersonDao<Rc<tokio::runtime::Runtime>> for PersonUsecaseImpl {
    fn get_dao(&self) -> &impl dao::PersonDao<Rc<tokio::runtime::Runtime>> {
        &self.dao
    }
}

pub struct PersonServiceImpl {
    runtime: Rc<tokio::runtime::Runtime>,
    cache_client: redis::Client,
    reporter: DefaultReporter<'static>,
    usecase: RefCell<PersonUsecaseImpl>,
}
impl PersonServiceImpl {
    #[cfg_attr(any(feature = "use_pq", feature = "use_hash"), allow(unused))]
    pub fn new(
        runtime: Rc<tokio::runtime::Runtime>,
        dynamo_uri: &str,
        cache_uri: &str,
        mq_uri: &str,
    ) -> Self {
        let pid = process::id();
        trace!("pid: {}", pid);
        let cache_client = redis::Client::open(cache_uri).expect("create cache client");
        let mq_client = rabbitmq::Client::open(runtime.clone(), mq_uri).expect("create mq client");
        let syslog_client =
            crate::syslog::Client::new("ddd_tx_tut", pid).expect("crate syslog client");
        let mut reporter = DefaultReporter::new();
        reporter
            .register(mq_client)
            .expect("register observer: rabbitmq");
        reporter
            .register(syslog_client)
            .expect("register observer: syslog");

        let usecase = RefCell::new(PersonUsecaseImpl::new(DynamoDbPersonDao::new(
            runtime.clone(),
            dynamo_uri,
        )));

        Self {
            runtime,
            cache_client,
            reporter,
            usecase,
        }
    }
}
impl<'a> PersonService<'a, Rc<tokio::runtime::Runtime>> for PersonServiceImpl {
    type U = PersonUsecaseImpl;
    type N = DefaultReporter<'a>;

    fn run_tx<T, F>(&'a mut self, f: F) -> Result<T, ServiceError>
    where
        F: FnOnce(&mut Self::U, &mut Rc<tokio::runtime::Runtime>) -> Result<T, UsecaseError>,
    {
        let mut usecase = self.usecase.borrow_mut();
        let res = f(&mut usecase, &mut self.runtime.clone());

        match res {
            Ok(v) => {
                trace!("transaction succeeded");
                Ok(v)
            }
            Err(e) => {
                trace!("transaction aborted: {:?}", e);
                Err(ServiceError::TransactionFailed(e))
            }
        }
    }

    fn get_reporter(&self) -> Self::N {
        self.reporter.clone()
    }
}
impl<'a> PersonCachedService<'a, redis::Connection, Rc<tokio::runtime::Runtime>>
    for PersonServiceImpl
{
    type C = redis_cache::RedisPersonCao;

    fn get_cao(&self) -> Self::C {
        redis_cache::RedisPersonCao::new(self.cache_client.clone(), Duration::from_secs(2))
    }
}

// a crude presenter
#[cfg_attr(any(feature = "use_pq", feature = "use_hash"), allow(unused))]
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
