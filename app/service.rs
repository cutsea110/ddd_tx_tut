use std::cell::RefMut;
use thiserror::Error;

use crate::domain::{Person, PersonId};
use crate::usecase::{PersonUsecase, UsecaseError};
use tx_rs::Tx;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("transaction failed: {0}")]
    TransactionFailed(UsecaseError),
    #[error("service unavailable: {0}")]
    ServiceUnavailable(UsecaseError),
}
pub trait PersonService<'a, Ctx> {
    type U: PersonUsecase<Ctx>;

    fn run_tx<T, F>(&'a mut self, f: F) -> Result<T, ServiceError>
    where
        F: FnOnce(&mut RefMut<'_, Self::U>, &mut Ctx) -> Result<T, UsecaseError>;

    fn register(
        &'a mut self,
        name: &str,
        age: i32,
        data: &str,
    ) -> Result<(PersonId, Person), ServiceError> {
        self.run_tx(move |usecase, ctx| {
            usecase
                .entry_and_verify(Person::new(name, age, Some(data)))
                .run(ctx)
        })
    }

    fn batch_import(&'a mut self, persons: Vec<Person>) -> Result<(), ServiceError> {
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

    fn list_all(&'a mut self) -> Result<Vec<(PersonId, Person)>, ServiceError> {
        self.run_tx(move |usecase, ctx| usecase.collect().run(ctx))
    }
}
