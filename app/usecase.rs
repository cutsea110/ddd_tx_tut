use thiserror::Error;

use crate::dao::{DaoError, HavePersonDao, PersonDao};
use crate::domain::{Person, PersonId};
use tx_rs::Tx;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("entry person failed: {0}")]
    EntryPersonFailed(DaoError),
    #[error("find person failed: {0}")]
    FindPersonFailed(DaoError),
    #[error("entry and verify failed: {0}")]
    EntryAndVerifyPersonFailed(DaoError),
    #[error("collect person failed: {0}")]
    CollectPersonFailed(DaoError),
}
pub trait PersonUsecase<Ctx>: HavePersonDao<Ctx> {
    fn entry<'a>(
        &'a mut self,
        person: Person,
    ) -> impl tx_rs::Tx<Ctx, Item = PersonId, Err = ServiceError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        dao.insert(person)
            .map_err(|e| ServiceError::EntryPersonFailed(e))
    }
    fn find<'a>(
        &'a mut self,
        id: PersonId,
    ) -> impl tx_rs::Tx<Ctx, Item = Option<Person>, Err = ServiceError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        dao.fetch(id).map_err(|e| ServiceError::FindPersonFailed(e))
    }
    fn entry_and_verify<'a>(
        &'a mut self,
        person: Person,
    ) -> impl tx_rs::Tx<Ctx, Item = (PersonId, Person), Err = ServiceError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        dao.insert(person)
            .and_then(move |id| dao.fetch(id).map(move |p| (id, p.unwrap())))
            .map_err(|e| ServiceError::EntryAndVerifyPersonFailed(e))
    }
    fn collect<'a>(
        &'a mut self,
    ) -> impl tx_rs::Tx<Ctx, Item = Vec<(PersonId, Person)>, Err = ServiceError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        dao.select()
            .map_err(|e| ServiceError::CollectPersonFailed(e))
    }
}
