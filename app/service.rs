use chrono::NaiveDate;
use log::{error, trace};
use std::fmt;
use std::rc::Rc;
use thiserror::Error;

use crate::domain::PersonId;
use crate::dto::PersonDto;
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

pub trait PersonOutputBoundary<T> {
    fn started(&self);
    fn in_progress(&self, progress: T);
    fn completed(&self);
    fn aborted(&self, err: ServiceError);
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
    ) -> Result<(PersonId, PersonDto), ServiceError> {
        trace!(
            "register person: name={}, birth_date={}, death_date={:?}, data={}",
            name,
            birth_date,
            death_date,
            data
        );
        let notifier = self.get_notifier();

        self.run_tx(move |usecase, ctx| {
            usecase
                .entry_and_verify(PersonDto::new(name, birth_date, death_date, Some(data)))
                .run(ctx)
        })
        .and_then(|(id, p)| {
            let msg = format!(r#"{{ "person_id": {} }}"#, id);
            if let Err(e) = notifier.notify("entry_person", &msg) {
                error!("notification service not available: {}", e);
            }
            return Ok((id, p));
        })
        .map_err(|e| {
            let msg = format!(
                "cannot register person: name={}, birth_date={}, death_date={:?}, data={}",
                name, birth_date, death_date, data
            );
            if let Err(e) = notifier.notify("admin", &msg) {
                error!("notification service not available: {}", e);
            }
            return e;
        })
    }

    fn find(&'a mut self, id: PersonId) -> Result<Option<PersonDto>, ServiceError> {
        trace!("find person: id={}", id);
        let notifier = self.get_notifier();

        self.run_tx(move |usecase, ctx| usecase.find(id).run(ctx))
            .map_err(|e| {
                let msg = format!("cannot find person: id={}", id);
                if let Err(e) = notifier.notify("admin", &msg) {
                    error!("notification service not available: {}", e);
                }
                return e;
            })
    }

    fn batch_import(
        &'a mut self,
        persons: Vec<PersonDto>,
        out_port: Rc<impl PersonOutputBoundary<(u64, u64)>>,
    ) -> Result<Vec<PersonId>, ServiceError> {
        trace!("batch import persons: {:?}", persons);
        out_port.started();
        let notifier = self.get_notifier();

        let mut ids = vec![];
        let total = persons.len() as u64;
        self.run_tx(move |usecase, ctx| {
            for person in persons {
                let res = usecase.entry(person).run(ctx);
                match res {
                    Ok(id) => {
                        ids.push(id);

                        let msg = format!(r#"{{ "person_id": {} }}"#, id);
                        if let Err(e) = notifier.notify("entry_person", &msg) {
                            error!("notification service not available: {}", e);
                        }
                    }
                    Err(e) => {
                        out_port.aborted(ServiceError::TransactionFailed(e.clone()));

                        let msg = format!("cannot entry person: {:?}", e);
                        if let Err(e) = notifier.notify("admin", &msg) {
                            error!("notification service not available: {}", e);
                        }
                        return Err(e);
                    }
                }
                out_port.in_progress((total, ids.len() as u64));
            }
            out_port.completed();
            Ok(ids)
        })
    }

    fn list_all(&'a mut self) -> Result<Vec<(PersonId, PersonDto)>, ServiceError> {
        trace!("list all persons");
        let notifier = self.get_notifier();

        self.run_tx(move |usecase, ctx| usecase.collect().run(ctx))
            .map_err(|e| {
                if let Err(e) = notifier.notify("admin", "cannot list all persons") {
                    error!("notification service not available: {}", e);
                }
                return e;
            })
    }

    fn death(&'a mut self, id: PersonId, death_date: NaiveDate) -> Result<(), ServiceError> {
        trace!("death person: id={}, death_date={}", id, death_date);
        let notifier = self.get_notifier();

        self.run_tx(move |usecase, ctx| usecase.death(id, death_date).run(ctx))
            .and_then(|_| {
                let msg = format!(r#"{{ "person_id": {}, "death_date": {} }}"#, id, death_date);
                if let Err(e) = notifier.notify("death_person", &msg) {
                    error!("notification service not available: {}", e);
                }
                return Ok(());
            })
            .map_err(|e| {
                let msg = format!("cannot death person: id={}, death_date={}", id, death_date);
                if let Err(e) = notifier.notify("admin", &msg) {
                    error!("notification service not available: {}", e);
                }
                return e;
            })
    }

    fn unregister(&'a mut self, id: PersonId) -> Result<(), ServiceError> {
        trace!("unregister person: id={}", id);
        let notifier = self.get_notifier();

        self.run_tx(move |usecase, ctx| usecase.remove(id).run(ctx))
            .and_then(|_| {
                let msg = format!(r#"{{ "person_id": {} }}"#, id);
                if let Err(e) = notifier.notify("unregister_person", &msg) {
                    error!("notification service not available: {}", e);
                }
                return Ok(());
            })
            .map_err(|e| {
                let msg = format!("cannot remove person: id={}", id);
                if let Err(e) = notifier.notify("admin", &msg) {
                    error!("notification service not available: {}", e);
                }
                return e;
            })
    }
}

// # フェイクテスト
//
// ## 目的
//
//   Service の正常系のテストを行う
//   Service の各メソッドが Usecase から通常期待される結果を受け取ったときに適切にふるまうことを保障する
//
// ## 方針
//
//   Usecase のフェイクに対して Service を実行し、その結果を確認する
//   フェイクはテスト時の比較チェックのしやすさを考慮して HashMap ではなく Vec で登録データを保持する
//   データ数は多くないので、Vec でリニアサーチしても十分な速度が出ると考える
//
// ## 実装
//
//                          Test Double
//        +---------+      +----------------+.oOo.+-----------+
//        | Service |      | Fake Usecase   |     | Dummy DAO |
//        | ======= |      | ============== |     | ========= |
//        |         |      |                |     |           |
//   --c->| ---c--> |---+->| ---+           |     |           |
//     |  |    |    |   |  |    | fake logic|     |           |
//   <-c--| <--c--- |<-+|--| <--+           |     |           |
//     |  +----|----+  ||  +----------------+	  +-----------+
//     |       |       ||
//     |       |       ||  +----------------+
//     |       |       ||  | Dummy Notifier |
//     |       |       ||  | ============== |
//     |       |       ||  |                |
//     |       |       |+->| --->           |
//     |       |       |   |                |
//     |       |       +---| <---           |
//     |       |           +----------------+
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
//   4. ダミーの Notifier 構造体を用意する
//   5. Service をここまでに用意したフェイクとダミーで構築する
//   6. Service のメソッドを呼び出す
//   7. Service からの戻り値を検証する
//
// ## 注意
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
        dto::PersonDto,
        notifier::NotifierError,
        HavePersonDao,
    };

    struct DummyPersonDao;
    impl PersonDao<()> for DummyPersonDao {
        fn insert(
            &self,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(1))
        }
        fn fetch(
            &self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(None))
        }
        fn select(&self) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(vec![]))
        }
        fn save(
            &self,
            _id: PersonId,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(()))
        }
        fn delete(&self, _id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(()))
        }
    }

    struct FakePersonUsecase {
        next_id: RefCell<PersonId>,
        db: Vec<(PersonId, PersonDto)>,
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
            person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = UsecaseError>
        where
            (): 'a,
        {
            let id = self.next_id.replace_with(|&mut i| i + 1);
            self.db.push((id, person));

            tx_rs::with_tx(move |&mut ()| Ok(id))
        }
        fn find<'a>(
            &'a mut self,
            id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = UsecaseError>
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
            person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (PersonId, PersonDto), Err = UsecaseError>
        where
            (): 'a,
        {
            let id = self.next_id.replace_with(|&mut i| i + 1);
            self.db.push((id, person.clone()));

            tx_rs::with_tx(move |&mut ()| Ok((id, person)))
        }
        fn collect<'a>(
            &'a mut self,
        ) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = UsecaseError>
        where
            (): 'a,
        {
            let result = self.db.clone();

            tx_rs::with_tx(move |&mut ()| Ok(result))
        }
        fn death<'a>(
            &'a mut self,
            id: PersonId,
            date: NaiveDate,
        ) -> impl tx_rs::Tx<(), Item = (), Err = UsecaseError>
        where
            (): 'a,
        {
            let person = self.db.iter_mut().find(|(i, _)| *i == id);

            if let Some((_, p)) = person {
                p.death_date = Some(date);
            }

            tx_rs::with_tx(move |&mut ()| Ok(()))
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

    struct DummyPersonOutputBoundary;
    impl PersonOutputBoundary<(u64, u64)> for DummyPersonOutputBoundary {
        fn started(&self) {}
        fn in_progress(&self, _progress: (u64, u64)) {}
        fn completed(&self) {}
        fn aborted(&self, _err: ServiceError) {}
    }

    #[test]
    fn test_register() {
        let usecase = Rc::new(RefCell::new(FakePersonUsecase {
            next_id: RefCell::new(1),
            db: vec![],
            dao: DummyPersonDao,
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };
        let expected_id = 1;
        let expected = PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"));

        let res = service.register("Alice", date(2012, 11, 2), None, "Alice is sender");
        assert_eq!(res, Ok((expected_id, expected)));
    }

    #[test]
    fn test_batch_import() {
        let usecase = Rc::new(RefCell::new(FakePersonUsecase {
            next_id: RefCell::new(1),
            db: vec![],
            dao: DummyPersonDao,
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };
        let persons = vec![
            PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
            PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
            PersonDto::new("Eve", date(1996, 12, 15), None, Some("Eve is interceptor")),
        ];
        let expected = persons.clone();

        let _ = service.batch_import(persons, Rc::new(DummyPersonOutputBoundary));
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
            next_id: RefCell::new(1),
            db: vec![
                (
                    1,
                    PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
                ),
                (
                    2,
                    PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
                ),
                (
                    3,
                    PersonDto::new("Eve", date(1996, 12, 15), None, Some("Eve is interceptor")),
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
    fn test_death() {
        let usecase = Rc::new(RefCell::new(FakePersonUsecase {
            next_id: RefCell::new(1),
            db: vec![(
                1,
                PersonDto::new(
                    "poor man",
                    date(2020, 5, 7),
                    None,
                    Some("poor man will be dead"),
                ),
            )],
            dao: DummyPersonDao,
        }));
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
        };

        let _ = service.death(1, date(2100, 4, 7));
        let expected = vec![(
            1,
            PersonDto::new(
                "poor man",
                date(2020, 5, 7),
                Some(date(2100, 4, 7)),
                Some("poor man will be dead"),
            ),
        )];

        assert_eq!(usecase.borrow().db, expected);
    }
    #[test]
    fn test_unregister() {
        let usecase = Rc::new(RefCell::new(FakePersonUsecase {
            next_id: RefCell::new(1),
            db: vec![
                (
                    1,
                    PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
                ),
                (
                    2,
                    PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
                ),
                (
                    3,
                    PersonDto::new("Eve", date(1996, 12, 15), None, Some("Eve is interceptor")),
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
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
            ),
            (
                3,
                PersonDto::new("Eve", date(1996, 12, 15), None, Some("Eve is interceptor")),
            ),
        ];

        assert_eq!(usecase.borrow().db, expected);
    }
}

// # スパイテスト
//
// ## 目的
//
//   Service の各メソッドが Usecase のメソッドを適切に呼び出していることを保障する
//   つまり、
//    1. 必要なメソッドを必要回数だけ呼び出していること
//    2. 不必要なメソッドを呼び出していないこと
//    3. Service に渡った引数が適切に Usecase のメソッドに渡されていること
//   を保障する
//
// ## 方針
//
//   スパイ Usecase はメソッドが呼び出されるたびに、それらを全て記録する
//   各メソッドの呼び出された記録をテストの最後で確認する
//
// ## 実装
//
//                            Test Double
//        +---------+        +------------------+.oOo.+------------+
//        | Service |        | Spy Usecase      |     | Dummy DAO  |
//        | ======= |        | ============     |     | ========== |
//        |         |        |                  |     |            |
//   ---->| ------> |--c+--->| --> [ c ] request|     |            |
//        |         |  ||    |       |    log   |     |            |
//   <----| <------ |<-||----| <--   |          |     |            |
//        +---------+  ||    +-------|----------+     +------------+
//                     ||            |
//       テスト対象 ---+|            +-- ここを確認する
//                      |
//                      |     Test Double
//                      |    +------------------+
//                      |    | Spy Notifier     |
//                      |    | =============    |
//                      |    |                  |
//                      +-c->| --> [ c ] request|
//                        |  |       |    log   |
//       テスト対象 ------+  | <--   |          |
//                           |       |          |
//                           +-------|----------+
//                                   |
//                                   +-- ここを確認する
//
//
//   1. ダミーの DAO 構造体を用意する
//      この構造体は実質使われないが、 Usecase の構成で必要になるため用意する
//   2. Usecase のメソッド呼び出しを記録する Usecase 構造体を用意する
//      この構造体はスパイなので、Service の間接的な出力のみを記録する
//   3. スパイの Notifier 構造体を用意する
//   4. Service をここまでに用意したスパイで構築する
//     4a. batch_import のみ Output Boundary への出力を記録する Spy Output Boundary を渡している
//   5. Service のメソッドを呼び出す
//   6. Usecase 構造体と Notifier 構造体の記録を検証する
//     6a. batch_import のみ Output Boundary 構造体の記録を検証する
//
// ## 注意
//
//   1. このテストは Service の実装を保障するものであって、Usecase の実装を保障するものではない
//   2. このテストは Service のメソッドが Usecase のメソッドと Notifier のメソッドとを適切に呼び出していることを保障するものであって、
//      Usecase や Notifier のメソッドが適切な処理を行っていることを保障するものではない
//   3. このテストは Service のメソッドが Usecase や Notifier のメソッドを不適切に呼び出していないことを保障するものであって、
//      Usecase のメソッドや Notifier のメソッドが不適切な処理をしていないことを保障するものではない
//   4. このテストでは Usecase のメソッドや Notifier のメソッドの呼び出し順序については検証しない (将来的には検証することは拒否しない)
#[cfg(test)]
mod spy_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::{
        dao::{DaoError, PersonDao},
        domain::date,
        dto::PersonDto,
        notifier::NotifierError,
        HavePersonDao,
    };

    struct DummyPersonDao;
    impl PersonDao<()> for DummyPersonDao {
        fn insert(
            &self,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(1))
        }
        fn fetch(
            &self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(None))
        }
        fn select(&self) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(vec![]))
        }
        fn save(
            &self,
            _id: PersonId,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(()))
        }
        fn delete(&self, _id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(()))
        }
    }

    struct SpyPersonUsecase {
        dao: DummyPersonDao,
        entry: RefCell<Vec<PersonDto>>,
        find: RefCell<Vec<PersonId>>,
        entry_and_verify: RefCell<Vec<PersonDto>>,
        collect: RefCell<i32>,
        death: RefCell<Vec<(PersonId, NaiveDate)>>,
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
            person: PersonDto,
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
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = UsecaseError>
        where
            (): 'a,
        {
            self.find.borrow_mut().push(id);

            // 返り値に意味はない
            tx_rs::with_tx(|&mut ()| Ok(None))
        }
        fn entry_and_verify<'a>(
            &'a mut self,
            person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (PersonId, PersonDto), Err = UsecaseError>
        where
            (): 'a,
        {
            self.entry_and_verify.borrow_mut().push(person.clone());

            // 返り値に意味はない
            tx_rs::with_tx(move |&mut ()| Ok((42, person)))
        }
        fn collect<'a>(
            &'a mut self,
        ) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = UsecaseError>
        where
            (): 'a,
        {
            *self.collect.borrow_mut() += 1;

            // 返り値に意味はない
            tx_rs::with_tx(|&mut ()| Ok(vec![]))
        }
        fn death<'a>(
            &'a mut self,
            id: PersonId,
            date: NaiveDate,
        ) -> impl tx_rs::Tx<(), Item = (), Err = UsecaseError>
        where
            (): 'a,
        {
            self.death.borrow_mut().push((id, date));

            // 返り値に意味はない
            tx_rs::with_tx(move |&mut ()| Ok(()))
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

    #[derive(Debug, Clone)]
    struct SpyNotifier {
        notify: Rc<RefCell<Vec<(String, String)>>>,
    }
    impl Notifier for SpyNotifier {
        fn notify(&self, _to: &str, _message: &str) -> Result<(), NotifierError> {
            self.notify
                .borrow_mut()
                .push((_to.to_string(), _message.to_string()));

            // 返り値に意味はない
            Ok(())
        }
    }

    struct TargetPersonService {
        usecase: Rc<RefCell<SpyPersonUsecase>>,
        notifier: SpyNotifier,
    }
    impl PersonService<'_, ()> for TargetPersonService {
        type U = SpyPersonUsecase;
        type N = SpyNotifier;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, ServiceError>
        where
            F: FnOnce(&mut Self::U, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(ServiceError::TransactionFailed)
        }

        fn get_notifier(&self) -> Self::N {
            self.notifier.clone()
        }
    }

    #[derive(Debug, Clone, Default)]
    struct SpyPersonOutputBoundary {
        started: RefCell<i32>,
        in_progress: RefCell<Vec<(u64, u64)>>,
        completed: RefCell<i32>,
        aborted: RefCell<Vec<ServiceError>>,
    }
    impl PersonOutputBoundary<(u64, u64)> for SpyPersonOutputBoundary {
        fn started(&self) {
            *self.started.borrow_mut() += 1;
        }
        fn in_progress(&self, progress: (u64, u64)) {
            self.in_progress.borrow_mut().push(progress);
        }
        fn completed(&self) {
            *self.completed.borrow_mut() += 1;
        }
        fn aborted(&self, err: ServiceError) {
            self.aborted.borrow_mut().push(err);
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
            death: RefCell::new(vec![]),
            remove: RefCell::new(vec![]),
        }));
        let notifier = SpyNotifier {
            notify: RefCell::new(vec![]).into(),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let expected = PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"));

        let _ = service.register("Alice", date(2012, 11, 2), None, "Alice is sender");

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 0);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 1);
        assert_eq!(*usecase.borrow().collect.borrow(), 0);
        assert_eq!(usecase.borrow().death.borrow().len(), 0);
        assert_eq!(usecase.borrow().remove.borrow().len(), 0);

        // Service の引数が Usecase にそのまま渡されていることを検証
        assert_eq!(usecase.borrow().entry_and_verify.borrow()[0], expected);

        // Notifier のメソッド呼び出しの記録の検証
        assert_eq!(service.get_notifier().notify.borrow().len(), 1);

        // Service の引数が Notifier にそのまま渡されていることを検証
        assert_eq!(
            service.get_notifier().notify.borrow()[0],
            (
                "entry_person".to_string(),
                r#"{ "person_id": 42 }"#.to_string()
            )
        );
    }

    #[test]
    fn test_batch_import() {
        let usecase = Rc::new(RefCell::new(SpyPersonUsecase {
            dao: DummyPersonDao,
            entry: RefCell::new(vec![]),
            find: RefCell::new(vec![]),
            entry_and_verify: RefCell::new(vec![]),
            collect: RefCell::new(0),
            death: RefCell::new(vec![]),
            remove: RefCell::new(vec![]),
        }));
        let notifier = SpyNotifier {
            notify: RefCell::new(vec![]).into(),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let persons = vec![
            PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
            PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
            PersonDto::new("Eve", date(1996, 12, 15), None, Some("Eve is interseptor")),
        ];
        let expected = persons.clone();
        let out_port = Rc::new(SpyPersonOutputBoundary::default());

        let _ = service.batch_import(persons, out_port.clone());

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 3);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 0);
        assert_eq!(*usecase.borrow().collect.borrow(), 0);
        assert_eq!(usecase.borrow().death.borrow().len(), 0);
        assert_eq!(usecase.borrow().remove.borrow().len(), 0);

        // Service の引数が Usecase にそのまま渡されていることを検証
        assert_eq!(usecase.borrow().entry.borrow().clone(), expected);

        // Notifier のメソッド呼び出しの記録の検証
        assert_eq!(service.get_notifier().notify.borrow().len(), 3);

        // Service の引数が Notifier にそのまま渡されていることを検証
        assert_eq!(
            *service.get_notifier().notify.borrow(),
            vec![
                (
                    "entry_person".to_string(),
                    r#"{ "person_id": 42 }"#.to_string()
                ),
                (
                    "entry_person".to_string(),
                    r#"{ "person_id": 42 }"#.to_string()
                ),
                (
                    "entry_person".to_string(),
                    r#"{ "person_id": 42 }"#.to_string()
                )
            ]
        );

        // PersonOutputBoundary のメソッド呼び出しの記録の検証
        assert_eq!(*out_port.started.borrow(), 1);
        assert_eq!(out_port.in_progress.borrow().len(), 3);
        assert_eq!(*out_port.completed.borrow(), 1);
        assert_eq!(out_port.aborted.borrow().len(), 0);

        // PersonOutputboundary の引数がそのまま渡されていることを検証
        assert_eq!(*out_port.in_progress.borrow(), vec![(3, 1), (3, 2), (3, 3)]);
    }

    #[test]
    fn list_all() {
        let usecase = Rc::new(RefCell::new(SpyPersonUsecase {
            dao: DummyPersonDao,
            entry: RefCell::new(vec![]),
            find: RefCell::new(vec![]),
            entry_and_verify: RefCell::new(vec![]),
            collect: RefCell::new(0),
            death: RefCell::new(vec![]),
            remove: RefCell::new(vec![]),
        }));
        let notifier = SpyNotifier {
            notify: RefCell::new(vec![]).into(),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let _ = service.list_all();

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 0);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 0);
        assert_eq!(*usecase.borrow().collect.borrow(), 1);
        assert_eq!(usecase.borrow().death.borrow().len(), 0);
        assert_eq!(usecase.borrow().remove.borrow().len(), 0);

        // Notifier のメソッド呼び出しの記録の検証
        assert_eq!(service.get_notifier().notify.borrow().len(), 0);
    }
    #[test]
    fn test_death() {
        let usecase = Rc::new(RefCell::new(SpyPersonUsecase {
            dao: DummyPersonDao,
            entry: RefCell::new(vec![]),
            find: RefCell::new(vec![]),
            entry_and_verify: RefCell::new(vec![]),
            collect: RefCell::new(0),
            death: RefCell::new(vec![]),
            remove: RefCell::new(vec![]),
        }));
        let notifier = SpyNotifier {
            notify: RefCell::new(vec![]).into(),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let _ = service.death(42, date(2020, 7, 19));

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 0);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 0);
        assert_eq!(*usecase.borrow().collect.borrow(), 0);
        assert_eq!(usecase.borrow().remove.borrow().len(), 0);

        // Service の引数が Usecase にそのまま渡されていることを検証
        assert_eq!(usecase.borrow().death.borrow()[0], (42, date(2020, 7, 19)));

        // Notifier のメソッド呼び出しの記録の検証
        assert_eq!(service.get_notifier().notify.borrow().len(), 1);

        // Service の引数が Notifier にそのまま渡されていることを検証
        assert_eq!(
            service.get_notifier().notify.borrow()[0],
            (
                "death_person".to_string(),
                r#"{ "person_id": 42, "death_date": 2020-07-19 }"#.to_string()
            )
        );
    }
    #[test]
    fn test_unregister() {
        let usecase = Rc::new(RefCell::new(SpyPersonUsecase {
            dao: DummyPersonDao,
            entry: RefCell::new(vec![]),
            find: RefCell::new(vec![]),
            entry_and_verify: RefCell::new(vec![]),
            collect: RefCell::new(0),
            death: RefCell::new(vec![]),
            remove: RefCell::new(vec![]),
        }));
        let notifier = SpyNotifier {
            notify: RefCell::new(vec![]).into(),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let _ = service.unregister(42);

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 0);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 0);
        assert_eq!(*usecase.borrow().collect.borrow(), 0);
        assert_eq!(usecase.borrow().death.borrow().len(), 0);
        assert_eq!(usecase.borrow().remove.borrow().len(), 1);

        // Service の引数が Usecase にそのまま渡されていることを検証
        assert_eq!(usecase.borrow().remove.borrow()[0], 42);

        // Notifier のメソッド呼び出しの記録の検証
        assert_eq!(service.get_notifier().notify.borrow().len(), 1);

        // Service の引数が Notifier にそのまま渡されていることを検証
        assert_eq!(
            *service.get_notifier().notify.borrow(),
            vec![(
                "unregister_person".to_string(),
                r#"{ "person_id": 42 }"#.to_string()
            )]
        );
    }
}

// # エラー系スタブテスト
//
// ## 目的
//
//   Usecase がエラーを返した場合の Service の挙動を保障する
//
// ## 方針
//
//   スタブ Usecase はメソッドが呼び出されると、事前に設定された任意のエラー値を返す
//   Service のメソッドを呼び出して Usecase からエラーを受け取ったときの Service の返り値を確認する
//
// ## 実装
//
//                          Test Double
//        +---------+      +---------------+.oOo.+-----------+
//        | Service |      | Stub Usecase  |     | Dummy DAO |
//        | ======= |      | ============  |     | ========= |
//        |         |      |               |     |           |
//   ---->| ------> |----->| --->          |     |           |
//        |         |      |               |     |           |
//   <-c--| <--c--- |<--+--| <--- any error|     |           |
//     |  +----|----+   |  +---------------+     +-----------+
//     |       |        |
//     |       |        |  +---------------+
//     |       |        |  | Stub Notifier |
//     |       | 	|  | ============= |
//     |       |        |  |               |
//     |       |        |  | --->          |
//     |       |        |  |               |
//     |       |        +--| <--- any error|
//     |       |           +---------------+
//     |       |
//     |       +-- テスト対象
//     |
//     +-- ここを確認する
//
//   1. ダミーの DAO 構造体を用意する
//      この構造体は実質使われないが、 Usecase の構成で必要になるため用意する
//   2. Usecase のメソッドが任意の結果を返せる種類の Usecase 構造体を用意する
//      この Usecase 構造体はスタブであり、Service への間接的な入力のみ制御する
//   3. Notifier のメソッドが任意の結果を返せる種類の Notifier 構造体を用意する
//      この Notifier 構造体はスタブであり、Service への間接的な入力のみ制御する
//   4. Service をここまでに用意したスタブで構築する
//   5. Service のメソッドを呼び出す
//   6. Service のメソッドからの戻り値を確認する
//
// ## 注意
//
#[cfg(test)]
mod error_stub_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::{
        dao::{DaoError, PersonDao},
        domain::date,
        dto::PersonDto,
        notifier::NotifierError,
        HavePersonDao,
    };

    struct DummyPersonDao;
    impl PersonDao<()> for DummyPersonDao {
        fn insert(
            &self,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(1))
        }
        fn fetch(
            &self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(None))
        }
        fn select(&self) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(vec![]))
        }
        fn save(
            &self,
            _id: PersonId,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(()))
        }
        fn delete(&self, _id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(()))
        }
    }

    struct StubPersonUsecase {
        dao: DummyPersonDao,
        entry_result: Result<PersonId, UsecaseError>,
        find_result: Result<Option<PersonDto>, UsecaseError>,
        entry_and_verify_result: Result<(PersonId, PersonDto), UsecaseError>,
        collect_result: Result<Vec<(PersonId, PersonDto)>, UsecaseError>,
        death_result: Result<(), UsecaseError>,
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
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(|&mut ()| self.entry_result.clone())
        }
        fn find<'a>(
            &'a mut self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(|&mut ()| self.find_result.clone())
        }
        fn entry_and_verify<'a>(
            &'a mut self,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (PersonId, PersonDto), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| self.entry_and_verify_result.clone())
        }
        fn collect<'a>(
            &'a mut self,
        ) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(|&mut ()| self.collect_result.clone())
        }
        fn death<'a>(
            &'a mut self,
            _id: PersonId,
            _date: NaiveDate,
        ) -> impl tx_rs::Tx<(), Item = (), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(|&mut ()| self.death_result.clone())
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

    #[derive(Debug, Clone)]
    struct StubNotifier {
        admin_result: Result<(), NotifierError>,
        entry_person_result: Result<(), NotifierError>,
        death_person_result: Result<(), NotifierError>,
        unregister_person_result: Result<(), NotifierError>,
        otherwise_result: Result<(), NotifierError>,
    }
    impl Notifier for StubNotifier {
        fn notify(&self, to: &str, _message: &str) -> Result<(), NotifierError> {
            match to {
                "entry_person" => self.entry_person_result.clone(),
                "death_person" => self.death_person_result.clone(),
                "unregister_person" => self.unregister_person_result.clone(),
                "admin" => self.admin_result.clone(),
                _ => self.otherwise_result.clone(),
            }
        }
    }

    struct TargetPersonService {
        usecase: Rc<RefCell<StubPersonUsecase>>,
        notifier: StubNotifier,
    }
    impl PersonService<'_, ()> for TargetPersonService {
        type U = StubPersonUsecase;
        type N = StubNotifier;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, ServiceError>
        where
            F: FnOnce(&mut Self::U, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(ServiceError::TransactionFailed)
        }

        fn get_notifier(&self) -> Self::N {
            self.notifier.clone()
        }
    }

    struct DummyPersonOutputBoundary;
    impl PersonOutputBoundary<(u64, u64)> for DummyPersonOutputBoundary {
        fn started(&self) {}
        fn in_progress(&self, _progress: (u64, u64)) {}
        fn completed(&self) {}
        fn aborted(&self, _err: ServiceError) {}
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
            death_result: Ok(()),       // 使わない
            remove_result: Ok(()),      // 使わない
        }));
        let notifier = StubNotifier {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
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
    fn test_register_entry_and_verify_notify_error_for_entry_person() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                1,
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
            )),
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),       // 使わない
            remove_result: Ok(()),      // 使わない
        }));
        let notifier = StubNotifier {
            admin_result: Ok(()),
            entry_person_result: Err(NotifierError::Unavailable("valid req".to_string())),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let result = service.register("Alice", date(2012, 11, 2), None, "Alice is sender");

        assert!(result.is_ok());
    }

    #[test]
    fn test_register_entry_and_verify_notify_error_for_admin() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Err(UsecaseError::EntryAndVerifyPersonFailed(
                DaoError::InsertError("valid dao".to_string()),
            )),
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),       // 使わない
            remove_result: Ok(()),      // 使わない
        }));
        let notifier = StubNotifier {
            admin_result: Err(NotifierError::Unavailable("valid req".to_string())),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
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
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()), // 使わない
        }));
        let notifier = StubNotifier {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let result = service.batch_import(
            vec![
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
                PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
            ],
            Rc::new(DummyPersonOutputBoundary),
        );
        let expected = usecase.borrow().entry_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }

    #[test]
    fn test_batch_import_notify_error_for_entry_person() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(42),
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()), // 使わない
        }));
        let notifier = StubNotifier {
            admin_result: Ok(()),
            entry_person_result: Err(NotifierError::Unavailable("valid req".to_string())),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let result = service.batch_import(
            vec![
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
                PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
            ],
            Rc::new(DummyPersonOutputBoundary),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_batch_import_notify_error_for_admin() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Err(UsecaseError::EntryPersonFailed(DaoError::InsertError(
                "valid dao".to_string(),
            ))),
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()), // 使わない
        }));
        let notifier = StubNotifier {
            admin_result: Err(NotifierError::Unavailable("valid req".to_string())),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let result = service.batch_import(
            vec![
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender")),
                PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver")),
            ],
            Rc::new(DummyPersonOutputBoundary),
        );
        let expected = Err(ServiceError::TransactionFailed(
            usecase.borrow().entry_result.clone().unwrap_err(),
        ));

        assert_eq!(result, expected);
    }

    #[test]
    fn test_list_all() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None),
            )), // 使わない
            collect_result: Err(UsecaseError::CollectPersonFailed(DaoError::SelectError(
                "valid dao".to_string(),
            ))),
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()), // 使わない
        }));
        let notifier = StubNotifier {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let result = service.list_all();
        let expected = usecase.borrow().collect_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }

    #[test]
    fn test_list_all_notify_for_admin() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None),
            )), // 使わない
            collect_result: Err(UsecaseError::CollectPersonFailed(DaoError::SelectError(
                "valid dao".to_string(),
            ))),
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()), // 使わない
        }));
        let notifier = StubNotifier {
            admin_result: Err(NotifierError::Unavailable("valid req".to_string())),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
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
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Err(UsecaseError::RemovePersonFailed(DaoError::DeleteError(
                "valid dao".to_string(),
            ))),
        }));
        let notifier = StubNotifier {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let result = service.unregister(42);
        let expected = usecase.borrow().remove_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }

    #[test]
    fn test_death_notify_for_unregister_person() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),
            remove_result: Ok(()), // 使わない
        }));
        let notifier = StubNotifier {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Err(NotifierError::Unavailable("valid req".to_string())),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let result = service.death(42, date(2020, 8, 30));

        assert!(result.is_ok());
    }

    #[test]
    fn test_death_notify_for_admin() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Err(UsecaseError::SavePersonFailed(DaoError::UpdateError(
                "valid dao".to_string(),
            ))),
            remove_result: Ok(()), // 使わない
        }));
        let notifier = StubNotifier {
            admin_result: Err(NotifierError::Unavailable("valid req".to_string())),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let result = service.death(42, date(2020, 8, 30));
        let expected = usecase.borrow().death_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }

    #[test]
    fn test_unregister_notify_for_unregister_person() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()),
        }));
        let notifier = StubNotifier {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Err(NotifierError::Unavailable("valid req".to_string())),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let result = service.unregister(42);

        assert!(result.is_ok());
    }

    #[test]
    fn test_unregister_notify_for_admin() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Err(UsecaseError::RemovePersonFailed(DaoError::DeleteError(
                "valid dao".to_string(),
            ))),
        }));
        let notifier = StubNotifier {
            admin_result: Err(NotifierError::Unavailable("valid req".to_string())),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            notifier,
        };

        let result = service.unregister(42);
        let expected = usecase.borrow().remove_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }
}
