use log::{error, trace};
use postgres::NoTls;
use std::cell::RefCell;
use std::process;
use std::rc::Rc;
use std::time::Duration;

use crate::cached_service::PersonCachedService;
use crate::dao::{self, HavePersonDao};
use crate::pg_db::PgPersonDao;
use crate::rabbitmq;
use crate::redis_cache;
use crate::reporter::{DefaultReporter, Reporter};
use crate::service::{PersonOutputBoundary, PersonService, ServiceError};
use crate::usecase::{PersonUsecase, UsecaseError};

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
    fn get_dao(&self) -> &impl dao::PersonDao<postgres::Transaction<'a>> {
        &self.dao
    }
}

pub struct PersonServiceImpl {
    db_client: postgres::Client,
    cache_client: redis::Client,
    reporter: DefaultReporter<'static>,
    usecase: RefCell<PersonUsecaseImpl>,
}
impl PersonServiceImpl {
    pub fn new(
        runtime: Rc<tokio::runtime::Runtime>,
        db_uri: &str,
        cache_uri: &str,
        mq_uri: &str,
    ) -> Self {
        let pid = process::id();
        trace!("pid: {}", pid);
        let db_client = postgres::Client::connect(db_uri, NoTls).expect("create db client");
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

        let usecase = RefCell::new(PersonUsecaseImpl::new(PgPersonDao));

        Self {
            db_client,
            cache_client,
            reporter,
            usecase,
        }
    }
}
impl<'a> PersonService<'a, postgres::Transaction<'a>> for PersonServiceImpl {
    type U = PersonUsecaseImpl;
    type N = DefaultReporter<'a>;

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

    fn get_reporter(&self) -> Self::N {
        self.reporter.clone()
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

// a crude presenter
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
