use chrono::NaiveDate;
use log::{error, trace, warn};
use std::rc::Rc;

use crate::cache::PersonCao;
use crate::domain::PersonId;
use crate::dto::PersonDto;
use crate::location;
use crate::reporter::{Level, Reporter};
use crate::service::{InvalidErrorKind, PersonOutputBoundary, PersonService, ServiceError};

pub trait PersonCachedService<'a, Conn, Ctx>: PersonService<'a, Ctx> {
    type C: PersonCao<Conn>;

    fn get_cao(&self) -> Self::C;

    fn register(
        &'a mut self,
        name: &str,
        birth_date: NaiveDate,
        death_date: Option<NaiveDate>,
        data: &str,
    ) -> Result<(PersonId, PersonDto), ServiceError> {
        trace!(
            "cached register: {} {} {:?} {}",
            name,
            birth_date,
            death_date,
            data
        );
        let cao = self.get_cao();
        let reporter = self.get_reporter();

        let result = PersonService::register(self, name, birth_date, death_date, data);
        trace!("register person to db: {:?}", result);

        if let Ok((id, person)) = &result {
            if let Err(e) = cao.run_tx(cao.load(*id, &person)) {
                // ここはエラーを返す必要はない
                warn!("failed to load person to cache: {}", e);
                if let Err(e) = reporter.send_report(
                    Level::Error,
                    "admin",
                    "cache service not available",
                    location!(),
                ) {
                    error!("reporter service not available: {}", e);
                }
            }

            trace!("load person to cache: {:?}", person);
        }

        result
    }

    fn find(&'a mut self, id: PersonId) -> Result<Option<PersonDto>, ServiceError> {
        trace!("cached find: {}", id);
        let cao = self.get_cao();
        let reporter = self.get_reporter();

        // if the person is found in the cache, return it
        if let Ok(Some(p)) = cao.run_tx(cao.find(id)) {
            trace!("cache hit!: {}", id);
            return Ok(Some(p));
        }
        trace!("cache miss!: {}", id);

        let result = PersonService::find(self, id)?;
        trace!("find person in db: {:?}", result);

        // if the person is found in the db, load it to the cache
        if let Some(person) = &result {
            if let Err(e) = cao.run_tx(cao.load(id, &person)) {
                // ここはエラーを返す必要はない
                warn!("failed to load person to cache: {}", e);
                if let Err(e) = reporter.send_report(
                    Level::Error,
                    "admin",
                    "cache service not available",
                    location!(),
                ) {
                    error!("reporter service not available: {}", e);
                }
            } else {
                trace!("load person to cache: {:?}", person);
            }
        }

        Ok(result)
    }

    fn batch_import(
        &'a mut self,
        persons: Vec<PersonDto>,
        out_port: Rc<impl PersonOutputBoundary<(u64, u64), ServiceError>>,
    ) -> Result<Vec<PersonId>, ServiceError> {
        if persons.is_empty() {
            return Err(ServiceError::InvalidRequest(
                InvalidErrorKind::EmptyArgument,
            ));
        }

        trace!("cached batch import: {:?}", persons);
        let cao = self.get_cao();
        let reporter = self.get_reporter();

        let ids = PersonService::batch_import(self, persons.clone().into_iter(), out_port.clone())?;

        // load all persons to the cache
        for (id, person) in ids.iter().zip(persons.iter()) {
            // ここはエラーを返す必要はない
            if let Err(e) = cao.run_tx(cao.load(*id, &person)) {
                warn!("failed to load person to cache: {}", e);
                if let Err(e) = reporter.send_report(
                    Level::Error,
                    "admin",
                    "cache service not available",
                    location!(),
                ) {
                    error!("reporter service not available: {}", e);
                }
                return Ok(ids);
            }
        }
        trace!("load persons to cache: {:?}", ids);

        Ok(ids)
    }

    fn list_all(&'a mut self) -> Result<Vec<(PersonId, PersonDto)>, ServiceError> {
        trace!("cached list all");
        let cao = self.get_cao();
        let reporter = self.get_reporter();

        let result = PersonService::list_all(self)?;

        // load all persons to the cache
        for (id, person) in result.iter() {
            // ここはエラーを返す必要はない
            if let Err(e) = cao.run_tx(cao.load(*id, &person)) {
                warn!("failed to load person to cache: {}", e);
                if let Err(e) = reporter.send_report(
                    Level::Error,
                    "admin",
                    "cache service not available",
                    location!(),
                ) {
                    error!("reporter service not available: {}", e);
                }
                return Ok(result);
            }
        }
        trace!("load all persons to cache");

        Ok(result)
    }

    fn death(&'a mut self, id: PersonId, death_date: NaiveDate) -> Result<(), ServiceError> {
        trace!("cached death: {} {}", id, death_date);
        let cao = self.get_cao();
        let reporter = self.get_reporter();

        let _ = PersonService::death(self, id, death_date)?;
        trace!("update death date in db: {} {}", id, death_date);

        // even if delete from db failed below, this cache clear is not a matter.
        if let Err(e) = cao.run_tx(cao.unload(id)) {
            // ここはエラーを返す必要はない
            warn!("failed to unload person from cache: {}", e);
            if let Err(e) = reporter.send_report(
                Level::Error,
                "admin",
                "cache service not available",
                location!(),
            ) {
                error!("reporter service not available: {}", e);
            }
        } else {
            trace!("unload from cache: {}", id);
        }

        Ok(())
    }

    fn unregister(&'a mut self, id: PersonId) -> Result<(), ServiceError> {
        trace!("cached unregister: {}", id);
        let cao = self.get_cao();
        let reporter = self.get_reporter();

        // even if delete from db failed below, this cache clear is not a matter.
        if let Err(e) = cao.run_tx(cao.unload(id)) {
            // ここはエラーを返す必要はない
            warn!("failed to unload person from cache: {}", e);
            if let Err(e) = reporter.send_report(
                Level::Error,
                "admin",
                "cache service not available",
                location!(),
            ) {
                error!("reporter service not available: {}", e);
            }
        } else {
            trace!("unload from cache: {}", id);
        }

        let result = PersonService::unregister(self, id);
        trace!("delete from db: {}", id);

        result
    }
}

// # フェイクテスト
//
// ## 目的
//
//   CachedService の正常系のテストを行う
//   CachedService の各メソッドが、 Cache と Service とから通常期待される結果を受け取ったときに
//   適切にふるまうことを保障する
//
// ## 方針
//
//   Cache のフェイクと Service のフェイクに対して CachedService を実行し、その結果を確認する
//   フェイクはテスト時の比較チェックのしやすさを考慮して HashMap ではなく Vec で登録データを保持する
//   データ数は多くないので、Vec でリニアサーチしても十分な速度が出ると考える
//
// ## 実装
//
//                                 Test Double
//        +----------------+      +----------------+.oOo.+---------------+
//        | Cached Service |      | Fake Service   |     | Dummy Usecase |
//        | ============== |      | ============== |     | ============= |
//        |                |      |                |     |               |
//   --c->| ---c---------> |---+->| ---+           |     |               |
//     |  |    |           |   |  |    | fake logic|     |               |
//   <-c--| <--c---------- |<-+|--| <--+           |     |               |
//     |  +----|-----------+  ||  +----------------+	  +--------------+
//     |       |              ||   Test Double
//     |       |              ||  +----------------+
//     |       +-- テスト対象 ||  | Fake Cache     |
//     |                      ||  | ============== |
//     +-- ここを確認する     ||  |                |
//                            |+->| ---+ fake logic|
//                            ||  |    |           |
//                            +|--| <--+           |
//                            ||  +----------------+
//                            ||   Test Double
//                            ||  +----------------+
//                            ||  | Fake Reporter  |
//                            ||  | ============== |
//                            ||  |                |
//                            |+->| ---+ fake logic|
//                            |   |    |           |
//                            +---| <--+           |
//                                +----------------+
//
//   1. ダミーの DAO 構造体、ユースケース構造体を用意する
//      この構造体は実質使われないが、 Service に必要なので用意する
//   2. CachedService のメソッド呼び出しに対して、期待される結果を返す Service の実装を用意する
//      この Service 実装はフェイクなので、間接的な入力と間接的な出力が整合するようにする
//   3. CachedService のメソッド呼び出しに対して、期待される結果を返す Cache 構造体を用意する
//      この Cache 構造体はフェイクなので、間接的な入力と間接的な出力が整合するようにする
//   4. ダミーの Reporter 構造体を用意する
//   5. CachedService をここまでに用意したフェイクとダミーで構築する
//   6. Service のメソッドを呼び出す
//   7. Service からの戻り値を検証する
//
// ## 注意
//
//   1. このテストは CachedService の実装を保障するものであって、Service や Cache の実装を保障するものではない
//   2. 同様にこのテストは ユースケースや DAO の実装を保障するものではない
//   3. CachedService とフェイクとなる Fake Service とは構造体としては同一になっている
//
#[cfg(test)]
mod fake_tests {
    use chrono::NaiveDate;
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::{cell::RefCell, collections::VecDeque};
    use uuid::Uuid;

    use crate::{
        cache::{CaoError, PersonCao},
        cached_service::PersonCachedService,
        dao::{DaoError, HavePersonDao, PersonDao},
        domain::{date, PersonId, Revision},
        dto::PersonDto,
        location::Location,
        reporter::{Level, Reporter, ReporterError},
        usecase::{PersonUsecase, UsecaseError},
    };

    struct DummyPersonDao;
    impl PersonDao<()> for DummyPersonDao {
        fn insert(
            &self,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(Uuid::now_v7()))
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

    struct DummyPersonUsecase {
        dao: DummyPersonDao,
    }
    impl HavePersonDao<()> for DummyPersonUsecase {
        fn get_dao(&self) -> &impl PersonDao<()> {
            &self.dao
        }
    }
    impl PersonUsecase<()> for DummyPersonUsecase {
        fn entry<'a>(
            &'a mut self,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(Uuid::now_v7()))
        }
        fn find<'a>(
            &'a mut self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(None))
        }
        fn entry_and_verify<'a>(
            &'a mut self,
            person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (PersonId, PersonDto), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok((Uuid::now_v7(), person)))
        }
        fn collect<'a>(
            &'a mut self,
        ) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(vec![]))
        }
        fn death<'a>(
            &'a mut self,
            _id: PersonId,
            _date: NaiveDate,
        ) -> impl tx_rs::Tx<(), Item = (), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(()))
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

    /// テスト用のフェイクサービスです。
    /// Clone できるようにしていないので基本は Rc でラップしていません。
    /// FakePersonCao のみ get_cao() で clone されるため内部データを Rc でラップしています。
    struct TargetPersonService {
        next_id: RefCell<VecDeque<PersonId>>,
        db: RefCell<HashMap<PersonId, PersonDto>>,
        usecase: Rc<RefCell<DummyPersonUsecase>>,
        cao: FakePersonCao,
    }
    // フェイクのサービス実装です。ユースケースより先はダミーです。
    impl crate::service::PersonService<'_, ()> for TargetPersonService {
        type U = DummyPersonUsecase;
        type N = DummyReporter;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, crate::service::ServiceError>
        where
            F: FnOnce(&mut Self::U, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(crate::service::ServiceError::TransactionFailed)
        }

        fn get_reporter(&self) -> Self::N {
            DummyReporter
        }

        fn register(
            &'_ mut self,
            name: &str,
            birth_date: NaiveDate,
            death_date: Option<NaiveDate>,
            data: &str,
        ) -> Result<(PersonId, PersonDto), crate::service::ServiceError> {
            let id = self.next_id.borrow_mut().pop_front().unwrap();

            let person = PersonDto::new(name, birth_date, death_date, Some(data), 0);

            self.db.borrow_mut().insert(id, person.clone());
            Ok((id, person))
        }

        fn find(
            &'_ mut self,
            id: PersonId,
        ) -> Result<Option<PersonDto>, crate::service::ServiceError> {
            Ok(self.db.borrow().get(&id).cloned())
        }

        fn batch_import(
            &'_ mut self,
            persons: impl Iterator<Item = PersonDto>,
            _out_port: Rc<
                impl crate::service::PersonOutputBoundary<(u64, u64), crate::service::ServiceError>,
            >,
        ) -> Result<Vec<PersonId>, crate::service::ServiceError> {
            let mut ids = vec![];
            for person in persons {
                let id = self.next_id.borrow_mut().pop_front().unwrap();

                self.db.borrow_mut().insert(id, person.clone());
                ids.push(id);
            }
            Ok(ids)
        }

        fn list_all(
            &'_ mut self,
        ) -> Result<Vec<(PersonId, PersonDto)>, crate::service::ServiceError> {
            Ok(self
                .db
                .borrow()
                .iter()
                .map(|(id, person)| (*id, person.clone()))
                .collect())
        }

        fn unregister(&'_ mut self, id: PersonId) -> Result<(), crate::service::ServiceError> {
            self.db.borrow_mut().remove(&id);
            Ok(())
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct FakePersonCao {
        cache: Rc<RefCell<HashMap<PersonId, PersonDto>>>,
    }
    impl PersonCao<()> for FakePersonCao {
        fn get_conn(&self) -> Result<(), CaoError> {
            Ok(())
        }
        fn run_tx<T, F>(&self, f: F) -> Result<T, CaoError>
        where
            F: tx_rs::Tx<(), Item = T, Err = CaoError>,
        {
            f.run(&mut ())
        }
        fn find(
            &self,
            id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = CaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(self.cache.borrow().get(&id).cloned()))
        }
        fn load(
            &self,
            id: PersonId,
            person: &PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (), Err = CaoError> {
            tx_rs::with_tx(move |&mut ()| {
                self.cache.borrow_mut().insert(id, person.clone());
                Ok(())
            })
        }
        fn unload(&self, id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = CaoError> {
            tx_rs::with_tx(move |&mut ()| {
                self.cache.borrow_mut().remove(&id);
                Ok(())
            })
        }
    }
    impl PersonCachedService<'_, (), ()> for TargetPersonService {
        type C = FakePersonCao;

        fn get_cao(&self) -> Self::C {
            self.cao.clone()
        }
    }

    struct DummyPersonOutputBoundary;
    impl crate::service::PersonOutputBoundary<(u64, u64), crate::service::ServiceError>
        for DummyPersonOutputBoundary
    {
        fn started(&self) {}
        fn in_progress(&self, _progress: (u64, u64)) {}
        fn completed(&self) {}
        fn aborted(&self, _err: crate::service::ServiceError) {}
    }

    #[test]
    fn test_register() {
        let id = Uuid::now_v7();
        let mut service = TargetPersonService {
            next_id: RefCell::new(VecDeque::from(vec![id])),
            db: RefCell::new(HashMap::new()),
            usecase: Rc::new(RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            })),
            cao: FakePersonCao {
                cache: RefCell::new(HashMap::new()).into(),
            },
        };

        let expected = PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0);
        let result = service.register("Alice", date(2000, 1, 1), None, "Alice is here");

        assert!(result.is_ok());
        assert_eq!(result, Ok((id, expected)));
    }

    #[test]
    fn test_find() {
        let id1 = Uuid::now_v7();
        let id2 = Uuid::now_v7();
        let mut service = TargetPersonService {
            next_id: RefCell::new(VecDeque::from(vec![id1])),
            db: RefCell::new(HashMap::new()),
            usecase: Rc::new(RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            })),
            cao: FakePersonCao {
                cache: RefCell::new(HashMap::new()).into(),
            },
        };

        let result = service.find(id1);

        assert!(result.is_ok());
        assert_eq!(result, Ok(None), "not found");

        let mut service = TargetPersonService {
            next_id: RefCell::new(VecDeque::from(vec![id2])),
            db: RefCell::new(HashMap::new()),
            usecase: Rc::new(RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            })),
            cao: FakePersonCao {
                cache: RefCell::new(
                    vec![(
                        id1,
                        PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
                    )]
                    .into_iter()
                    .collect(),
                )
                .into(),
            },
        };

        let expected = PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0);
        let result = service.find(id1);

        assert!(result.is_ok());
        assert_eq!(result, Ok(Some(expected)), "hit cache");

        let mut service = TargetPersonService {
            next_id: RefCell::new(VecDeque::from(vec![id2])),
            db: RefCell::new(
                vec![(
                    id1,
                    PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
                )]
                .into_iter()
                .collect(),
            ),
            usecase: Rc::new(RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            })),
            cao: FakePersonCao {
                cache: RefCell::new(HashMap::new()).into(),
            },
        };

        let expected = PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0);
        let result = service.find(id1);

        assert!(result.is_ok());
        assert_eq!(result, Ok(Some(expected)), "found db");
    }

    #[test]
    fn test_batch_import() {
        let id1 = Uuid::now_v7();
        let id2 = Uuid::now_v7();
        let mut service = TargetPersonService {
            next_id: RefCell::new(VecDeque::from(vec![id1, id2])),
            db: RefCell::new(HashMap::new()),
            usecase: Rc::new(RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            })),
            cao: FakePersonCao {
                cache: RefCell::new(HashMap::new()).into(),
            },
        };

        let result = service.batch_import(
            vec![
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
                PersonDto::new("Bob", date(2000, 1, 2), None, Some("Bob is here"), 0),
            ],
            Rc::new(DummyPersonOutputBoundary),
        );

        assert!(result.is_ok());
        assert_eq!(result, Ok(vec![id1, id2]));
    }

    #[test]
    fn test_list_all() {
        let id1 = Uuid::now_v7();
        let id2 = Uuid::now_v7();
        let mut service = TargetPersonService {
            next_id: RefCell::new(VecDeque::from(vec![Uuid::now_v7()])),
            db: RefCell::new(
                vec![
                    (
                        id1,
                        PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
                    ),
                    (
                        id2,
                        PersonDto::new("Bob", date(2000, 1, 2), None, Some("Bob is here"), 0),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            usecase: Rc::new(RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            })),
            cao: FakePersonCao {
                cache: RefCell::new(HashMap::new()).into(),
            },
        };

        let result = service.list_all();

        assert!(result.is_ok());
        assert_eq!(result.clone().map(|v| v.len()), Ok(2), "list from db");
    }

    #[test]
    fn test_death() {
        let id = Uuid::now_v7();
        let mut service = TargetPersonService {
            next_id: RefCell::new(VecDeque::from(vec![Uuid::now_v7()])),
            db: RefCell::new(
                vec![(
                    id,
                    PersonDto::new(
                        "poor man",
                        date(2000, 1, 1),
                        None,
                        Some("poor man will be dead"),
                        0,
                    ),
                )]
                .into_iter()
                .collect(),
            ),
            usecase: Rc::new(RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            })),
            cao: FakePersonCao {
                cache: RefCell::new(HashMap::new()).into(),
            },
        };

        let result = service.death(id, date(2030, 11, 22));

        assert!(result.is_ok());
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_unregister() {
        let id1 = Uuid::now_v7();
        let id2 = Uuid::now_v7();
        let mut service = TargetPersonService {
            next_id: RefCell::new(VecDeque::from(vec![Uuid::now_v7()])),
            db: RefCell::new(
                vec![
                    (
                        id1,
                        PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
                    ),
                    (
                        id2,
                        PersonDto::new("Bob", date(2000, 1, 2), None, Some("Bob is here"), 0),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            usecase: Rc::new(RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            })),
            cao: FakePersonCao {
                cache: RefCell::new(HashMap::new()).into(),
            },
        };

        let result = service.unregister(id1);

        assert!(result.is_ok());
        assert_eq!(result, Ok(()));
    }
}

// # スパイテスト(モック利用)
//
// ## 目的
//
//   CachedService の各メソッドが、 Cache, Reporter と Service のメソッドを適切に呼び出していることを保障する
//   つまり、
//    1. 必要なメソッドを必要回数だけ呼び出していること
//    2. 不必要なメソッドを呼び出していないこと
//    3. CachedService に渡った引数が適切に Cache, Reporter や Service のメソッドに渡されていること
//   を保障する
//
// ## 方針
//
//   スパイ Service と スパイ Cache, スパイ Reporter は呼び出されるたびに、それらを全て記録する
//   ただし、 Service の返り値が Cache や Reporter に使われたりその逆があるため、
//   各スパイは返り値も制御する必要がある
//   よってスタブを兼ねる必要があるため、それぞれをモックとして実装する
//   各メソッドの呼び出された記録をテストの最後で確認する
//
// ## 実装
//
//                                   Test Double
//        +----------------+        +------------------+.oOo.+----------------+
//        | Cached Service |        | Spy Service      |     | Dummy Usecase  |
//        | ============== |        | ============     |     | ============== |
//        |                |        |                  |     |                |
//   ---->| -------------> |--c+--->| --> [ c ] request|     |                |
//        |                |  ||    |       |    log   |     |                |
//   <----| <------------- |<-||----| <--   |          |     |                |
//        +----------------+  ||    +-------|----------+     +----------------+
//                            ||            +-- ここを確認する
//              テスト対象 ---+|     Test Double
//                             |    +------------------+
//                             |    | Spy Cache        |
//                             |    | ============     |
//                             |    |                  |
//                             +-c->| --> [ c ] request|
//                             | |  |       |    log   |
//              テスト対象 ----|-+  | <--   |          |
//                             |    +-------|----------+
//                             |            |
//                             |            +-- ここを確認する
//                             |     Test Double
//                             |    +------------------+
//                             |    | Spy Reporter     |
//                             |    | ============     |
//                             |    |                  |
//                             +-c->| --> [ c ] request|
//                               |  |       |    log   |
//              テスト対象 ------+  | <--   |          |
//                                  +-------|----------+
//                                          |
//                                          +-- ここを確認する
//
//   1. ダミーの DAO 構造体、ユースケース構造体を用意する
//      この構造体は実質使われないが、 Service に必要なので用意する
//   2. メソッド呼び出しを記録しつつ、設定された返り値を返すモック Service を実装する
//   3. メソッド呼び出しを記録しつつ、設定された返り値を返すモック Cache を実装する
//   4. メソッド呼び出しを記録するスパイ Reporter を実装する
//   5. CachedService をここまでに用意したモック、スパイとダミーで構築する
//   6. CachedService のメソッドを呼び出す
//   7. Cache, Reporter と Service の記録を検証する
//
// ## 注意
//
//   1. このテストは CachedService の実装を保障するものであって、Service や Cache, Reporter の実装を保障するものではない
//   2. このテストは CachedService のメソッドが不適切な Cache メソッドや Reporter メソッド あるいは Service メソッド呼び出しをしていないことを保障するものであって Cache, Reporter や Service の不適切な処理をしていないことを保障するものではない
//   3. このテストでは Cache, Reporter と Service のメソッド呼び出し順序については検証しない (将来的に検証することを拒否しない)
//   4. CachedService とスパイとなる Spy Service とは構造体としては同一になっている
//
#[cfg(test)]
mod spy_tests {
    use chrono::NaiveDate;
    use std::cell::RefCell;
    use std::rc::Rc;
    use uuid::Uuid;

    use crate::{
        cache::{CaoError, PersonCao},
        cached_service::PersonCachedService,
        dao::{DaoError, HavePersonDao, PersonDao},
        domain::{date, PersonId, Revision},
        dto::PersonDto,
        location::Location,
        reporter::{Level, Reporter, ReporterError},
        usecase::{PersonUsecase, UsecaseError},
    };

    struct DummyPersonDao;
    impl PersonDao<()> for DummyPersonDao {
        fn insert(
            &self,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(Uuid::now_v7()))
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

    struct DummyPersonUsecase {
        dao: DummyPersonDao,
    }
    impl HavePersonDao<()> for DummyPersonUsecase {
        fn get_dao(&self) -> &impl PersonDao<()> {
            &self.dao
        }
    }
    impl PersonUsecase<()> for DummyPersonUsecase {
        fn entry<'a>(
            &'a mut self,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(Uuid::now_v7()))
        }
        fn find<'a>(
            &'a mut self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(None))
        }
        fn entry_and_verify<'a>(
            &'a mut self,
            person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (PersonId, PersonDto), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok((Uuid::now_v7(), person)))
        }
        fn collect<'a>(
            &'a mut self,
        ) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(vec![]))
        }
        fn death<'a>(
            &'a mut self,
            _id: PersonId,
            _date: NaiveDate,
        ) -> impl tx_rs::Tx<(), Item = (), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(()))
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

    #[derive(Debug, Clone)]
    struct SpyReporter {
        report: Rc<RefCell<Vec<(Level, String, String)>>>,
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
            level: Level,
            to: &str,
            message: &str,
            _loc: Location,
        ) -> Result<(), ReporterError> {
            self.report
                .borrow_mut()
                .push((level, to.to_string(), message.to_string()));

            // 返り値に意味はない
            Ok(())
        }
    }

    /// テスト用のスパイサービスです。
    struct TargetPersonService {
        register: RefCell<Vec<(String, NaiveDate, Option<NaiveDate>, Option<String>)>>,
        register_result: Result<(PersonId, PersonDto), crate::service::ServiceError>,
        find: RefCell<Vec<PersonId>>,
        find_result: Result<Option<PersonDto>, crate::service::ServiceError>,
        batch_import: RefCell<Vec<Vec<PersonDto>>>,
        batch_import_result: Result<Vec<PersonId>, crate::service::ServiceError>,
        list_all: RefCell<i32>,
        list_all_result: Result<Vec<(PersonId, PersonDto)>, crate::service::ServiceError>,
        death: RefCell<Vec<(PersonId, NaiveDate)>>,
        death_result: Result<(), crate::service::ServiceError>,
        unregister: RefCell<Vec<PersonId>>,
        unregister_result: Result<(), crate::service::ServiceError>,

        usecase: RefCell<DummyPersonUsecase>,
        cao: MockPersonCao,
        reporter: SpyReporter,
    }
    // スパイサービス実装です。ユースケースより先はダミーです。
    impl crate::service::PersonService<'_, ()> for TargetPersonService {
        type U = DummyPersonUsecase;
        type N = SpyReporter;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, crate::service::ServiceError>
        where
            F: FnOnce(&mut Self::U, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(crate::service::ServiceError::TransactionFailed)
        }

        fn get_reporter(&self) -> Self::N {
            self.reporter.clone()
        }

        fn register(
            &'_ mut self,
            name: &str,
            birth_date: NaiveDate,
            death_date: Option<NaiveDate>,
            data: &str,
        ) -> Result<(PersonId, PersonDto), crate::service::ServiceError> {
            self.register.borrow_mut().push((
                name.to_string(),
                birth_date,
                death_date,
                Some(data.to_string()),
            ));
            self.register_result.clone()
        }

        fn find(
            &'_ mut self,
            id: PersonId,
        ) -> Result<Option<PersonDto>, crate::service::ServiceError> {
            self.find.borrow_mut().push(id);
            self.find_result.clone()
        }

        fn batch_import(
            &'_ mut self,
            persons: impl Iterator<Item = PersonDto>,
            _out_port: Rc<
                impl crate::service::PersonOutputBoundary<(u64, u64), crate::service::ServiceError>,
            >,
        ) -> Result<Vec<PersonId>, crate::service::ServiceError> {
            self.batch_import.borrow_mut().push(persons.collect());
            self.batch_import_result.clone()
        }

        fn list_all(
            &'_ mut self,
        ) -> Result<Vec<(PersonId, PersonDto)>, crate::service::ServiceError> {
            *self.list_all.borrow_mut() += 1;
            self.list_all_result.clone()
        }

        fn death(
            &'_ mut self,
            id: PersonId,
            date: NaiveDate,
        ) -> Result<(), crate::service::ServiceError> {
            self.death.borrow_mut().push((id, date));
            self.death_result.clone()
        }

        fn unregister(&'_ mut self, id: PersonId) -> Result<(), crate::service::ServiceError> {
            self.unregister.borrow_mut().push(id);
            self.unregister_result.clone()
        }
    }
    // スパイキャッシュ実装です
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MockPersonCao {
        find: Rc<RefCell<Vec<PersonId>>>,
        find_result: Result<Option<PersonDto>, CaoError>,
        load: Rc<RefCell<Vec<(PersonId, PersonDto)>>>,
        load_result: Result<(), CaoError>,
        unload: Rc<RefCell<Vec<PersonId>>>,
        unload_result: Result<(), CaoError>,
    }
    impl PersonCao<()> for MockPersonCao {
        fn get_conn(&self) -> Result<(), CaoError> {
            Ok(())
        }
        fn run_tx<T, F>(&self, f: F) -> Result<T, CaoError>
        where
            F: tx_rs::Tx<(), Item = T, Err = CaoError>,
        {
            f.run(&mut ())
        }
        fn find(
            &self,
            id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = CaoError> {
            tx_rs::with_tx(move |&mut ()| {
                self.find.borrow_mut().push(id);
                self.find_result.clone()
            })
        }
        fn load(
            &self,
            id: PersonId,
            person: &PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (), Err = CaoError> {
            tx_rs::with_tx(move |&mut ()| {
                self.load.borrow_mut().push((id, person.clone()));
                self.load_result.clone()
            })
        }
        fn unload(&self, id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = CaoError> {
            tx_rs::with_tx(move |&mut ()| {
                self.unload.borrow_mut().push(id);
                self.unload_result.clone()
            })
        }
    }
    impl PersonCachedService<'_, (), ()> for TargetPersonService {
        type C = MockPersonCao;

        fn get_cao(&self) -> Self::C {
            self.cao.clone()
        }
    }

    struct DummyPersonOutputBoundary;
    impl crate::service::PersonOutputBoundary<(u64, u64), crate::service::ServiceError>
        for DummyPersonOutputBoundary
    {
        fn started(&self) {}
        fn in_progress(&self, _progress: (u64, u64)) {}
        fn completed(&self) {}
        fn aborted(&self, _err: crate::service::ServiceError) {}
    }

    #[test]
    fn test_register() {
        let id = Uuid::now_v7();
        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find: RefCell::new(vec![]),
            find_result: Ok(None), // 使われない
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![]), // 使われない
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![]), // 使われない
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Ok(None), // 使われない
                load: Rc::new(RefCell::new(vec![])),
                load_result: Ok(()), // 使われない
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Ok(()), // 使われない
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.register("Alice", date(2000, 1, 1), None, "Alice is here");
        assert_eq!(
            *service.register.borrow(),
            vec![(
                "Alice".to_string(),
                date(2000, 1, 1),
                None,
                Some("Alice is here".to_string())
            )]
        );
        assert_eq!(*service.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![] as Vec<Vec<PersonDto>>
        );
        assert_eq!(*service.list_all.borrow(), 0);
        assert_eq!(
            *service.death.borrow(),
            vec![] as Vec<(PersonId, NaiveDate)>
        );
        assert_eq!(*service.unregister.borrow(), vec![] as Vec<PersonId>);

        assert_eq!(*service.cao.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.cao.load.borrow(),
            vec![(
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0)
            )]
        );
        assert_eq!(*service.cao.unload.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.reporter.report.borrow(),
            vec![] as Vec<(Level, String, String)>
        );

        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find: RefCell::new(vec![]),
            find_result: Ok(None), // 使われない
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![]), // 使われない
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![]), // 使われない
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Ok(None), // 使われない
                load: Rc::new(RefCell::new(vec![])),
                load_result: Err(CaoError::Unavailable("valid cao".to_string())),
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Ok(()), // 使われない
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.register("Alice", date(2000, 1, 1), None, "Alice is here");
        assert_eq!(
            *service.register.borrow(),
            vec![(
                "Alice".to_string(),
                date(2000, 1, 1),
                None,
                Some("Alice is here".to_string())
            )]
        );
        assert_eq!(*service.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![] as Vec<Vec<PersonDto>>
        );
        assert_eq!(*service.list_all.borrow(), 0);
        assert_eq!(
            *service.death.borrow(),
            vec![] as Vec<(PersonId, NaiveDate)>
        );
        assert_eq!(*service.unregister.borrow(), vec![] as Vec<PersonId>);

        assert_eq!(*service.cao.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.cao.load.borrow(),
            vec![(
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0)
            )]
        );
        assert_eq!(*service.cao.unload.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.reporter.report.borrow(),
            vec![(
                Level::Error,
                "admin".to_string(),
                "cache service not available".to_string()
            )],
        );
    }

    #[test]
    fn test_find() {
        let id = Uuid::now_v7();
        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                Uuid::now_v7(),
                PersonDto::new("", date(2000, 1, 1), None, Some(""), 0),
            )), // 使われない
            find: RefCell::new(vec![]),
            find_result: Ok(None), // 使われない
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![]), // 使われない
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![]), // 使われない
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Ok(Some(PersonDto::new(
                    "Alice",
                    date(2000, 1, 1),
                    None,
                    Some("Alice is here"),
                    0,
                ))),
                load: Rc::new(RefCell::new(vec![])),
                load_result: Ok(()), // 使われない
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Ok(()), // 使われない
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.find(id);
        assert_eq!(*service.register.borrow(), vec![]);
        assert_eq!(*service.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![] as Vec<Vec<PersonDto>>
        );
        assert_eq!(*service.list_all.borrow(), 0);
        assert_eq!(*service.unregister.borrow(), vec![] as Vec<PersonId>);

        assert_eq!(*service.cao.find.borrow(), vec![id]);
        assert_eq!(
            *service.cao.load.borrow(),
            vec![] as Vec<(PersonId, PersonDto)>
        );
        assert_eq!(*service.cao.unload.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.reporter.report.borrow(),
            vec![] as Vec<(Level, String, String)>
        );

        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                Uuid::now_v7(),
                PersonDto::new("", date(2000, 1, 1), None, Some(""), 0),
            )), // 使われない
            find: RefCell::new(vec![]),
            find_result: Ok(Some(PersonDto::new(
                "Alice",
                date(2000, 1, 1),
                None,
                Some("Alice is here"),
                0,
            ))),
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![]), // 使われない
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![]), // 使われない
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Ok(None),
                load: Rc::new(RefCell::new(vec![])),
                load_result: Ok(()), // 使われない
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Ok(()), // 使われない
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.find(id);
        assert_eq!(*service.register.borrow(), vec![]);
        assert_eq!(*service.find.borrow(), vec![id]);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![] as Vec<Vec<PersonDto>>
        );
        assert_eq!(*service.list_all.borrow(), 0);
        assert_eq!(
            *service.death.borrow(),
            vec![] as Vec<(PersonId, NaiveDate)>
        );
        assert_eq!(*service.unregister.borrow(), vec![] as Vec<PersonId>);

        assert_eq!(*service.cao.find.borrow(), vec![id]);
        assert_eq!(
            *service.cao.load.borrow(),
            vec![(
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0)
            )]
        );
        assert_eq!(*service.cao.unload.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.reporter.report.borrow(),
            vec![] as Vec<(Level, String, String)>
        );

        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                Uuid::now_v7(),
                PersonDto::new("", date(2000, 1, 1), None, Some(""), 0),
            )), // 使われない
            find: RefCell::new(vec![]),
            find_result: Ok(Some(PersonDto::new(
                "Alice",
                date(2000, 1, 1),
                None,
                Some("Alice is here"),
                0,
            ))),
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![]), // 使われない
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![]), // 使われない
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Err(CaoError::Unavailable("valid cao".to_string())),
                load: Rc::new(RefCell::new(vec![])),
                load_result: Ok(()), // 使われない
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Ok(()), // 使われない
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.find(id);
        assert_eq!(*service.register.borrow(), vec![]);
        assert_eq!(*service.find.borrow(), vec![id]);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![] as Vec<Vec<PersonDto>>
        );
        assert_eq!(*service.list_all.borrow(), 0);
        assert_eq!(
            *service.death.borrow(),
            vec![] as Vec<(PersonId, NaiveDate)>
        );
        assert_eq!(*service.unregister.borrow(), vec![] as Vec<PersonId>);

        assert_eq!(*service.cao.find.borrow(), vec![id]);
        assert_eq!(
            *service.cao.load.borrow(),
            vec![(
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0)
            )]
        );
        assert_eq!(*service.cao.unload.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.reporter.report.borrow(),
            vec![] as Vec<(Level, String, String)>
        );

        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                Uuid::now_v7(),
                PersonDto::new("", date(2000, 1, 1), None, Some(""), 0),
            )), // 使われない
            find: RefCell::new(vec![]),
            find_result: Ok(Some(PersonDto::new(
                "Alice",
                date(2000, 1, 1),
                None,
                Some("Alice is here"),
                0,
            ))),
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![]), // 使われない
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![]), // 使われない
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Ok(None),
                load: Rc::new(RefCell::new(vec![])),
                load_result: Err(CaoError::Unavailable("valid cao".to_string())),
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Ok(()), // 使われない
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.find(id);
        assert_eq!(*service.register.borrow(), vec![]);
        assert_eq!(*service.find.borrow(), vec![id]);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![] as Vec<Vec<PersonDto>>
        );
        assert_eq!(*service.list_all.borrow(), 0);
        assert_eq!(
            *service.death.borrow(),
            vec![] as Vec<(PersonId, NaiveDate)>
        );
        assert_eq!(*service.unregister.borrow(), vec![] as Vec<PersonId>);

        assert_eq!(*service.cao.find.borrow(), vec![id]);
        assert_eq!(
            *service.cao.load.borrow(),
            vec![(
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0)
            )]
        );
        assert_eq!(*service.cao.unload.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.reporter.report.borrow(),
            vec![(
                Level::Error,
                "admin".to_string(),
                "cache service not available".to_string()
            )],
        );
    }

    #[test]
    fn test_batch_import() {
        let id1 = Uuid::now_v7();
        let id2 = Uuid::now_v7();
        let id3 = Uuid::now_v7();
        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                Uuid::now_v7(),
                PersonDto::new("", date(2000, 1, 1), None, Some(""), 0),
            )), // 使われない
            find: RefCell::new(vec![]),
            find_result: Ok(None), // 使われない
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![id1, id2, id3]),
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![]), // 使われない
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Ok(None), // 使われない
                load: Rc::new(RefCell::new(vec![])),
                load_result: Ok(()), // 使われない
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Ok(()), // 使われない
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.batch_import(
            vec![
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is sender"), 0),
                PersonDto::new("Bob", date(2001, 2, 2), None, Some("Bob is receiver"), 0),
                PersonDto::new("Eve", date(2002, 3, 3), None, Some("Eve is interceptor"), 0),
            ],
            Rc::new(DummyPersonOutputBoundary),
        );
        assert_eq!(*service.register.borrow(), vec![]);
        assert_eq!(*service.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![vec![
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is sender"), 0),
                PersonDto::new("Bob", date(2001, 2, 2), None, Some("Bob is receiver"), 0),
                PersonDto::new("Eve", date(2002, 3, 3), None, Some("Eve is interceptor"), 0),
            ]]
        );
        assert_eq!(*service.list_all.borrow(), 0);
        assert_eq!(
            *service.death.borrow(),
            vec![] as Vec<(PersonId, NaiveDate)>
        );
        assert_eq!(*service.unregister.borrow(), vec![] as Vec<PersonId>);

        assert_eq!(*service.cao.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.cao.load.borrow(),
            vec![
                (
                    id1,
                    PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is sender"), 0)
                ),
                (
                    id2,
                    PersonDto::new("Bob", date(2001, 2, 2), None, Some("Bob is receiver"), 0)
                ),
                (
                    id3,
                    PersonDto::new("Eve", date(2002, 3, 3), None, Some("Eve is interceptor"), 0)
                ),
            ]
        );
        assert_eq!(*service.cao.unload.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.reporter.report.borrow(),
            vec![] as Vec<(Level, String, String)>
        );

        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                Uuid::now_v7(),
                PersonDto::new("", date(2000, 1, 1), None, Some(""), 0),
            )), // 使われない
            find: RefCell::new(vec![]),
            find_result: Ok(None), // 使われない
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![id1, id2, id3]),
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![]), // 使われない
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Ok(None), // 使われない
                load: Rc::new(RefCell::new(vec![])),
                load_result: Err(CaoError::Unavailable("valid cao".to_string())),
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Ok(()), // 使われない
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.batch_import(
            vec![
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is sender"), 0),
                PersonDto::new("Bob", date(2001, 2, 2), None, Some("Bob is receiver"), 0),
                PersonDto::new("Eve", date(2002, 3, 3), None, Some("Eve is interceptor"), 0),
            ],
            Rc::new(DummyPersonOutputBoundary),
        );
        assert_eq!(*service.register.borrow(), vec![]);
        assert_eq!(*service.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![vec![
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is sender"), 0),
                PersonDto::new("Bob", date(2001, 2, 2), None, Some("Bob is receiver"), 0),
                PersonDto::new("Eve", date(2002, 3, 3), None, Some("Eve is interceptor"), 0),
            ]]
        );
        assert_eq!(*service.list_all.borrow(), 0);
        assert_eq!(
            *service.death.borrow(),
            vec![] as Vec<(PersonId, NaiveDate)>
        );
        assert_eq!(*service.unregister.borrow(), vec![] as Vec<PersonId>);

        assert_eq!(*service.cao.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.cao.load.borrow(),
            // 一つ目はロードされて、そのあとはエラーにより中断されている状態
            // 実際の場面では空であることが多いと思うが不定であるため、この値の検証にはあまり意味はない
            vec![(
                id1,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is sender"), 0)
            ),]
        );
        assert_eq!(*service.cao.unload.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.reporter.report.borrow(),
            vec![(
                Level::Error,
                "admin".to_string(),
                "cache service not available".to_string()
            )]
        );
    }

    #[test]
    fn test_list_all() {
        let id1 = Uuid::now_v7();
        let id2 = Uuid::now_v7();
        let id3 = Uuid::now_v7();
        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                Uuid::now_v7(),
                PersonDto::new("", date(2000, 1, 1), None, Some(""), 0),
            )), // 使われない
            find: RefCell::new(vec![]),
            find_result: Ok(None), // 使われない
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![]), // 使われない
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![
                (
                    id1,
                    PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
                ),
                (
                    id2,
                    PersonDto::new("Bob", date(2001, 2, 2), None, Some("Bob is here"), 0),
                ),
                (
                    id3,
                    PersonDto::new("Eve", date(2002, 3, 3), None, Some("Eve is here"), 0),
                ),
            ]),
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Ok(None), // 使われない
                load: Rc::new(RefCell::new(vec![])),
                load_result: Ok(()), // 使われない
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Ok(()), // 使われない
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.list_all();
        assert_eq!(*service.register.borrow(), vec![]);
        assert_eq!(*service.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![] as Vec<Vec<PersonDto>>
        );
        assert_eq!(*service.list_all.borrow(), 1);
        assert_eq!(
            *service.death.borrow(),
            vec![] as Vec<(PersonId, NaiveDate)>
        );
        assert_eq!(*service.unregister.borrow(), vec![] as Vec<PersonId>);

        assert_eq!(*service.cao.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.cao.load.borrow(),
            vec![
                (
                    id1,
                    PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
                ),
                (
                    id2,
                    PersonDto::new("Bob", date(2001, 2, 2), None, Some("Bob is here"), 0),
                ),
                (
                    id3,
                    PersonDto::new("Eve", date(2002, 3, 3), None, Some("Eve is here"), 0),
                ),
            ]
        );
        assert_eq!(*service.cao.unload.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(*service.reporter.report.borrow(), vec![]);

        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                Uuid::now_v7(),
                PersonDto::new("", date(2000, 1, 1), None, Some(""), 0),
            )), // 使われない
            find: RefCell::new(vec![]),
            find_result: Ok(None), // 使われない
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![]), // 使われない
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![
                (
                    id1,
                    PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
                ),
                (
                    id2,
                    PersonDto::new("Bob", date(2001, 2, 2), None, Some("Bob is here"), 0),
                ),
                (
                    id3,
                    PersonDto::new("Eve", date(2002, 3, 3), None, Some("Eve is here"), 0),
                ),
            ]),
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Ok(None), // 使われない
                load: Rc::new(RefCell::new(vec![])),
                load_result: Err(CaoError::Unavailable("valid cao".to_string())),
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Ok(()), // 使われない
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.list_all();
        assert_eq!(*service.register.borrow(), vec![]);
        assert_eq!(*service.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![] as Vec<Vec<PersonDto>>
        );
        assert_eq!(*service.list_all.borrow(), 1);
        assert_eq!(
            *service.death.borrow(),
            vec![] as Vec<(PersonId, NaiveDate)>
        );
        assert_eq!(*service.unregister.borrow(), vec![] as Vec<PersonId>);

        assert_eq!(*service.cao.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.cao.load.borrow(),
            vec![(
                id1,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            ),]
        );
        assert_eq!(*service.cao.unload.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.reporter.report.borrow(),
            vec![(
                Level::Error,
                "admin".to_string(),
                "cache service not available".to_string()
            )]
        );
    }

    #[test]
    fn test_unregister() {
        let id = Uuid::now_v7();
        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                Uuid::now_v7(),
                PersonDto::new("", date(2000, 1, 1), None, Some(""), 0),
            )), // 使われない
            find: RefCell::new(vec![]),
            find_result: Ok(None), // 使われない
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![]), // 使われない
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![]),
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Ok(None), // 使われない
                load: Rc::new(RefCell::new(vec![])),
                load_result: Ok(()), // 使われない
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Ok(()), // 使われない
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.unregister(id);
        assert_eq!(*service.register.borrow(), vec![]);
        assert_eq!(*service.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![] as Vec<Vec<PersonDto>>
        );
        assert_eq!(*service.list_all.borrow(), 0);
        assert_eq!(
            *service.death.borrow(),
            vec![] as Vec<(PersonId, NaiveDate)>
        );
        assert_eq!(*service.unregister.borrow(), vec![id]);

        assert_eq!(*service.cao.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.cao.load.borrow(),
            vec![] as Vec<(PersonId, PersonDto)>
        );
        assert_eq!(*service.cao.unload.borrow(), vec![id]);
        assert_eq!(*service.reporter.report.borrow(), vec![]);

        let mut service = TargetPersonService {
            register: RefCell::new(vec![]),
            register_result: Ok((
                Uuid::now_v7(),
                PersonDto::new("", date(2000, 1, 1), None, Some(""), 0),
            )), // 使われない
            find: RefCell::new(vec![]),
            find_result: Ok(None), // 使われない
            batch_import: RefCell::new(vec![]),
            batch_import_result: Ok(vec![]), // 使われない
            list_all: RefCell::new(0),
            list_all_result: Ok(vec![]),
            death: RefCell::new(vec![]),
            death_result: Ok(()), // 使われない
            unregister: RefCell::new(vec![]),
            unregister_result: Ok(()), // 使われない
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: MockPersonCao {
                find: Rc::new(RefCell::new(vec![])),
                find_result: Ok(None), // 使われない
                load: Rc::new(RefCell::new(vec![])),
                load_result: Ok(()), // 使われない
                unload: Rc::new(RefCell::new(vec![])),
                unload_result: Err(CaoError::Unavailable("cao valid".to_string())),
            },
            reporter: SpyReporter {
                report: RefCell::new(vec![]).into(),
            },
        };

        let _ = service.unregister(id);
        assert_eq!(*service.register.borrow(), vec![]);
        assert_eq!(*service.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.batch_import.borrow(),
            vec![] as Vec<Vec<PersonDto>>
        );
        assert_eq!(*service.list_all.borrow(), 0);
        assert_eq!(
            *service.death.borrow(),
            vec![] as Vec<(PersonId, NaiveDate)>
        );
        assert_eq!(*service.unregister.borrow(), vec![id]);

        assert_eq!(*service.cao.find.borrow(), vec![] as Vec<PersonId>);
        assert_eq!(
            *service.cao.load.borrow(),
            vec![] as Vec<(PersonId, PersonDto)>
        );
        assert_eq!(*service.cao.unload.borrow(), vec![id]);
        assert_eq!(
            *service.reporter.report.borrow(),
            vec![(
                Level::Error,
                "admin".to_string(),
                "cache service not available".to_string()
            )]
        );
    }
}

// # エラー系スタブテスト
//
// ## 目的
//
//   Cache, Reporter や Service がエラーを返した場合の CachedService の挙動を保障する
//
// ## 方針
//
//   スタブ Cache, スタブ Reporter や Service はメソッドが呼び出されると、事前に設定された任意のエラー値を返す
//   CachedService のメソッドを呼び出して Cache, Reporter あるいは Service からエラーを受け取ったときの CachedService の返り値を確認する
//
// ## 実装
//
//                                 Test Double
//        +----------------+      +---------------+.oOo.+---------------+
//        | Cached Service |      | Stub Service  |     | Dummy Usecase |
//        | ============== |      | ============  |     | ============= |
//        |                |      |               |     |               |
//   ---->| -------------> |---+->| --->          |     |               |
//        |                |   |  |               |     |               |
//   <-c--| <------c------ |<-+|--| <--- any error|     |               |
//     |  +--------|-------+  ||  +---------------+     +---------------+
//     |           |          ||   Test Double
//     |           |          ||  +---------------+
//     |           |          ||  | Stub Cache    |
//     |           |          ||  | ============  |
//     |           |          ||  |               |
//     |           |          |+->| --->          |
//     |           |          ||  |               |
//     |           |          +|--| <--- any error|
//     |           |          ||  +---------------+
//     |           |          ||   Test Double
//     |           |          ||  +---------------+
//     |           |          ||  | Stub Reporter |
//     |           |          ||  | ============= |
//     |           |          ||  |               |
//     |           |          |+->| --->          |
//     |           |          |   |               |
//     |           |          +---| <--- any error|
//     |           |              +---------------+
//     |           |
//     |           +-- テスト対象
//     |
//     +-- ここを確認する
//
//   1. ダミーの DAO 構造体とユースケース構造体を用意する
//      この構造体は実質使われないが、 Service の構成で必要になるため用意する
//   2. Service のメソッドが任意の結果を返せる種類の Service 構造体を用意する
//      この Service 構造体はスタブであり、CachedService への間接的な入力のみ制御する
//   3. Cache のメソッドが任意の結果を返せる種類の Cache 構造体を用意する
//      この Cache 構造体はスタブであり、CachedService への間接的な入力のみ制御する
//   4. Reporter のメソッドが任意の結果を返せる種類の Reporter 構造体を用意する
//      この Reporter 構造体はスタブであり、CachedService への間接的な入力のみ制御する
//   5. その構造体を CachedService にプラグインする
//   6. CachedService のメソッドを呼び出す
//   7. CachedService のメソッドからの戻り値を確認する
//
// ## 注意
//
//   1. CachedService とスタブとなる Stub Service とは構造体としては同一になっている
//
#[cfg(test)]
mod error_stub_tests {
    use chrono::NaiveDate;
    use std::cell::RefCell;
    use std::rc::Rc;
    use uuid::Uuid;

    use crate::{
        cache::{CaoError, PersonCao},
        cached_service::PersonCachedService,
        dao::{DaoError, HavePersonDao, PersonDao},
        domain::{date, PersonId, Revision},
        dto::PersonDto,
        location::Location,
        reporter::{Level, Reporter, ReporterError},
        usecase::{PersonUsecase, UsecaseError},
    };

    struct DummyPersonDao;
    impl PersonDao<()> for DummyPersonDao {
        fn insert(
            &self,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
            tx_rs::with_tx(move |&mut ()| Ok(Uuid::now_v7()))
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

    struct DummyPersonUsecase {
        dao: DummyPersonDao,
    }
    impl HavePersonDao<()> for DummyPersonUsecase {
        fn get_dao(&self) -> &impl PersonDao<()> {
            &self.dao
        }
    }
    impl PersonUsecase<()> for DummyPersonUsecase {
        fn entry<'a>(
            &'a mut self,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(Uuid::now_v7()))
        }
        fn find<'a>(
            &'a mut self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(None))
        }
        fn entry_and_verify<'a>(
            &'a mut self,
            person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (PersonId, PersonDto), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok((Uuid::now_v7(), person)))
        }
        fn collect<'a>(
            &'a mut self,
        ) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(vec![]))
        }
        fn death<'a>(
            &'a mut self,
            _id: PersonId,
            _date: NaiveDate,
        ) -> impl tx_rs::Tx<(), Item = (), Err = UsecaseError>
        where
            (): 'a,
        {
            tx_rs::with_tx(move |&mut ()| Ok(()))
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

    /// テスト用のスタブサービスです。
    struct TargetPersonService {
        register_result: Result<(PersonId, PersonDto), crate::service::ServiceError>,
        find_result: Result<Option<PersonDto>, crate::service::ServiceError>,
        batch_import_result: Result<Vec<PersonId>, crate::service::ServiceError>,
        list_all_result: Result<Vec<(PersonId, PersonDto)>, crate::service::ServiceError>,
        death_result: Result<(), crate::service::ServiceError>,
        unregister_result: Result<(), crate::service::ServiceError>,

        usecase: RefCell<DummyPersonUsecase>,
        cao: StubPersonCao,
    }
    // スタブサービス実装です。ユースケースより先はダミーです。
    impl crate::service::PersonService<'_, ()> for TargetPersonService {
        type U = DummyPersonUsecase;
        type N = DummyReporter;

        fn run_tx<T, F>(&mut self, f: F) -> Result<T, crate::service::ServiceError>
        where
            F: FnOnce(&mut Self::U, &mut ()) -> Result<T, UsecaseError>,
        {
            let mut usecase = self.usecase.borrow_mut();
            f(&mut usecase, &mut ()).map_err(crate::service::ServiceError::TransactionFailed)
        }

        fn get_reporter(&self) -> Self::N {
            DummyReporter
        }

        fn register(
            &'_ mut self,
            _name: &str,
            _birth_date: NaiveDate,
            _death_date: Option<NaiveDate>,
            _data: &str,
        ) -> Result<(PersonId, PersonDto), crate::service::ServiceError> {
            self.register_result.clone()
        }

        fn find(
            &'_ mut self,
            _id: PersonId,
        ) -> Result<Option<PersonDto>, crate::service::ServiceError> {
            self.find_result.clone()
        }

        fn batch_import(
            &'_ mut self,
            _persons: impl Iterator<Item = PersonDto>,
            _out_port: Rc<
                impl crate::service::PersonOutputBoundary<(u64, u64), crate::service::ServiceError>,
            >,
        ) -> Result<Vec<PersonId>, crate::service::ServiceError> {
            self.batch_import_result.clone()
        }

        fn list_all(
            &'_ mut self,
        ) -> Result<Vec<(PersonId, PersonDto)>, crate::service::ServiceError> {
            self.list_all_result.clone()
        }

        fn death(
            &'_ mut self,
            _id: PersonId,
            _date: NaiveDate,
        ) -> Result<(), crate::service::ServiceError> {
            self.death_result.clone()
        }

        fn unregister(&'_ mut self, _id: PersonId) -> Result<(), crate::service::ServiceError> {
            self.unregister_result.clone()
        }
    }
    // スタブキャッシュ実装です
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct StubPersonCao {
        find_result: Result<Option<PersonDto>, CaoError>,
        load_result: Result<(), CaoError>,
        unload_result: Result<(), CaoError>,
    }
    impl PersonCao<()> for StubPersonCao {
        fn get_conn(&self) -> Result<(), CaoError> {
            Ok(())
        }
        fn run_tx<T, F>(&self, f: F) -> Result<T, CaoError>
        where
            F: tx_rs::Tx<(), Item = T, Err = CaoError>,
        {
            f.run(&mut ())
        }
        fn find(
            &self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = CaoError> {
            tx_rs::with_tx(move |&mut ()| self.find_result.clone())
        }
        fn load(
            &self,
            _id: PersonId,
            _person: &PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (), Err = CaoError> {
            tx_rs::with_tx(move |&mut ()| self.load_result.clone())
        }
        fn unload(&self, _id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = CaoError> {
            tx_rs::with_tx(move |&mut ()| self.unload_result.clone())
        }
    }
    impl PersonCachedService<'_, (), ()> for TargetPersonService {
        type C = StubPersonCao;

        fn get_cao(&self) -> Self::C {
            self.cao.clone()
        }
    }

    struct DummyPersonOutputBoundary;
    impl crate::service::PersonOutputBoundary<(u64, u64), crate::service::ServiceError>
        for DummyPersonOutputBoundary
    {
        fn started(&self) {}
        fn in_progress(&self, _progress: (u64, u64)) {}
        fn completed(&self) {}
        fn aborted(&self, _err: crate::service::ServiceError) {}
    }

    #[test]
    fn test_register() {
        let id = Uuid::now_v7();
        let mut service = TargetPersonService {
            register_result: Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::EntryPersonFailed(DaoError::InsertError("valid dao".to_string())),
            )),
            find_result: Ok(None),
            batch_import_result: Ok(vec![]),
            list_all_result: Ok(vec![]),
            death_result: Ok(()),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Ok(()),
                unload_result: Ok(()),
            },
        };
        let result = service.register("test", date(2000, 1, 1), None, "test");
        assert_eq!(
            result,
            Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::EntryPersonFailed(DaoError::InsertError("valid dao".to_string()))
            ))
        );

        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find_result: Ok(None),
            batch_import_result: Ok(vec![]),
            list_all_result: Ok(vec![]),
            death_result: Ok(()),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Err(CaoError::Unavailable("valid cao".to_string())),
                unload_result: Ok(()),
            },
        };
        let result = service.register("test", date(2000, 1, 1), None, "test");
        assert_eq!(
            result,
            Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0)
            ))
        );
    }

    #[test]
    fn test_find() {
        let id = Uuid::now_v7();
        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find_result: Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::FindPersonFailed(DaoError::SelectError("valid dao".to_string())),
            )),
            batch_import_result: Ok(vec![]),
            list_all_result: Ok(vec![]),
            death_result: Ok(()),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Ok(()),
                unload_result: Ok(()),
            },
        };
        let result = service.find(id);
        assert_eq!(
            result,
            Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::FindPersonFailed(DaoError::SelectError("valid dao".to_string()))
            ))
        );

        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find_result: Ok(Some(PersonDto::new(
                "Alice",
                date(2000, 1, 1),
                None,
                Some("Alice is here"),
                0,
            ))),
            batch_import_result: Ok(vec![]),
            list_all_result: Ok(vec![]),
            death_result: Ok(()),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Err(CaoError::Unavailable("valid cao".to_string())),
                load_result: Ok(()),
                unload_result: Ok(()),
            },
        };
        let result = service.find(id);
        assert_eq!(
            result,
            Ok(Some(PersonDto::new(
                "Alice",
                date(2000, 1, 1),
                None,
                Some("Alice is here"),
                0
            )))
        );
    }

    #[test]
    fn test_batch_import() {
        let id = Uuid::now_v7();
        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find_result: Ok(None),
            batch_import_result: Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::EntryPersonFailed(DaoError::InsertError("valid dao".to_string())),
            )),
            list_all_result: Ok(vec![]),
            death_result: Ok(()),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Ok(()),
                unload_result: Ok(()),
            },
        };
        let result = service.batch_import(vec![], Rc::new(DummyPersonOutputBoundary));
        assert_eq!(
            result,
            Err(crate::service::ServiceError::InvalidRequest(
                crate::service::InvalidErrorKind::EmptyArgument
            ))
        );

        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find_result: Ok(None),
            batch_import_result: Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::EntryPersonFailed(DaoError::InsertError("valid dao".to_string())),
            )),
            list_all_result: Ok(vec![]),
            death_result: Ok(()),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Ok(()),
                unload_result: Ok(()),
            },
        };
        let result = service.batch_import(
            vec![PersonDto::new(
                "Alice",
                date(2000, 1, 1),
                None,
                Some("Alice is here"),
                0,
            )],
            Rc::new(DummyPersonOutputBoundary),
        );
        assert_eq!(
            result,
            Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::EntryPersonFailed(DaoError::InsertError("valid dao".to_string()))
            ))
        );

        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find_result: Ok(None),
            batch_import_result: Ok(vec![id]),
            list_all_result: Ok(vec![]),
            death_result: Ok(()),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Err(CaoError::Unavailable("valid cao".to_string())),
                unload_result: Ok(()),
            },
        };
        let result = service.batch_import(
            vec![PersonDto::new(
                "Alice",
                date(2000, 1, 1),
                None,
                Some("Alice is here"),
                0,
            )],
            Rc::new(DummyPersonOutputBoundary),
        );
        assert_eq!(result, Ok(vec![id]));
    }

    #[test]
    fn test_list_all() {
        let id = Uuid::now_v7();
        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find_result: Ok(None),
            batch_import_result: Ok(vec![]),
            list_all_result: Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::CollectPersonFailed(DaoError::SelectError("valid dao".to_string())),
            )),
            death_result: Ok(()),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Ok(()),
                unload_result: Ok(()),
            },
        };
        let result = service.list_all();
        assert_eq!(
            result,
            Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::CollectPersonFailed(DaoError::SelectError("valid dao".to_string()))
            ))
        );

        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find_result: Ok(None),
            batch_import_result: Ok(vec![]),
            list_all_result: Ok(vec![(
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )]),
            death_result: Ok(()),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Err(CaoError::Unavailable("valid cao".to_string())),
                unload_result: Ok(()),
            },
        };
        let result = service.list_all();
        assert_eq!(
            result,
            Ok(vec![(
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )])
        );
    }

    #[test]
    fn test_death() {
        let id = Uuid::now_v7();
        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new(
                    "poor man",
                    date(2000, 1, 1),
                    None,
                    Some("poor man will be dead"),
                    0,
                ),
            )),
            find_result: Ok(None),
            batch_import_result: Ok(vec![]),
            list_all_result: Ok(vec![]),
            death_result: Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::SavePersonFailed(DaoError::UpdateError("valid dao".to_string())),
            )),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Ok(()),
                unload_result: Ok(()),
            },
        };
        let result = service.death(id, date(2030, 12, 31));
        assert_eq!(
            result,
            Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::SavePersonFailed(DaoError::UpdateError("valid dao".to_string()))
            ))
        );

        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new(
                    "poor man",
                    date(2000, 1, 1),
                    None,
                    Some("poor man will be dead"),
                    0,
                ),
            )),
            find_result: Ok(None),
            batch_import_result: Ok(vec![]),
            list_all_result: Ok(vec![]),
            death_result: Ok(()),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Ok(()),
                unload_result: Err(CaoError::Unavailable("valid cao".to_string())),
            },
        };
        let result = service.death(id, date(2030, 12, 31));
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn test_unregister() {
        let id = Uuid::now_v7();
        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find_result: Ok(None),
            batch_import_result: Ok(vec![]),
            list_all_result: Ok(vec![]),
            death_result: Ok(()),
            unregister_result: Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::RemovePersonFailed(DaoError::DeleteError("valid dao".to_string())),
            )),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Ok(()),
                unload_result: Ok(()),
            },
        };
        let result = service.unregister(id);
        assert_eq!(
            result,
            Err(crate::service::ServiceError::TransactionFailed(
                UsecaseError::RemovePersonFailed(DaoError::DeleteError("valid dao".to_string()))
            ))
        );

        let mut service = TargetPersonService {
            register_result: Ok((
                id,
                PersonDto::new("Alice", date(2000, 1, 1), None, Some("Alice is here"), 0),
            )),
            find_result: Ok(None),
            batch_import_result: Ok(vec![]),
            list_all_result: Ok(vec![]),
            death_result: Ok(()),
            unregister_result: Ok(()),
            usecase: RefCell::new(DummyPersonUsecase {
                dao: DummyPersonDao,
            }),
            cao: StubPersonCao {
                find_result: Ok(None),
                load_result: Ok(()),
                unload_result: Err(CaoError::Unavailable("valid cao".to_string())),
            },
        };
        let result = service.unregister(id);
        assert_eq!(result, Ok(()));
    }
}
