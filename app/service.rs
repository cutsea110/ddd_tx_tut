use postgres::{Client, NoTls};
use std::cell::RefCell;
use std::cell::RefMut;
use std::rc::Rc;
use thiserror::Error;

use crate::dao::HavePersonDao;
use crate::domain::{Person, PersonId};
use crate::pg_db::PgPersonDao;
use crate::usecase::{PersonUsecase, ServiceError};
use crate::PersonDao;
use tx_rs::Tx;

#[derive(Debug, Clone)]
pub struct PersonUsecaseImpl {
    dao: Rc<PgPersonDao>,
}
impl PersonUsecaseImpl {
    pub fn new(dao: Rc<PgPersonDao>) -> Self {
        Self { dao }
    }
}
impl<'a> HavePersonDao<postgres::Transaction<'a>> for PersonUsecaseImpl {
    fn get_dao<'b>(&'b self) -> Box<&impl PersonDao<postgres::Transaction<'a>>> {
        Box::new(&*self.dao)
    }
}

impl<'a> PersonUsecase<postgres::Transaction<'a>> for PersonUsecaseImpl {}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("transaction failed: {0}")]
    TransactionFailed(ServiceError),
    #[error("service unavailable: {0}")]
    ServiceUnavailable(ServiceError),
}
pub trait Api<'a, Ctx> {
    type U: PersonUsecase<Ctx>;

    fn run_tx<T, F>(&'a mut self, f: F) -> Result<T, ApiError>
    where
        F: FnOnce(&mut RefMut<'_, Self::U>, &mut Ctx) -> Result<T, ServiceError>;

    fn register(
        &'a mut self,
        name: &str,
        age: i32,
        data: &str,
    ) -> Result<(PersonId, Person), ApiError> {
        self.run_tx(move |usecase, ctx| {
            usecase
                .entry_and_verify(Person::new(name, age, Some(data)))
                .run(ctx)
        })
    }

    fn batch_import(&'a mut self, persons: Vec<Person>) -> Result<(), ApiError> {
        self.run_tx(move |usecase, ctx| {
            for person in persons {
                let res = usecase.entry(person).run(ctx);
                if let Err(e) = res {
                    return Err(e);
                }
            }
            Ok(())
        })
    }

    fn list_all(&'a mut self) -> Result<Vec<(PersonId, Person)>, ApiError> {
        self.run_tx(move |usecase, ctx| usecase.collect().run(ctx))
    }
}

pub struct PersonApi {
    db_client: Client,
    usecase: Rc<RefCell<PersonUsecaseImpl>>,
}
impl PersonApi {
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
impl<'a> Api<'a, postgres::Transaction<'a>> for PersonApi {
    type U = PersonUsecaseImpl;

    // api is responsible for transaction management
    fn run_tx<T, F>(&'a mut self, f: F) -> Result<T, ApiError>
    where
        F: FnOnce(
            &mut RefMut<'_, PersonUsecaseImpl>,
            &mut postgres::Transaction<'a>,
        ) -> Result<T, ServiceError>,
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
                Err(ApiError::TransactionFailed(e))
            }
        }
    }
}
