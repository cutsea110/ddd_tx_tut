use chrono::NaiveDate;
use log::trace;
use std::fmt;
use thiserror::Error;

use crate::domain::{Person, PersonId};
use crate::notifier::Notifier;
use crate::usecase::{PersonUsecase, UsecaseError};
use tx_rs::Tx;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ServiceError {
    #[error("transaction failed: {0}")]
    TransactionFailed(UsecaseError),
    #[error("service unavailable: {0}")]
    ServiceUnavailable(String),
    #[error("invalid request: {0}")]
    InvalidRequest(InvalidErrorKind),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidErrorKind {
    EmptyArgument,
}
impl fmt::Display for InvalidErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InvalidErrorKind::EmptyArgument => write!(f, "empty argument"),
        }
    }
}

pub trait PersonService<'a, Ctx> {
    type U: PersonUsecase<Ctx>;
    type N: Notifier;

    fn run_tx<T, F>(&'a mut self, f: F) -> Result<T, ServiceError>
    where
        F: FnOnce(&mut Self::U, &mut Ctx) -> Result<T, UsecaseError>;

    fn get_notifier(&self) -> Self::N;

    fn register(
        &'a mut self,
        name: &str,
        birth_date: NaiveDate,
        death_date: Option<NaiveDate>,
        data: &str,
    ) -> Result<(PersonId, Person), ServiceError> {
        trace!(
            "register person: name={}, birth_date={}, death_date={:?}, data={}",
            name,
            birth_date,
            death_date,
            data
        );
        self.run_tx(move |usecase, ctx| {
            usecase
                .entry_and_verify(Person::new(name, birth_date, death_date, Some(data)))
                .run(ctx)
        })
    }

    fn find(&'a mut self, id: PersonId) -> Result<Option<Person>, ServiceError> {
        trace!("find person: id={}", id);
        self.run_tx(move |usecase, ctx| usecase.find(id).run(ctx))
    }

    fn batch_import(&'a mut self, persons: Vec<Person>) -> Result<Vec<PersonId>, ServiceError> {
        trace!("batch import persons: {:?}", persons);
        let mut ids = vec![];
        self.run_tx(move |usecase, ctx| {
            for person in persons {
                let res = usecase.entry(person).run(ctx);
                match res {
                    Ok(id) => ids.push(id),
                    Err(e) => return Err(e),
                }
            }
            Ok(ids)
        })
    }

    fn list_all(&'a mut self) -> Result<Vec<(PersonId, Person)>, ServiceError> {
        trace!("list all persons");
        self.run_tx(move |usecase, ctx| usecase.collect().run(ctx))
    }

    fn unregister(&'a mut self, id: PersonId) -> Result<(), ServiceError> {
        trace!("unregister person: id={}", id);
        self.run_tx(move |usecase, ctx| usecase.remove(id).run(ctx))
    }
}

// # フェイクテスト
//
// * 目的
//
//   Service の正常系のテストを行う
//   Service の各メソッドが Usecase から通常期待される結果を受け取ったときに適切にふるまうことを保障する
//
// * 方針
//
//   Usecase のフェイクに対して Service を実行し、その結果を確認する
//   フェイクはテスト時の比較チェックのしやすさを考慮して HashMap ではなく Vec で登録データを保持する
//   データ数は多くないので、Vec でリニアサーチしても十分な速度が出ると考える
//
// * 実装
//
//                          Test Double
//        +---------+      +----------------+.oOo.+-----------+
//        | Service |      | Fake Usecase   |     | Dummy DAO |
//        | ======= |      | ============== |     | ========= |
//        |         |      |                |     |           |
//   --c->| ---c--> |----->| ---+           |     |           |
//     |  |    |    |      |    | fake logic|     |           |
//   <-c--| <--c--- |<-----| <--+           |     |           |
//     |  +----|----+      +----------------+	  +-----------+
//     |       |
//     |       +-- テスト対象
//     |
//     +-- ここを確認する
//
//   1. ダミーの DAO 構造体を用意する
//      この構造体は実質使われないが、 Usecase の構成で必要になるため用意する
//   2. Usecase のメソッド呼び出しに対して、期待される結果を返す Usecase 構造体を用意する
//      この Usecase 構造体はフェイクなので、間接的な入力と間接的な出力が整合するようにする
//   3. Usecase にダミーの DAO 構造体をプラグインする
//   4. Service にこのフェイク Usecase をプラグインする
//   5. Service のメソッドを呼び出す
//   6. Service からの戻り値を検証する
//
// * 注意
//
//   1. このテストは Service の実装を保障するものであって、Usecase の実装を保障するものではない
//   2. 同様にこのテストは DAO の実装を保障するものではない
//   3. Service が返すエラー値は tx_run の実装に依存している
//      したがってテストとして意味があるのは UsecaseError までである
//
#[cfg(test)]
mod fake_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::{
        dao::{DaoError, PersonDao},
        domain::date,
        notifier::NotifierError,
        HavePersonDao,
    };

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

    struct FakePersonUsecase {
        db: Vec<(PersonId, Person)>,
        dao: DummyPersonDao,
    }
    impl HavePersonDao<()> for FakePersonUsecase {
        fn get_dao<'b>(&'b self) -> Box<&impl PersonDao<()>> {
            Box::new(&self.dao)
        }
    }
    impl PersonUsecase<()> for FakePersonUsecase {
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
        fn remove<'a>(
            &'a mut self,
            id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = (), Err = UsecaseError>
        where
            (): 'a,
        {
            self.db.retain(|(i, _)| *i != id);

            tx_rs::with_tx(move |&mut ()| Ok(()))
        }
    }

    struct DummyNotifier;
    impl Notifier for DummyNotifier {
        fn notify(&self, _to: &str, _message: &str) -> Result<(), NotifierError> {
            Ok(())
        }
    }

    struct TargetPersonService {
        usecase: Rc<RefCell<FakePersonUsecase>>,
    }
    impl PersonService<'_, ()> for TargetPersonService {
        type U = FakePersonUsecase;
        type N = DummyNotifier;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, ServiceError>
        where
            F: FnOnce(&mut Self::U, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(ServiceError::TransactionFailed)
        }

        fn get_notifier(&self) -> Self::N {
            DummyNotifier
        }
    }

    #[test]
    fn test_register() {
        let usecase = Rc::new(RefCell::new(FakePersonUsecase {
            db: vec![],
            dao: DummyPersonDao,
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };
        let expected_id = 1;
        let expected = Person::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"));

        let res = service.register("Alice", date(2012, 11, 2), None, "Alice is sender");
        assert_eq!(res, Ok((expected_id, expected)));
    }

    #[test]
    fn test_batch_import() {
        let usecase = Rc::new(RefCell::new(FakePersonUsecase {
            db: vec![],
            dao: DummyPersonDao,
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };
        let persons = vec![
            Person::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
            Person::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
            Person::new("Eve", date(1996, 12, 15), None, Some("Eve is interceptor")),
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
        let usecase = Rc::new(RefCell::new(FakePersonUsecase {
            db: vec![
                (
                    1,
                    Person::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
                ),
                (
                    2,
                    Person::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
                ),
                (
                    3,
                    Person::new("Eve", date(1996, 12, 15), None, Some("Eve is interceptor")),
                ),
            ],
            dao: DummyPersonDao,
        }));
        let mut service = TargetPersonService {
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
    #[test]
    fn test_unregister() {
        let usecase = Rc::new(RefCell::new(FakePersonUsecase {
            db: vec![
                (
                    1,
                    Person::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
                ),
                (
                    2,
                    Person::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
                ),
                (
                    3,
                    Person::new("Eve", date(1996, 12, 15), None, Some("Eve is interceptor")),
                ),
            ],
            dao: DummyPersonDao,
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };

        let _ = service.unregister(2);
        let expected = vec![
            (
                1,
                Person::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
            ),
            (
                3,
                Person::new("Eve", date(1996, 12, 15), None, Some("Eve is interceptor")),
            ),
        ];

        assert_eq!(usecase.borrow().db, expected);
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
//   スパイ Usecase はメソッドが呼び出されるたびに、それらを全て記録する
//   各メソッドの呼び出された記録をテストの最後で確認する
//
// * 実装
//
//                          Test Double
//        +---------+      +------------------+.oOo.+------------+
//        | Service |      | Spy Usecase      |     | Dummy DAO  |
//        | ======= |      | ============     |     | ========== |
//        |         |      |                  |     |            |
//   ---->| ------> |--c-->| --> [ c ] request|     |            |
//        |         |  |   |       |    log   |     |            |
//   <----| <------ |<-|---| <--   |          |     |            |
//        +---------+  |   +-------|----------+     +------------+
//                     |           |
//                     |           +-- ここを確認する
//                     |
//                     +-- テスト対象
//
//   1. ダミーの DAO 構造体を用意する
//      この構造体は実質使われないが、 Usecase の構成で必要になるため用意する
//   2. Usecase のメソッド呼び出しを記録する Usecase 構造体を用意する
//      この構造体はスパイなので、Service の間接的な出力のみを記録する
//   3. その構造体を Service にプラグインする
//   4. Service のメソッドを呼び出す
//   5. その後で Usecase 構造体の記録を検証する
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

    use super::*;
    use crate::{
        dao::{DaoError, PersonDao},
        domain::date,
        notifier::NotifierError,
        HavePersonDao,
    };

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

    struct SpyPersonUsecase {
        dao: DummyPersonDao,
        entry: RefCell<Vec<Person>>,
        find: RefCell<Vec<PersonId>>,
        entry_and_verify: RefCell<Vec<Person>>,
        collect: RefCell<i32>,
        remove: RefCell<Vec<PersonId>>,
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
        fn remove<'a>(
            &'a mut self,
            id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = (), Err = UsecaseError>
        where
            (): 'a,
        {
            self.remove.borrow_mut().push(id);

            // 返り値に意味はない
            tx_rs::with_tx(move |&mut ()| Ok(()))
        }
    }

    struct DummyNotifier;
    impl Notifier for DummyNotifier {
        fn notify(&self, _to: &str, _message: &str) -> Result<(), NotifierError> {
            Ok(())
        }
    }

    struct TargetPersonService {
        usecase: Rc<RefCell<SpyPersonUsecase>>,
    }
    impl PersonService<'_, ()> for TargetPersonService {
        type U = SpyPersonUsecase;
        type N = DummyNotifier;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, ServiceError>
        where
            F: FnOnce(&mut Self::U, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(ServiceError::TransactionFailed)
        }

        fn get_notifier(&self) -> Self::N {
            DummyNotifier
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
            remove: RefCell::new(vec![]),
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };

        let expected = Person::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"));

        let _ = service.register("Alice", date(2012, 11, 2), None, "Alice is sender");

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 0);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 1);
        assert_eq!(*usecase.borrow().collect.borrow(), 0);
        assert_eq!(usecase.borrow().remove.borrow().len(), 0);

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
            remove: RefCell::new(vec![]),
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };

        let persons = vec![
            Person::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
            Person::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
            Person::new("Eve", date(1996, 12, 15), None, Some("Eve is interseptor")),
        ];
        let expected = persons.clone();

        let _ = service.batch_import(persons);

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 3);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 0);
        assert_eq!(*usecase.borrow().collect.borrow(), 0);
        assert_eq!(usecase.borrow().remove.borrow().len(), 0);

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
            remove: RefCell::new(vec![]),
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };

        let _ = service.list_all();

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 0);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 0);
        assert_eq!(*usecase.borrow().collect.borrow(), 1);
        assert_eq!(usecase.borrow().remove.borrow().len(), 0);
    }
    #[test]
    fn test_unregister() {
        let usecase = Rc::new(RefCell::new(SpyPersonUsecase {
            dao: DummyPersonDao,
            entry: RefCell::new(vec![]),
            find: RefCell::new(vec![]),
            entry_and_verify: RefCell::new(vec![]),
            collect: RefCell::new(0),
            remove: RefCell::new(vec![]),
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };

        let _ = service.unregister(42);

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 0);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 0);
        assert_eq!(*usecase.borrow().collect.borrow(), 0);
        assert_eq!(usecase.borrow().remove.borrow().len(), 1);

        // Service の引数が Usecase にそのまま渡されていることを検証
        assert_eq!(usecase.borrow().remove.borrow()[0], 42);
    }
}

// # エラー系スタブテスト
//
// * 目的
//
//   Usecase がエラーを返した場合の Service の挙動を保障する
//
// * 方針
//
//   スタブ Usecase はメソッドが呼び出されると、事前に設定された任意のエラー値を返す
//   Service のメソッドを呼び出して Usecase からエラーを受け取ったときの Service の返り値を確認する
//
// * 実装
//
//   1. ダミーの DAO 構造体を用意する
//      この構造体は実質使われないが、 Usecase の構成で必要になるため用意する
//   2. Usecase のメソッドが任意の結果を返せる種類の Usecase 構造体を用意する
//      この Usecase 構造体はスタブであり、Service への間接的な入力のみ制御する
//   3. その構造体を Service にプラグインする
//   4. Service のメソッドを呼び出す
//   5. Service のメソッドからの戻り値を確認する
//
// * 注意
//
#[cfg(test)]
mod error_stub_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::{
        dao::{DaoError, PersonDao},
        domain::date,
        notifier::NotifierError,
        HavePersonDao,
    };

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

    struct StubPersonUsecase {
        dao: DummyPersonDao,
        entry_result: Result<PersonId, UsecaseError>,
        find_result: Result<Option<Person>, UsecaseError>,
        entry_and_verify_result: Result<(PersonId, Person), UsecaseError>,
        collect_result: Result<Vec<(PersonId, Person)>, UsecaseError>,
        remove_result: Result<(), UsecaseError>,
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
        fn remove<'a>(
            &'a mut self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = (), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(|&mut ()| self.remove_result.clone())
        }
    }

    struct DummyNotifier;
    impl Notifier for DummyNotifier {
        fn notify(&self, _to: &str, _message: &str) -> Result<(), NotifierError> {
            Ok(())
        }
    }

    struct TargetPersonService {
        usecase: Rc<RefCell<StubPersonUsecase>>,
    }
    impl PersonService<'_, ()> for TargetPersonService {
        type U = StubPersonUsecase;
        type N = DummyNotifier;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, ServiceError>
        where
            F: FnOnce(&mut Self::U, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(ServiceError::TransactionFailed)
        }

        fn get_notifier(&self) -> Self::N {
            DummyNotifier
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
            remove_result: Ok(()),      // 使わない
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };

        let result = service.register("Alice", date(2012, 11, 2), None, "Alice is sender");
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
            entry_and_verify_result: Ok((42, Person::new("Alice", date(2012, 11, 2), None, None))), // 使わない
            collect_result: Ok(vec![]), // 使わない
            remove_result: Ok(()),      // 使わない
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };

        let result = service.batch_import(vec![
            Person::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
            Person::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
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
            entry_and_verify_result: Ok((42, Person::new("Alice", date(2012, 11, 2), None, None))), // 使わない
            collect_result: Err(UsecaseError::CollectPersonFailed(DaoError::SelectError(
                "valid dao".to_string(),
            ))),
            remove_result: Ok(()), // 使わない
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };

        let result = service.list_all();
        let expected = usecase.borrow().collect_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }

    #[test]
    fn test_unregister() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((42, Person::new("Alice", date(2012, 11, 2), None, None))), // 使わない
            collect_result: Ok(vec![]), // 使わない
            remove_result: Err(UsecaseError::RemovePersonFailed(DaoError::DeleteError(
                "valid dao".to_string(),
            ))),
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };

        let result = service.unregister(42);
        let expected = usecase.borrow().remove_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }
}
