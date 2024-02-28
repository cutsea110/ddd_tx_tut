use chrono::NaiveDate;
use log::trace;

pub use crate::cache::PersonCao;
pub use crate::domain::{Person, PersonId};
pub use crate::service::{PersonService, ServiceError};

pub trait PersonCachedService<'a, Conn, Ctx>: PersonService<'a, Ctx> {
    type C: PersonCao<Conn>;

    fn get_cao(&self) -> Self::C;

    fn cached_register(
        &'a mut self,
        name: &str,
        birth_date: NaiveDate,
        death_date: Option<NaiveDate>,
        data: &str,
    ) -> Result<(PersonId, Person), ServiceError> {
        trace!(
            "cached register: {} {} {:?} {}",
            name,
            birth_date,
            death_date,
            data
        );
        let cao = self.get_cao();

        let result = self.register(name, birth_date, death_date, data);
        trace!("register person to db: {:?}", result);

        if let Ok((id, person)) = &result {
            let _: () = cao
                .run_tx(cao.load(*id, person))
                .map_err(|e| ServiceError::ServiceUnavailable(e.to_string()))?;

            trace!("load person to cache: {}", person);
        }

        result
    }

    fn cached_find(&'a mut self, id: PersonId) -> Result<Option<Person>, ServiceError> {
        trace!("cached find: {}", id);
        let cao = self.get_cao();

        // if the person is found in the cache, return it
        if let Some(p) = cao
            .run_tx(cao.find(id))
            .map_err(|e| ServiceError::ServiceUnavailable(e.to_string()))?
        {
            trace!("cache hit!: {}", id);
            return Ok(Some(p));
        }
        trace!("cache miss!: {}", id);

        let result = self.find(id)?;
        trace!("find person in db: {:?}", result);

        // if the person is found in the db, load it to the cache
        if let Some(person) = &result {
            let _: () = cao
                .run_tx(cao.load(id, person))
                .map_err(|e| ServiceError::ServiceUnavailable(e.to_string()))?;
            trace!("load person to cache: {}", person);
        }

        Ok(result)
    }

    fn cached_batch_import(
        &'a mut self,
        persons: Vec<Person>,
    ) -> Result<Vec<PersonId>, ServiceError> {
        trace!("cached batch import: {:?}", persons);
        let cao = self.get_cao();

        let ids = self.batch_import(persons.clone())?;

        // load all persons to the cache
        ids.iter().zip(persons.iter()).for_each(|(id, person)| {
            let _: () = cao.run_tx(cao.load(*id, person)).expect("load cache");
        });
        trace!("load persons to cache: {:?}", ids);

        Ok(ids)
    }

    fn cached_list_all(&'a mut self) -> Result<Vec<(PersonId, Person)>, ServiceError> {
        trace!("cached list all");
        let cao = self.get_cao();

        let result = self.list_all()?;

        // load all persons to the cache
        result.iter().for_each(|(id, person)| {
            let _: () = cao.run_tx(cao.load(*id, person)).expect("load cache");
        });
        trace!("load all persons to cache");

        Ok(result)
    }

    fn cached_unregister(&'a mut self, id: PersonId) -> Result<(), ServiceError> {
        trace!("cached unregister: {}", id);
        let cao = self.get_cao();

        // even if delete from db failed below, this cache clear is not a matter.
        let _: () = cao
            .run_tx(cao.unload(id))
            .map_err(|e| ServiceError::ServiceUnavailable(e.to_string()))?;
        trace!("unload from cache: {}", id);

        let result = self.unregister(id);
        trace!("delete from db: {}", id);

        result
    }
}

#[cfg(test)]
mod fake_tests {
    use std::cell::{RefCell, RefMut};
    use std::collections::HashMap;
    use std::rc::Rc;

    use crate::{
        dao::{DaoError, PersonDao},
        HavePersonDao, PersonUsecase, UsecaseError,
    };

    use super::*;

    struct DummyPersonDao;
    impl PersonDao<()> for DummyPersonDao {
        fn insert(&self, _person: Person) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(1))
        }
        fn fetch(
            &self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<Person>, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(None))
        }
        fn select(&self) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, Person)>, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(vec![]))
        }
        fn delete(&self, _id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(()))
        }
    }

    struct DummyPersonUsecase {
        dao: DummyPersonDao,
    }
    impl HavePersonDao<()> for DummyPersonUsecase {
        fn get_dao<'b>(&'b self) -> Box<&impl PersonDao<()>> {
            Box::new(&self.dao)
        }
    }
    impl PersonUsecase<()> for DummyPersonUsecase {
        fn entry<'a>(
            &'a mut self,
            _person: Person,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(1))
        }
        fn find<'a>(
            &'a mut self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<Person>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(None))
        }
        fn entry_and_verify<'a>(
            &'a mut self,
            person: Person,
        ) -> impl tx_rs::Tx<(), Item = (PersonId, Person), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok((1, person)))
        }
        fn collect<'a>(
            &'a mut self,
        ) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, Person)>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(vec![]))
        }
        fn remove<'a>(
            &'a mut self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = (), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(()))
        }
    }

    struct TargetPersonService {
        next_id: RefCell<PersonId>,
        db: RefCell<HashMap<PersonId, Person>>,
        usecase: Rc<RefCell<DummyPersonUsecase>>,
        cao: FakePersonCao,
    }
    // フェイクのサービス実装です。ユースケースより先はダミーです。
    impl PersonService<'_, ()> for TargetPersonService {
        type U = DummyPersonUsecase;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, ServiceError>
        where
            F: FnOnce(&mut RefMut<'_, Self::U>, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(ServiceError::TransactionFailed)
        }

        fn register(
            &'_ mut self,
            name: &str,
            birth_date: NaiveDate,
            death_date: Option<NaiveDate>,
            data: &str,
        ) -> Result<(PersonId, Person), ServiceError> {
            let id = *self.next_id.borrow();
            *self.next_id.borrow_mut() += 1;

            let person = Person::new(name, birth_date, death_date, Some(data));

            self.db.borrow_mut().insert(id, person.clone());
            Ok((id, person))
        }

        fn find(&'_ mut self, id: PersonId) -> Result<Option<Person>, ServiceError> {
            Ok(self.db.borrow().get(&id).cloned())
        }

        fn batch_import(&'_ mut self, persons: Vec<Person>) -> Result<Vec<PersonId>, ServiceError> {
            let mut ids = vec![];
            for person in persons {
                let id = *self.next_id.borrow();
                *self.next_id.borrow_mut() += 1;

                self.db.borrow_mut().insert(id, person.clone());
                ids.push(id);
            }
            Ok(ids)
        }

        fn list_all(&'_ mut self) -> Result<Vec<(PersonId, Person)>, ServiceError> {
            Ok(self
                .db
                .borrow()
                .iter()
                .map(|(id, person)| (*id, person.clone()))
                .collect())
        }

        fn unregister(&'_ mut self, id: PersonId) -> Result<(), ServiceError> {
            self.db.borrow_mut().remove(&id);
            Ok(())
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct FakePersonCao {
        cache: RefCell<HashMap<PersonId, Person>>,
    }
    impl PersonCao<()> for FakePersonCao {
        fn get_conn(&self) -> Result<(), crate::CaoError> {
            Ok(())
        }
        fn run_tx<T, F>(&self, f: F) -> Result<T, crate::CaoError>
        where
            F: tx_rs::Tx<(), Item = T, Err = crate::CaoError>,
        {
            f.run(&mut ())
        }
        fn exists(&self, id: PersonId) -> impl tx_rs::Tx<(), Item = bool, Err = crate::CaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(self.cache.borrow().contains_key(&id)))
        }
        fn find(
            &self,
            id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<Person>, Err = crate::CaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(self.cache.borrow().get(&id).cloned()))
        }
        fn load(
            &self,
            id: PersonId,
            person: &Person,
        ) -> impl tx_rs::Tx<(), Item = (), Err = crate::CaoError> {
            tx_rs::with_tx(move |&mut ()| {
                self.cache.borrow_mut().insert(id, person.clone());
                Ok(())
            })
        }
        fn unload(&self, id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = crate::CaoError> {
            tx_rs::with_tx(move |&mut ()| {
                self.cache.borrow_mut().remove(&id);
                Ok(())
            })
        }
    }
    impl PersonCachedService<'_, (), ()> for TargetPersonService {
        type C = FakePersonCao;

        // FIXME: 呼び出しの都度初期化される
        // サービスメソッド毎に一度しか呼ばれないのでテストではあまり問題にはならない
        fn get_cao(&self) -> FakePersonCao {
            self.cao.clone()
        }
    }
}
