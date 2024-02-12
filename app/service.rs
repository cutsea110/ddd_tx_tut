use std::cell::RefMut;
use thiserror::Error;

use crate::domain::{Person, PersonId};
use crate::usecase::{PersonUsecase, UsecaseError};
use tx_rs::Tx;

#[derive(Debug, Error, PartialEq, Eq)]
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

#[cfg(test)]
mod mock {
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::{
        dao::{DaoError, PersonDao},
        HavePersonDao,
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
    }

    struct MockPersonUsecase {
        db: Vec<(PersonId, Person)>,
        dao: DummyPersonDao,
    }
    impl HavePersonDao<()> for MockPersonUsecase {
        fn get_dao<'b>(&'b self) -> Box<&impl PersonDao<()>> {
            Box::new(&self.dao)
        }
    }
    impl PersonUsecase<()> for MockPersonUsecase {
        fn entry<'a>(
            &'a mut self,
            person: Person,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = UsecaseError>
        where
            (): 'a,
        {
            let next_id = self.db.len() as i32 + 1;
            self.db.push((next_id, person));

            tx_rs::with_tx(move |&mut ()| Ok(next_id))
        }
        fn find<'a>(
            &'a mut self,
            id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<Person>, Err = UsecaseError>
        where
            (): 'a,
        {
            let result = self
                .db
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, p)| p.clone());

            tx_rs::with_tx(move |&mut ()| Ok(result))
        }
        fn entry_and_verify<'a>(
            &'a mut self,
            person: Person,
        ) -> impl tx_rs::Tx<(), Item = (PersonId, Person), Err = UsecaseError>
        where
            (): 'a,
        {
            let next_id = self.db.len() as i32 + 1;
            self.db.push((next_id, person.clone()));

            tx_rs::with_tx(move |&mut ()| Ok((next_id, person)))
        }
        fn collect<'a>(
            &'a mut self,
        ) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, Person)>, Err = UsecaseError>
        where
            (): 'a,
        {
            let result = self.db.clone();

            tx_rs::with_tx(move |&mut ()| Ok(result))
        }
    }

    struct MockPersonService {
        usecase: Rc<RefCell<MockPersonUsecase>>,
    }
    impl PersonService<'_, ()> for MockPersonService {
        type U = MockPersonUsecase;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, ServiceError>
        where
            F: FnOnce(&mut RefMut<'_, Self::U>, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(ServiceError::TransactionFailed)
        }
    }

    #[test]
    fn test_register() {
        let usecase = Rc::new(RefCell::new(MockPersonUsecase {
            db: vec![],
            dao: DummyPersonDao,
        }));
        let mut service = MockPersonService {
            usecase: usecase.clone(),
        };
        let expected_id = 1;
        let expected = Person::new("Alice", 20, Some("Alice is sender"));

        let res = service.register("Alice", 20, "Alice is sender");
        assert_eq!(res, Ok((expected_id, expected)));
    }

    #[test]
    fn test_batch_import() {
        let usecase = Rc::new(RefCell::new(MockPersonUsecase {
            db: vec![],
            dao: DummyPersonDao,
        }));
        let mut service = MockPersonService {
            usecase: usecase.clone(),
        };
        let persons = vec![
            Person::new("Alice", 20, Some("Alice is sender")),
            Person::new("Bob", 24, Some("Bob is receiver")),
            Person::new("Eve", 10, Some("Eve is interceptor")),
        ];
        let expected = persons.clone();

        let _ = service.batch_import(persons);
        assert_eq!(
            usecase
                .borrow()
                .db
                .iter()
                .map(|(_, p)| p.clone())
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn test_list_all() {
        let usecase = Rc::new(RefCell::new(MockPersonUsecase {
            db: vec![
                (1, Person::new("Alice", 20, Some("Alice is sender"))),
                (2, Person::new("Bob", 24, Some("Bob is receiver"))),
                (3, Person::new("Eve", 10, Some("Eve is interceptor"))),
            ],
            dao: DummyPersonDao,
        }));
        let mut service = MockPersonService {
            usecase: usecase.clone(),
        };

        let result = service.list_all();
        let expected = usecase
            .borrow()
            .db
            .iter()
            .map(|(id, p)| (id.clone(), p.clone()))
            .collect::<Vec<_>>();

        assert_eq!(result, Ok(expected))
    }
}
