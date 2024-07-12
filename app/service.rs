use chrono::NaiveDate;
use log::{error, trace};
use std::fmt;
use std::iter::Iterator;
use std::rc::Rc;
use thiserror::Error;

use crate::domain::PersonId;
use crate::dto::PersonDto;
use crate::reporter::{Level, Reporter};
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

pub trait PersonOutputBoundary<T, E> {
    fn started(&self);
    fn in_progress(&self, progress: T);
    fn completed(&self);
    fn aborted(&self, err: E);
}

pub trait PersonService<'a, Ctx> {
    type U: PersonUsecase<Ctx>;
    type N: Reporter<'a>;

    fn run_tx<T, F>(&'a mut self, f: F) -> Result<T, ServiceError>
    where
        F: FnOnce(&mut Self::U, &mut Ctx) -> Result<T, UsecaseError>;

    fn get_reporter(&self) -> Self::N;

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
        let reporter = self.get_reporter();

        self.run_tx(move |usecase, ctx| {
            usecase
                .entry_and_verify(PersonDto::new(name, birth_date, death_date, Some(data), 0))
                .run(ctx)
        })
        .and_then(|(id, p)| {
            let msg = format!("registered person_id: {}", id);
            if let Err(e) = reporter.send_report(Level::Info, "entry_person", &msg, location!()) {
                error!("reporter service not available: {}", e);
            }
            return Ok((id, p));
        })
        .map_err(|e| {
            let msg = format!(
                "cannot register person: name={}, birth_date={}, death_date={:?}, data={}",
                name, birth_date, death_date, data
            );
            if let Err(e) = reporter.send_report(Level::Error, "admin", &msg, location!()) {
                error!("reporter service not available: {}", e);
            }
            return e;
        })
    }

    fn find(&'a mut self, id: PersonId) -> Result<Option<PersonDto>, ServiceError> {
        trace!("find person: id={}", id);
        let reporter = self.get_reporter();

        self.run_tx(move |usecase, ctx| usecase.find(id).run(ctx))
            .map_err(|e| {
                let msg = format!("cannot find person: id={}", id);
                if let Err(e) = reporter.send_report(Level::Error, "admin", &msg, location!()) {
                    error!("reporter service not available: {}", e);
                }
                return e;
            })
    }

    fn batch_import(
        &'a mut self,
        persons: impl Iterator<Item = PersonDto>,
        out_port: Rc<impl PersonOutputBoundary<(u64, u64), ServiceError>>,
    ) -> Result<Vec<PersonId>, ServiceError> {
        trace!("batch import persons");
        out_port.started();
        let reporter = self.get_reporter();

        let mut ids = vec![];
        let (lower_bound, upper_bound) = persons.size_hint();
        let total = upper_bound.unwrap_or(lower_bound) as u64;
        self.run_tx(move |usecase, ctx| {
            for person in persons {
                let res = usecase.entry(person).run(ctx);
                match res {
                    Ok(id) => {
                        ids.push(id);

                        let msg = format!("registered person_id: {}", id);
                        if let Err(e) =
                            reporter.send_report(Level::Info, "entry_person", &msg, location!())
                        {
                            error!("reporter service not available: {}", e);
                        }
                    }
                    Err(e) => {
                        trace!("batch import aborted: {:?}", e);
                        out_port.aborted(ServiceError::TransactionFailed(e.clone()));

                        let msg = format!("cannot entry person: {:?}", e);
                        if let Err(e) =
                            reporter.send_report(Level::Error, "admin", &msg, location!())
                        {
                            error!("reporter service not available: {}", e);
                        }
                        return Err(e);
                    }
                }
                trace!("batch import in_progress: {:?}", ids.len());
                out_port.in_progress((total, ids.len() as u64));
            }
            trace!("batch import completed: {:?}", ids.len());
            out_port.completed();
            Ok(ids)
        })
    }

    fn list_all(&'a mut self) -> Result<Vec<(PersonId, PersonDto)>, ServiceError> {
        trace!("list all persons");
        let reporter = self.get_reporter();

        self.run_tx(move |usecase, ctx| usecase.collect().run(ctx))
            .map_err(|e| {
                if let Err(e) = reporter.send_report(
                    Level::Error,
                    "admin",
                    "cannot list all persons",
                    location!(),
                ) {
                    error!("reporter service not available: {}", e);
                }
                return e;
            })
    }

    fn death(&'a mut self, id: PersonId, death_date: NaiveDate) -> Result<(), ServiceError> {
        trace!("death person: id={}, death_date={}", id, death_date);
        let reporter = self.get_reporter();

        self.run_tx(move |usecase, ctx| usecase.death(id, death_date).run(ctx))
            .and_then(|_| {
                let msg = format!("death person_id: {}, death_date: {}", id, death_date);
                if let Err(e) = reporter.send_report(Level::Info, "death_person", &msg, location!())
                {
                    error!("reporter service not available: {}", e);
                }
                return Ok(());
            })
            .map_err(|e| {
                let msg = format!("cannot death person: id={}, death_date={}", id, death_date);
                if let Err(e) = reporter.send_report(Level::Error, "admin", &msg, location!()) {
                    error!("reporter service not available: {}", e);
                }
                return e;
            })
    }

    fn unregister(&'a mut self, id: PersonId) -> Result<(), ServiceError> {
        trace!("unregister person: id={}", id);
        let reporter = self.get_reporter();

        self.run_tx(move |usecase, ctx| usecase.remove(id).run(ctx))
            .and_then(|_| {
                let msg = format!("unregistered person_id: {}", id);
                if let Err(e) =
                    reporter.send_report(Level::Info, "unregister_person", &msg, location!())
                {
                    error!("reporter service not available: {}", e);
                }
                return Ok(());
            })
            .map_err(|e| {
                let msg = format!("cannot remove person: id={}", id);
                if let Err(e) = reporter.send_report(Level::Error, "admin", &msg, location!()) {
                    error!("reporter service not available: {}", e);
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
//     |       |       ||  | Dummy Reporter |
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
//   4. ダミーの Reporter 構造体を用意する
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
        domain::{date, Revision},
        dto::PersonDto,
        reporter::{Location, ReporterError},
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
            _revision: Revision,
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

    struct DummyReporter;
    impl Reporter<'_> for DummyReporter {
        fn register(
            &mut self,
            _observer: impl crate::reporter::Observer,
        ) -> Result<(), ReporterError> {
            Ok(())
        }
        fn get_observers(&self) -> Vec<&dyn crate::reporter::Observer> {
            vec![]
        }
        fn send_report(
            &self,
            _level: Level,
            _to: &str,
            _message: &str,
            _loc: Location,
        ) -> Result<(), ReporterError> {
            Ok(())
        }
    }

    struct TargetPersonService {
        usecase: Rc<RefCell<FakePersonUsecase>>,
    }
    impl PersonService<'_, ()> for TargetPersonService {
        type U = FakePersonUsecase;
        type N = DummyReporter;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, ServiceError>
        where
            F: FnOnce(&mut Self::U, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(ServiceError::TransactionFailed)
        }

        fn get_reporter(&self) -> Self::N {
            DummyReporter
        }
    }

    struct DummyPersonOutputBoundary;
    impl PersonOutputBoundary<(u64, u64), ServiceError> for DummyPersonOutputBoundary {
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
        let expected = PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 0);

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
            PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 3),
            PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver"), 1),
            PersonDto::new(
                "Eve",
                date(1996, 12, 15),
                None,
                Some("Eve is interceptor"),
                7,
            ),
        ];
        let expected = persons.clone();

        let _ = service.batch_import(persons.into_iter(), Rc::new(DummyPersonOutputBoundary));
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
                    PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 3),
                ),
                (
                    2,
                    PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver"), 1),
                ),
                (
                    3,
                    PersonDto::new(
                        "Eve",
                        date(1996, 12, 15),
                        None,
                        Some("Eve is interceptor"),
                        7,
                    ),
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
                    0,
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
                0,
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
                    PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 3),
                ),
                (
                    2,
                    PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver"), 1),
                ),
                (
                    3,
                    PersonDto::new(
                        "Eve",
                        date(1996, 12, 15),
                        None,
                        Some("Eve is interceptor"),
                        7,
                    ),
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
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 3),
            ),
            (
                3,
                PersonDto::new(
                    "Eve",
                    date(1996, 12, 15),
                    None,
                    Some("Eve is interceptor"),
                    7,
                ),
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
//                      |    | Spy Reporter     |
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
//   3. スパイの Reporter 構造体を用意する
//   4. Service をここまでに用意したスパイで構築する
//     4a. batch_import のみ Output Boundary への出力を記録する Spy Output Boundary を渡している
//   5. Service のメソッドを呼び出す
//   6. Usecase 構造体と Reporter 構造体の記録を検証する
//     6a. batch_import のみ Output Boundary 構造体の記録を検証する
//
// ## 注意
//
//   1. このテストは Service の実装を保障するものであって、Usecase の実装を保障するものではない
//   2. このテストは Service のメソッドが Usecase のメソッドと Reporter のメソッドとを適切に呼び出していることを保障するものであって、
//      Usecase や Reporter のメソッドが適切な処理を行っていることを保障するものではない
//   3. このテストは Service のメソッドが Usecase や Reporter のメソッドを不適切に呼び出していないことを保障するものであって、
//      Usecase のメソッドや Reporter のメソッドが不適切な処理をしていないことを保障するものではない
//   4. このテストでは Usecase のメソッドや Reporter のメソッドの呼び出し順序については検証しない (将来的には検証することは拒否しない)
#[cfg(test)]
mod spy_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::{
        dao::{DaoError, PersonDao},
        domain::{date, Revision},
        dto::PersonDto,
        reporter::{Location, ReporterError},
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
            _revision: Revision,
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
    struct SpyReporter {
        report: Rc<RefCell<Vec<(String, String)>>>,
    }
    impl Reporter<'_> for SpyReporter {
        fn register(
            &mut self,
            _observer: impl crate::reporter::Observer,
        ) -> Result<(), ReporterError> {
            Ok(())
        }
        fn get_observers(&self) -> Vec<&dyn crate::reporter::Observer> {
            vec![]
        }
        fn send_report(
            &self,
            _level: Level,
            _to: &str,
            _message: &str,
            _loc: Location,
        ) -> Result<(), ReporterError> {
            self.report
                .borrow_mut()
                .push((_to.to_string(), _message.to_string()));

            // 返り値に意味はない
            Ok(())
        }
    }

    struct TargetPersonService {
        usecase: Rc<RefCell<SpyPersonUsecase>>,
        reporter: SpyReporter,
    }
    impl PersonService<'_, ()> for TargetPersonService {
        type U = SpyPersonUsecase;
        type N = SpyReporter;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, ServiceError>
        where
            F: FnOnce(&mut Self::U, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(ServiceError::TransactionFailed)
        }

        fn get_reporter(&self) -> Self::N {
            self.reporter.clone()
        }
    }

    #[derive(Debug, Clone, Default)]
    struct SpyPersonOutputBoundary {
        started: RefCell<i32>,
        in_progress: RefCell<Vec<(u64, u64)>>,
        completed: RefCell<i32>,
        aborted: RefCell<Vec<ServiceError>>,
    }
    impl PersonOutputBoundary<(u64, u64), ServiceError> for SpyPersonOutputBoundary {
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
        let reporter = SpyReporter {
            report: RefCell::new(vec![]).into(),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let expected = PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 0);

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

        // Reporter のメソッド呼び出しの記録の検証
        assert_eq!(service.get_reporter().report.borrow().len(), 1);

        // Service の引数が Reporter にそのまま渡されていることを検証
        assert_eq!(
            service.get_reporter().report.borrow()[0],
            (
                "entry_person".to_string(),
                "registered person_id: 42".to_string()
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
        let reporter = SpyReporter {
            report: RefCell::new(vec![]).into(),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let persons = vec![
            PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 3),
            PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver"), 1),
            PersonDto::new(
                "Eve",
                date(1996, 12, 15),
                None,
                Some("Eve is interseptor"),
                7,
            ),
        ];
        let expected = persons.clone();
        let out_port = Rc::new(SpyPersonOutputBoundary::default());

        let _ = service.batch_import(persons.into_iter(), out_port.clone());

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 3);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 0);
        assert_eq!(*usecase.borrow().collect.borrow(), 0);
        assert_eq!(usecase.borrow().death.borrow().len(), 0);
        assert_eq!(usecase.borrow().remove.borrow().len(), 0);

        // Service の引数が Usecase にそのまま渡されていることを検証
        assert_eq!(usecase.borrow().entry.borrow().clone(), expected);

        // Reporter のメソッド呼び出しの記録の検証
        assert_eq!(service.get_reporter().report.borrow().len(), 3);

        // Service の引数が Reporter にそのまま渡されていることを検証
        assert_eq!(
            *service.get_reporter().report.borrow(),
            vec![
                (
                    "entry_person".to_string(),
                    "registered person_id: 42".to_string()
                ),
                (
                    "entry_person".to_string(),
                    "registered person_id: 42".to_string()
                ),
                (
                    "entry_person".to_string(),
                    "registered person_id: 42".to_string()
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
        let reporter = SpyReporter {
            report: RefCell::new(vec![]).into(),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let _ = service.list_all();

        // Usecase のメソッドの呼び出し記録の検証
        assert_eq!(usecase.borrow().entry.borrow().len(), 0);
        assert_eq!(usecase.borrow().find.borrow().len(), 0);
        assert_eq!(usecase.borrow().entry_and_verify.borrow().len(), 0);
        assert_eq!(*usecase.borrow().collect.borrow(), 1);
        assert_eq!(usecase.borrow().death.borrow().len(), 0);
        assert_eq!(usecase.borrow().remove.borrow().len(), 0);

        // Reporter のメソッド呼び出しの記録の検証
        assert_eq!(service.get_reporter().report.borrow().len(), 0);
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
        let reporter = SpyReporter {
            report: RefCell::new(vec![]).into(),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
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

        // Reporter のメソッド呼び出しの記録の検証
        assert_eq!(service.get_reporter().report.borrow().len(), 1);

        // Service の引数が Reporter にそのまま渡されていることを検証
        assert_eq!(
            service.get_reporter().report.borrow()[0],
            (
                "death_person".to_string(),
                "death person_id: 42, death_date: 2020-07-19".to_string()
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
        let reporter = SpyReporter {
            report: RefCell::new(vec![]).into(),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
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

        // Reporter のメソッド呼び出しの記録の検証
        assert_eq!(service.get_reporter().report.borrow().len(), 1);

        // Service の引数が Reporter にそのまま渡されていることを検証
        assert_eq!(
            *service.get_reporter().report.borrow(),
            vec![(
                "unregister_person".to_string(),
                "unregistered person_id: 42".to_string()
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
//     |       |        |  | Stub Reporter |
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
//   3. Reporter のメソッドが任意の結果を返せる種類の Reporter 構造体を用意する
//      この Reporter 構造体はスタブであり、Service への間接的な入力のみ制御する
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
        domain::{date, Revision},
        dto::PersonDto,
        reporter::{Location, ReporterError},
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
            _revision: Revision,
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
    struct StubReporter {
        admin_result: Result<(), ReporterError>,
        entry_person_result: Result<(), ReporterError>,
        death_person_result: Result<(), ReporterError>,
        unregister_person_result: Result<(), ReporterError>,
        otherwise_result: Result<(), ReporterError>,
    }
    impl Reporter<'_> for StubReporter {
        fn register(
            &mut self,
            _observer: impl crate::reporter::Observer,
        ) -> Result<(), ReporterError> {
            Ok(())
        }
        fn get_observers(&self) -> Vec<&dyn crate::reporter::Observer> {
            vec![]
        }
        fn send_report(
            &self,
            _level: Level,
            to: &str,
            _message: &str,
            _loc: Location,
        ) -> Result<(), ReporterError> {
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
        reporter: StubReporter,
    }
    impl PersonService<'_, ()> for TargetPersonService {
        type U = StubPersonUsecase;
        type N = StubReporter;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, ServiceError>
        where
            F: FnOnce(&mut Self::U, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(ServiceError::TransactionFailed)
        }

        fn get_reporter(&self) -> Self::N {
            self.reporter.clone()
        }
    }

    struct DummyPersonOutputBoundary;
    impl PersonOutputBoundary<(u64, u64), ServiceError> for DummyPersonOutputBoundary {
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
        let reporter = StubReporter {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
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
    fn test_register_entry_and_verify_repoter_error_for_entry_person() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                1,
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 0),
            )),
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),       // 使わない
            remove_result: Ok(()),      // 使わない
        }));
        let reporter = StubReporter {
            admin_result: Ok(()),
            entry_person_result: Err(ReporterError::Unavailable("valid req".to_string())),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let result = service.register("Alice", date(2012, 11, 2), None, "Alice is sender");

        assert!(result.is_ok());
    }

    #[test]
    fn test_register_entry_and_verify_reporter_error_for_admin() {
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
        let reporter = StubReporter {
            admin_result: Err(ReporterError::Unavailable("valid req".to_string())),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
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
                PersonDto::new("Alice", date(2012, 11, 2), None, None, 0),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()), // 使わない
        }));
        let reporter = StubReporter {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let result = service.batch_import(
            vec![
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 0),
                PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver"), 0),
            ]
            .into_iter(),
            Rc::new(DummyPersonOutputBoundary),
        );
        let expected = usecase.borrow().entry_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }

    #[test]
    fn test_batch_import_reporter_error_for_entry_person() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(42),
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None, 0),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()), // 使わない
        }));
        let reporter = StubReporter {
            admin_result: Ok(()),
            entry_person_result: Err(ReporterError::Unavailable("valid req".to_string())),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let result = service.batch_import(
            vec![
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 0),
                PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver"), 0),
            ]
            .into_iter(),
            Rc::new(DummyPersonOutputBoundary),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_batch_import_reporter_error_for_admin() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Err(UsecaseError::EntryPersonFailed(DaoError::InsertError(
                "valid dao".to_string(),
            ))),
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None, 0),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()), // 使わない
        }));
        let reporter = StubReporter {
            admin_result: Err(ReporterError::Unavailable("valid req".to_string())),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let result = service.batch_import(
            vec![
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 0),
                PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver"), 0),
            ]
            .into_iter(),
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
                PersonDto::new("Alice", date(2012, 11, 2), None, None, 0),
            )), // 使わない
            collect_result: Err(UsecaseError::CollectPersonFailed(DaoError::SelectError(
                "valid dao".to_string(),
            ))),
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()), // 使わない
        }));
        let reporter = StubReporter {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let result = service.list_all();
        let expected = usecase.borrow().collect_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }

    #[test]
    fn test_list_all_reporter_for_admin() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None, 0),
            )), // 使わない
            collect_result: Err(UsecaseError::CollectPersonFailed(DaoError::SelectError(
                "valid dao".to_string(),
            ))),
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()), // 使わない
        }));
        let reporter = StubReporter {
            admin_result: Err(ReporterError::Unavailable("valid req".to_string())),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
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
                PersonDto::new("Alice", date(2012, 11, 2), None, None, 0),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Err(UsecaseError::RemovePersonFailed(DaoError::DeleteError(
                "valid dao".to_string(),
            ))),
        }));
        let reporter = StubReporter {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let result = service.unregister(42);
        let expected = usecase.borrow().remove_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }

    #[test]
    fn test_death_reporter_for_unregister_person() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None, 0),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),
            remove_result: Ok(()), // 使わない
        }));
        let reporter = StubReporter {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Err(ReporterError::Unavailable("valid req".to_string())),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let result = service.death(42, date(2020, 8, 30));

        assert!(result.is_ok());
    }

    #[test]
    fn test_death_reporter_for_admin() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None, 0),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Err(UsecaseError::SavePersonFailed(DaoError::UpdateError(
                "valid dao".to_string(),
            ))),
            remove_result: Ok(()), // 使わない
        }));
        let reporter = StubReporter {
            admin_result: Err(ReporterError::Unavailable("valid req".to_string())),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let result = service.death(42, date(2020, 8, 30));
        let expected = usecase.borrow().death_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }

    #[test]
    fn test_unregister_reporter_for_unregister_person() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None, 0),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Ok(()),
        }));
        let reporter = StubReporter {
            admin_result: Ok(()),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Err(ReporterError::Unavailable("valid req".to_string())),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let result = service.unregister(42);

        assert!(result.is_ok());
    }

    #[test]
    fn test_unregister_reporter_for_admin() {
        let usecase = Rc::new(RefCell::new(StubPersonUsecase {
            dao: DummyPersonDao,
            entry_result: Ok(1),   // 使わない
            find_result: Ok(None), // 使わない
            entry_and_verify_result: Ok((
                42,
                PersonDto::new("Alice", date(2012, 11, 2), None, None, 0),
            )), // 使わない
            collect_result: Ok(vec![]), // 使わない
            death_result: Ok(()),  // 使わない
            remove_result: Err(UsecaseError::RemovePersonFailed(DaoError::DeleteError(
                "valid dao".to_string(),
            ))),
        }));
        let reporter = StubReporter {
            admin_result: Err(ReporterError::Unavailable("valid req".to_string())),
            entry_person_result: Ok(()),
            death_person_result: Ok(()),
            unregister_person_result: Ok(()),
            otherwise_result: Ok(()),
        };
        let mut service = TargetPersonService {
            usecase: usecase.clone(),
            reporter,
        };

        let result = service.unregister(42);
        let expected = usecase.borrow().remove_result.clone().unwrap_err();

        assert_eq!(result, Err(ServiceError::TransactionFailed(expected)));
    }
}
