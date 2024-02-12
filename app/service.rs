use std::cell::RefMut;
use thiserror::Error;

use crate::domain::{Person, PersonId};
use crate::usecase::{PersonUsecase, UsecaseError};
use tx_rs::Tx;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
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

// # モックテスト
//
// * 目的
//
//   Service の正常系のテストを行う
//   Service の各メソッドが Usecase から通常期待される結果を受け取ったときに適切にふるまうことを保障する
//
// * 方針
//
//   Usecase のモックに対して Service を実行し、その結果を確認する
//   モックはテスト時の比較チェックのしやすさを考慮して HashMap ではなく Vec で登録データを保持する
//   データ数は多くないので、Vec でリニアサーチしても十分な速度が出ると考える
//
// * 実装
//
//   1. ダミーの DAO 構造体を用意する
//      この構造体は実質使われないが、 Usecase の構成で必要になるため用意する
//   2. Usecase のメソッド呼び出しに対して、期待される結果を返す Usecase 構造体を用意する
//      この Usecase 構造体はモックなので、間接的な入力と間接的な出力が整合するようにする
//   3. Usecase にダミーの DAO 構造体をプラグインする
//   4. Service にこのモック Usecase をプラグインする
//   3. Service のメソッドを呼び出す
//   4. Service からの戻り値を検証する
//
// * 注意
//
//   1. このテストは Service の実装を保障するものであって、Usecase の実装を保障するものではない
//   2. 同様にこのテストは DAO の実装を保障するものではない
//
#[cfg(test)]
mod mock_tests {
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

// # スパイテスト
//
// * 目的
//
//   Service の各メソッドが Usecase のメソッドを適切に呼び出していることを保障する
//   つまり、
//    1. 必要なメソッドを必要回数だけ呼び出していること
//    2. 不必要なメソッドを呼び出していないこと
//    3. Service に渡った引数が適切に Usecase のメソッドに渡されていること
//   を保障する
//
// * 方針
//
//   Usecase のメソッドを呼び出すたびに、その呼び出しを記録する
//   その記録をテストの最後で確認する
//
// * 実装
//
//   1. Usecase のメソッド呼び出しを記録する種類の Usecase 構造体を用意する
//      この構造体はスパイなので、Service の間接的な出力のみを記録する
//   2. その構造体を Service にプラグインする
//   3. Service のメソッドを呼び出す
//   4. その後で Usecase 構造体の記録を検証する
//
// * 注意
//
//   1. このテストは Service の実装を保障するものであって、Usecase の実装を保障するものではない
//   2. このテストは Service のメソッドが適切に Usecase のメソッドを呼び出していることを保障するものであって、
//      Usecase のメソッドが適切な処理を行っていることを保障するものではない
//   3. このテストは Service のメソッドが不適切な Usecase のメソッド呼び出しをしていないことを保障するものであって、
//      Usecase のメソッドが不適切な処理をしていないことを保障するものではない
//   4. このテストでは Usecase のメソッドの呼び出し順序については検証しない (将来的には検証することは拒否しない)
#[cfg(test)]
mod spy_tests {
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

    struct SpyPersonUsecase {
        dao: DummyPersonDao,
        entry: RefCell<Vec<Person>>,
        find: RefCell<Vec<PersonId>>,
        entry_and_verify: RefCell<Vec<Person>>,
        collect: RefCell<i32>,
    }
    impl HavePersonDao<()> for SpyPersonUsecase {
        fn get_dao<'b>(&'b self) -> Box<&impl PersonDao<()>> {
            Box::new(&self.dao)
        }
    }
    impl PersonUsecase<()> for SpyPersonUsecase {
        fn entry<'a>(
            &'a mut self,
            person: Person,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = UsecaseError>
        where
            (): 'a,
        {
            self.entry.borrow_mut().push(person);

            // 返り値に意味はない
            tx_rs::with_tx(|&mut ()| Ok(42 as PersonId))
        }
        fn find<'a>(
            &'a mut self,
            id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<Person>, Err = UsecaseError>
        where
            (): 'a,
        {
            self.find.borrow_mut().push(id);

            // 返り値に意味はない
            tx_rs::with_tx(|&mut ()| Ok(None))
        }
        fn entry_and_verify<'a>(
            &'a mut self,
            person: Person,
        ) -> impl tx_rs::Tx<(), Item = (PersonId, Person), Err = UsecaseError>
        where
            (): 'a,
        {
            self.entry_and_verify.borrow_mut().push(person.clone());

            // 返り値に意味はない
            tx_rs::with_tx(move |&mut ()| Ok((42, person)))
        }
        fn collect<'a>(
            &'a mut self,
        ) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, Person)>, Err = UsecaseError>
        where
            (): 'a,
        {
            *self.collect.borrow_mut() += 1;

            // 返り値に意味はない
            tx_rs::with_tx(|&mut ()| Ok(vec![]))
        }
    }

    struct SpyPersonService {
        usecase: Rc<RefCell<SpyPersonUsecase>>,
    }
    impl PersonService<'_, ()> for SpyPersonService {
        type U = SpyPersonUsecase;

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
        let usecase = Rc::new(RefCell::new(SpyPersonUsecase {
            dao: DummyPersonDao,
            entry: RefCell::new(vec![]),
            find: RefCell::new(vec![]),
            entry_and_verify: RefCell::new(vec![]),
            collect: RefCell::new(0),
        }));
        let mut service = SpyPersonService {
            usecase: usecase.clone(),
        };

        let expected = Person::new("Alice", 20, Some("Alice is sender"));

        let _ = service.register("Alice", 20, "Alice is sender");

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 0);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 1);
        assert_eq!(*usecase.borrow().collect.borrow(), 0);

        // Service の引数が Usecase にそのまま渡されていることを検証
        assert_eq!(usecase.borrow().entry_and_verify.borrow()[0], expected);
    }

    #[test]
    fn test_batch_import() {
        let usecase = Rc::new(RefCell::new(SpyPersonUsecase {
            dao: DummyPersonDao,
            entry: RefCell::new(vec![]),
            find: RefCell::new(vec![]),
            entry_and_verify: RefCell::new(vec![]),
            collect: RefCell::new(0),
        }));
        let mut service = SpyPersonService {
            usecase: usecase.clone(),
        };

        let persons = vec![
            Person::new("Alice", 20, Some("Alice is sender")),
            Person::new("Bob", 25, Some("Bob is receiver")),
            Person::new("Eve", 10, Some("Eve is interseptor")),
        ];
        let expected = persons.clone();

        let _ = service.batch_import(persons);

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 3);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 0);
        assert_eq!(*usecase.borrow().collect.borrow(), 0);

        // Service の引数が Usecase にそのまま渡されていることを検証
        assert_eq!(usecase.borrow().entry.borrow().clone(), expected);
    }

    #[test]
    fn list_all() {
        let usecase = Rc::new(RefCell::new(SpyPersonUsecase {
            dao: DummyPersonDao,
            entry: RefCell::new(vec![]),
            find: RefCell::new(vec![]),
            entry_and_verify: RefCell::new(vec![]),
            collect: RefCell::new(0),
        }));
        let mut service = SpyPersonService {
            usecase: usecase.clone(),
        };

        let _ = service.list_all();

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 0);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 0);
        assert_eq!(*usecase.borrow().collect.borrow(), 1);
    }
}

#[cfg(test)]
mod error_stub_tests {
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

    struct StubPersonUsecase {
        dao: DummyPersonDao,
        entry_result: Result<PersonId, UsecaseError>,
        find_result: Result<Option<Person>, UsecaseError>,
        entry_and_verify_result: Result<(PersonId, Person), UsecaseError>,
        collect_result: Result<Vec<(PersonId, Person)>, UsecaseError>,
    }
    impl HavePersonDao<()> for StubPersonUsecase {
        fn get_dao<'b>(&'b self) -> Box<&impl PersonDao<()>> {
            Box::new(&self.dao)
        }
    }
    impl PersonUsecase<()> for StubPersonUsecase {
        fn entry<'a>(
            &'a mut self,
            _person: Person,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(|&mut ()| self.entry_result.clone())
        }
        fn find<'a>(
            &'a mut self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<Person>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(|&mut ()| self.find_result.clone())
        }
        fn entry_and_verify<'a>(
            &'a mut self,
            _person: Person,
        ) -> impl tx_rs::Tx<(), Item = (PersonId, Person), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| self.entry_and_verify_result.clone())
        }
        fn collect<'a>(
            &'a mut self,
        ) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, Person)>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(|&mut ()| self.collect_result.clone())
        }
    }

    struct StubPersonService {
        usecase: Rc<RefCell<StubPersonUsecase>>,
    }
    impl PersonService<'_, ()> for StubPersonService {
        type U = StubPersonUsecase;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, ServiceError>
        where
            F: FnOnce(&mut RefMut<'_, Self::U>, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(ServiceError::TransactionFailed)
        }
    }

    #[test]
    fn test_register_entry_and_verify_error() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Err(UsecaseError::EntryAndVerifyPersonFailed(
                DaoError::InsertError("valid dao".to_string()),
            )),
            collect_result: Ok(vec![]), // 使わない
        }));
        let mut service = StubPersonService {
            usecase: usecase.clone(),
        };

        let result = service.register("Alice", 20, "Alice is sender");
        let expected = usecase
            .borrow()
            .entry_and_verify_result
            .clone()
            .unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }

    #[test]
    fn test_batch_import() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Err(UsecaseError::EntryPersonFailed(DaoError::InsertError(
                "valid dao".to_string(),
            ))),
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((42, Person::new("Alice", 20, None))), // 使わない
            collect_result: Ok(vec![]), // 使わない
        }));
        let mut service = StubPersonService {
            usecase: usecase.clone(),
        };

        let result = service.batch_import(vec![
            Person::new("Alice", 20, Some("Alice is sender")),
            Person::new("Bob", 25, Some("Bob is receiver")),
        ]);
        let expected = usecase.borrow().entry_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }

    #[test]
    fn test_list_all() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((42, Person::new("Alice", 20, None))), // 使わない
            collect_result: Err(UsecaseError::CollectPersonFailed(DaoError::SelectError(
                "valid dao".to_string(),
            ))),
        }));
        let mut service = StubPersonService {
            usecase: usecase.clone(),
        };

        let result = service.list_all();
        let expected = usecase.borrow().collect_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }
}
