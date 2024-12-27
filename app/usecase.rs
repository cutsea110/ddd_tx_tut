use chrono::NaiveDate;
use log::{trace, warn};
use thiserror::Error;
use tx_rs::Tx;

use crate::dao::{DaoError, HavePersonDao, PersonDao};
use crate::domain::{Person, PersonDomainError, PersonId};
use crate::dto::PersonDto;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum UsecaseError {
    #[error("entry person failed: {0}")]
    EntryPersonFailed(DaoError),
    #[error("find person failed: {0}")]
    FindPersonFailed(DaoError),
    #[error("entry and verify failed: {0}")]
    EntryAndVerifyPersonFailed(DaoError),
    #[error("collect person failed: {0}")]
    CollectPersonFailed(DaoError),
    #[error("save person failed: {0}")]
    SavePersonFailed(DaoError),
    #[error("remove person failed: {0}")]
    RemovePersonFailed(DaoError),
    #[error("remove person failed: {0}")]
    DomainObjectChangeFailed(PersonDomainError),
}
pub trait PersonUsecase<Ctx>: HavePersonDao<Ctx> {
    fn entry<'a>(
        &'a mut self,
        person: PersonDto,
    ) -> impl tx_rs::Tx<Ctx, Item = PersonId, Err = UsecaseError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        trace!("entry person: {:?}", person);
        dao.insert(person).map_err(UsecaseError::EntryPersonFailed)
    }
    fn find<'a>(
        &'a mut self,
        id: PersonId,
    ) -> impl tx_rs::Tx<Ctx, Item = Option<PersonDto>, Err = UsecaseError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        trace!("find person_id: {:?}", id);
        dao.fetch(id).map_err(UsecaseError::FindPersonFailed)
    }
    fn entry_and_verify<'a>(
        &'a mut self,
        person: PersonDto,
    ) -> impl tx_rs::Tx<Ctx, Item = (PersonId, PersonDto), Err = UsecaseError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        trace!("entry and verify person: {:?}", person);
        dao.insert(person)
            .and_then(move |id| {
                dao.fetch(id).try_map(move |person| {
                    if let Some(p) = person {
                        return Ok((id, p));
                    }

                    warn!("can't find the person just entried: {}", id);
                    Err(DaoError::SelectError(format!("not found: {id}")))
                })
            })
            .map_err(UsecaseError::EntryAndVerifyPersonFailed)
    }
    fn collect<'a>(
        &'a mut self,
    ) -> impl tx_rs::Tx<Ctx, Item = Vec<(PersonId, PersonDto)>, Err = UsecaseError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        trace!("collect all persons");
        dao.select().map_err(UsecaseError::CollectPersonFailed)
    }
    fn death<'a>(
        &'a mut self,
        id: PersonId,
        date: NaiveDate,
    ) -> impl tx_rs::Tx<Ctx, Item = (), Err = UsecaseError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        trace!("death person: id={} date={}", id, date);
        dao.fetch(id)
            .map_err(UsecaseError::FindPersonFailed)
            .try_map(move |p| {
                if let Some(person) = p {
                    trace!("found person (id={}): {:?}", id, person);
                    let mut p: Person = person.into();
                    return p
                        .dead_at(date)
                        .map(|_| p.into())
                        .map_err(UsecaseError::DomainObjectChangeFailed);
                }

                warn!("can't find the person to dead: {}", id);
                Err(UsecaseError::FindPersonFailed(DaoError::SelectError(
                    format!("person not found: {id}"),
                )))
            })
            .and_then(move |mut p: PersonDto| {
                trace!("save dead person (id={}): {:?}", id, p);
                // 最新版の管理はユースケースの責務
                let orig_revision = p.revision;
                p.revision += 1;
                dao.save(id, orig_revision, p)
                    .map_err(UsecaseError::SavePersonFailed)
            })
    }
    fn remove<'a>(&'a mut self, id: PersonId) -> impl tx_rs::Tx<Ctx, Item = (), Err = UsecaseError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        trace!("remove person_id: {:?}", id);
        dao.delete(id).map_err(UsecaseError::RemovePersonFailed)
    }
}

// # フェイクテスト
//
// ## 目的
//
//   Usecase の正常系のテストを行う
//   Usecase の各メソッドが DAO から通常期待される結果を受け取ったときに適切にふるまうことを保障する
//
// ## 方針
//
//   DAO のフェイクに対して Usecase を実行し、その結果を確認する
//   フェイクはテスト時の比較チェックのしやすさを考慮して HashMap ではなく Vec で登録データを保持する
//   データ数は多くないので、Vec でリニアサーチしても十分な速度が出ると考える
//
// ## 実装
//
//                          Test Double
//        +---------+      +------------+
//        | Usecase |      | Fake DAO   |
//        | ======= |      | ========== |
//        |         |      |            |
//   --c->| ---c--> |----->| ---+       |
//     |  |    |    |      |    | fake logic
//   <-c--| <--c--- |<-----| <--+       |
//     |  +----|----+      +------------+
//     |       |
//     |       +-- テスト対象
//     |
//     +-- ここを確認する
//
//   1. DAO のメソッド呼び出しに対して、凡そ正常系で期待される結果を返す DAO 構造体を用意する
//      この DAO 構造体はフェイクなので、間接的な入力と間接的な出力が整合するようにする
//   2. Usecase にそのフェイクをプラグインする
//   3. Usecase のメソッドを呼び出す
//   4. Usecase からの戻り値を検証する
//
// ## 注意
//
//   1. このテストは Usecase の実装を保障するものであって、DAO の実装を保障するものではない
//
#[cfg(test)]
mod fake_tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use uuid::Uuid;

    use super::*;
    use crate::domain::{date, Revision};
    use crate::dto::PersonDto;

    struct FakePersonDao {
        next_id: RefCell<VecDeque<PersonId>>,
        data: RefCell<Vec<(PersonId, PersonDto)>>,
    }
    // Ctx 不要なので () にしている
    impl PersonDao<()> for FakePersonDao {
        fn insert(&self, person: PersonDto) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
            let id = self.next_id.borrow_mut().pop_front().unwrap();
            self.data.borrow_mut().push((id, person));

            tx_rs::with_tx(move |()| Ok(id))
        }
        fn fetch(
            &self,
            id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = DaoError> {
            let data = self.data.borrow();
            let result = data.iter().find(|(i, _)| *i == id).map(|(_, p)| p.clone());

            tx_rs::with_tx(move |()| Ok(result))
        }
        fn select(&self) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = DaoError> {
            let result = self.data.borrow().clone();

            tx_rs::with_tx(move |()| Ok(result))
        }
        fn save(
            &self,
            id: PersonId,
            revision: Revision,
            person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            let result = self
                .data
                .borrow_mut()
                .iter_mut()
                .find(|(i, p)| *i == id && p.revision == revision)
                .map(|(_, p)| *p = person)
                .ok_or(DaoError::UpdateError(format!("person not found: {id}")));

            tx_rs::with_tx(move |()| result)
        }
        fn delete(&self, id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            let _ = self.data.borrow_mut().retain(|(i, _)| *i != id);

            tx_rs::with_tx(move |()| Ok(()))
        }
    }

    struct TargetPersonUsecase {
        dao: FakePersonDao,
    }
    impl HavePersonDao<()> for TargetPersonUsecase {
        fn get_dao(&self) -> &impl PersonDao<()> {
            &self.dao
        }
    }
    impl PersonUsecase<()> for TargetPersonUsecase {}

    #[test]
    fn test_entry() {
        let id = Uuid::now_v7();
        let dao = FakePersonDao {
            next_id: RefCell::new(VecDeque::from(vec![id])),
            data: RefCell::new(vec![]),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let person = PersonDto::new(
            "Alice",
            date(2012, 11, 2),
            None,
            Some("Alice wonderland"),
            0,
        );
        let expected = person.clone().into();
        let expected_id = id;

        let result = usecase.entry(person).run(&mut ());
        assert_eq!(result, Ok(expected_id));
        assert_eq!(usecase.dao.data.borrow().len(), 1);
        assert_eq!(*usecase.dao.data.borrow(), vec![(expected_id, expected)]);
    }
    #[test]
    fn test_find() {
        let id1 = Uuid::now_v7();
        let id2 = Uuid::now_v7();
        let id3 = Uuid::now_v7();
        let dao = FakePersonDao {
            next_id: RefCell::new(VecDeque::from(vec![Uuid::now_v7()])), // 使わない
            data: RefCell::new(vec![
                (
                    id1,
                    PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 3),
                ),
                (
                    id2,
                    PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver"), 1),
                ),
                (
                    id3,
                    PersonDto::new(
                        "Eve",
                        date(1996, 12, 15),
                        None,
                        Some("Eve is interceptor"),
                        8,
                    ),
                ),
            ]),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let result = usecase.find(id1).run(&mut ());
        let expected = Some(PersonDto::new(
            "Alice",
            date(2012, 11, 2),
            None,
            Some("Alice is sender"),
            3,
        ));
        assert_eq!(result, Ok(expected));
    }
    #[test]
    fn test_entry_and_verify() {
        let id = Uuid::now_v7();
        let dao = FakePersonDao {
            next_id: RefCell::new(VecDeque::from(vec![id])),
            data: RefCell::new(vec![]),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let person = PersonDto::new(
            "Alice",
            date(2012, 11, 2),
            None,
            Some("Alice wonderland"),
            3,
        );
        let expected = (id, person.clone());

        let result = usecase.entry_and_verify(person).run(&mut ());
        assert_eq!(result, Ok(expected));
    }
    #[test]
    fn test_collect() {
        let id1 = Uuid::now_v7();
        let id2 = Uuid::now_v7();
        let id3 = Uuid::now_v7();
        let data = vec![
            (
                id1,
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 3),
            ),
            (
                id2,
                PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver"), 1),
            ),
            (
                id3,
                PersonDto::new(
                    "Eve",
                    date(1996, 12, 15),
                    None,
                    Some("Eve is interceptor"),
                    7,
                ),
            ),
        ];
        let expected = data
            .clone()
            .into_iter()
            .map(|(id, p)| (id, p.into()))
            .collect::<Vec<_>>();

        let dao = FakePersonDao {
            next_id: RefCell::new(VecDeque::from(vec![Uuid::now_v7()])), // 使わない
            data: RefCell::new(data),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let result = usecase.collect().run(&mut ());
        assert_eq!(
            result.map(|mut v: Vec<(PersonId, PersonDto)>| {
                v.sort_by_key(|(id, _)| *id);
                v
            }),
            Ok(expected)
        );
    }
    #[test]
    fn test_death() {
        let id = Uuid::now_v7();
        let dao = FakePersonDao {
            next_id: RefCell::new(VecDeque::from(vec![])), // 使わない
            data: RefCell::new(vec![(
                id,
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 0),
            )]),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let result = usecase.death(id, date(2020, 12, 30)).run(&mut ());
        let expected = vec![(
            id,
            PersonDto::new(
                "Alice",
                date(2012, 11, 2),
                Some(date(2020, 12, 30)),
                Some("Alice is sender"),
                1,
            ),
        )];
        assert_eq!(result, Ok(()));
        assert_eq!(*usecase.dao.data.borrow(), expected);
    }
    #[test]
    fn test_remove() {
        let id1 = Uuid::now_v7();
        let id2 = Uuid::now_v7();
        let id3 = Uuid::now_v7();
        let data = vec![
            (
                id1,
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 3),
            ),
            (
                id2,
                PersonDto::new("Bob", date(1995, 11, 6), None, Some("Bob is receiver"), 1),
            ),
            (
                id3,
                PersonDto::new(
                    "Eve",
                    date(1996, 12, 15),
                    None,
                    Some("Eve is interceptor"),
                    7,
                ),
            ),
        ];

        let dao = FakePersonDao {
            next_id: RefCell::new(VecDeque::from(vec![])), // 使わない
            data: RefCell::new(data),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let result = usecase.remove(id2).run(&mut ());
        let expected = vec![
            (
                id1,
                PersonDto::new("Alice", date(2012, 11, 2), None, Some("Alice is sender"), 3),
            ),
            (
                id3,
                PersonDto::new(
                    "Eve",
                    date(1996, 12, 15),
                    None,
                    Some("Eve is interceptor"),
                    7,
                ),
            ),
        ];
        assert_eq!(result, Ok(()));
        assert_eq!(*usecase.dao.data.borrow(), expected);
    }
}

// # スパイテスト
//
// ## 目的
//
//   Usecase の各メソッドが DAO のメソッドを適切に呼び出していることを保障する
//   つまり、
//    1. 必要なメソッドを必要回数だけ呼び出していること
//    2. 不必要なメソッドを呼び出していないこと
//    3. Usecase に渡った引数が適切に DAO のメソッドに渡されていること
//   を保障する
//
//
// ## 方針
//
//   DAO のメソッドを呼び出すたびに、その呼び出しを記録する
//   その記録をテストの最後で確認する
//
// ## 実装
//
//                          Test Double
//        +---------+      +------------+
//        | Usecase |      | Spy DAO    |
//        | ======= |      | ========== |
//        |         |      |            |
//   ---->| ------> |--c-->| --> [ c ] request log
//        |         |  |   |       |    |
//   <----| <------ |<-|---| <--   |    |
//        +---------+  |   +-------|----+
//                     |           |
//                     |           +-- ここを確認する
//                     |
//                     +-- テスト対象
//
//   1. DAO のメソッド呼び出しを記録する種類の DAO 構造体を用意する
//      この構造体はスパイなので、Usecase の間接的な出力のみ記録する
//   2. その構造体を Usecase にプラグインする
//   3. Usecase のメソッドを呼び出す
//   4. その後で DAO 構造体の記録を検証する
//
// ## 注意
//
//   1. このテストは Usecase の実装を保障するものであって、DAO の実装を保障するものではない
//   2. このテストは Usecase のメソッドが適切に DAO のメソッドを呼び出していることを保障するものであって、
//      DAO のメソッドが適切にデータベースを操作していることを保障するものではない
//   3. このテストは Usecase のメソッドが不適切な DAO のメソッド呼び出しをしていないことを保障するものであって、
//      DAO のメソッドが不適切なデータベースの操作をしていないことを保障するものではない
//   4. このテストでは DAO のメソッドの呼び出し順序については検証しない (将来的に検証することは拒否しない)
//
#[cfg(test)]
mod spy_tests {
    use std::cell::RefCell;

    use uuid::Uuid;

    use super::*;
    use crate::domain::{date, Revision};
    use crate::dto::PersonDto;

    struct SpyPersonDao {
        insert: RefCell<Vec<PersonDto>>,
        inserted_id: PersonId,
        fetch: RefCell<Vec<PersonId>>,
        fetch_result: Result<Option<PersonDto>, DaoError>,
        select: RefCell<i32>,
        save: RefCell<Vec<(PersonId, Revision, PersonDto)>>,
        delete: RefCell<Vec<PersonId>>,
    }
    // Ctx 不要なので () にしている
    impl PersonDao<()> for SpyPersonDao {
        fn insert(&self, person: PersonDto) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
            self.insert.borrow_mut().push(person);

            tx_rs::with_tx(|()| Ok(self.inserted_id))
        }
        fn fetch(
            &self,
            id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = DaoError> {
            self.fetch.borrow_mut().push(id);

            tx_rs::with_tx(|()| self.fetch_result.clone())
        }
        fn select(&self) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = DaoError> {
            *self.select.borrow_mut() += 1;

            // 返り値には意味なし
            tx_rs::with_tx(|()| Ok(vec![]))
        }
        fn save(
            &self,
            id: PersonId,
            revision: Revision,
            person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            self.save.borrow_mut().push((id, revision, person));

            // 返り値には意味なし
            tx_rs::with_tx(|()| Ok(()))
        }
        fn delete(&self, id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            self.delete.borrow_mut().push(id);

            // 返り値には意味なし
            tx_rs::with_tx(|()| Ok(()))
        }
    }

    struct TargetPersonUsecase {
        dao: SpyPersonDao,
    }
    impl HavePersonDao<()> for TargetPersonUsecase {
        fn get_dao(&self) -> &impl PersonDao<()> {
            &self.dao
        }
    }
    impl PersonUsecase<()> for TargetPersonUsecase {}

    #[test]
    fn test_entry() {
        let id = Uuid::now_v7();
        let dao = SpyPersonDao {
            insert: RefCell::new(vec![]),
            inserted_id: id, // 使わない
            fetch: RefCell::new(vec![]),
            fetch_result: Ok(None),
            select: RefCell::new(0),
            save: RefCell::new(vec![]),
            delete: RefCell::new(vec![]),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let person = PersonDto::new("Alice", date(2012, 11, 2), None, None, 0);
        let expected = person.clone().into();

        let _ = usecase.entry(person).run(&mut ()).unwrap();

        // DAO のメソッドの呼び出し記録の検証
        assert_eq!(usecase.dao.insert.borrow().len(), 1);
        assert_eq!(usecase.dao.fetch.borrow().len(), 0);
        assert_eq!(*usecase.dao.select.borrow(), 0);
        assert_eq!(usecase.dao.save.borrow().len(), 0);
        assert_eq!(usecase.dao.delete.borrow().len(), 0);

        // Usecase の引数が DAO にそのまま渡されていることを検証
        assert_eq!(usecase.dao.insert.borrow()[0], expected);
    }

    #[test]
    fn test_find() {
        let id = Uuid::now_v7();
        let dao = SpyPersonDao {
            insert: RefCell::new(vec![]),
            inserted_id: id, // 使わない
            fetch: RefCell::new(vec![]),
            fetch_result: Ok(None),
            select: RefCell::new(0),
            save: RefCell::new(vec![]),
            delete: RefCell::new(vec![]),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let expected = id;
        let _ = usecase.find(id).run(&mut ());

        // DAO のメソッドの呼び出し記録の検証
        assert_eq!(usecase.dao.insert.borrow().len(), 0);
        assert_eq!(usecase.dao.fetch.borrow().len(), 1);
        assert_eq!(*usecase.dao.select.borrow(), 0);
        assert_eq!(usecase.dao.save.borrow().len(), 0);
        assert_eq!(usecase.dao.delete.borrow().len(), 0);

        // Usecase の引数が DAO にそのまま渡されていることを確認
        assert_eq!(usecase.dao.fetch.borrow()[0], expected);
    }

    #[test]
    fn test_entry_and_verify() {
        let id = Uuid::now_v7();
        let dao = SpyPersonDao {
            insert: RefCell::new(vec![]),
            inserted_id: id,
            fetch: RefCell::new(vec![]),
            fetch_result: Ok(None),
            select: RefCell::new(0),
            save: RefCell::new(vec![]),
            delete: RefCell::new(vec![]),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let person = PersonDto::new("Alice", date(2012, 11, 2), None, None, 0);
        let expected = person.clone().into();

        let _ = usecase.entry_and_verify(person).run(&mut ());

        // DAO のメソッドの呼び出し記録の検証
        assert_eq!(usecase.dao.insert.borrow().len(), 1);
        assert_eq!(usecase.dao.fetch.borrow().len(), 1);
        assert_eq!(*usecase.dao.select.borrow(), 0);
        assert_eq!(usecase.dao.save.borrow().len(), 0);
        assert_eq!(usecase.dao.delete.borrow().len(), 0);

        // Usecase の引数が DAO にそのまま渡されていることを検証
        assert_eq!(usecase.dao.insert.borrow()[0], expected);
        // insert で返された ID が fetch に渡されていることを検証
        assert_eq!(usecase.dao.fetch.borrow()[0], usecase.dao.inserted_id);
    }

    #[test]
    fn test_collect() {
        let id = Uuid::now_v7();
        let dao = SpyPersonDao {
            insert: RefCell::new(vec![]),
            inserted_id: id, // 使わない
            fetch: RefCell::new(vec![]),
            fetch_result: Ok(None),
            select: RefCell::new(0),
            save: RefCell::new(vec![]),
            delete: RefCell::new(vec![]),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let _ = usecase.collect().run(&mut ());

        // DAO のメソッドの呼び出し記録の検証
        assert_eq!(usecase.dao.insert.borrow().len(), 0);
        assert_eq!(usecase.dao.fetch.borrow().len(), 0);
        assert_eq!(*usecase.dao.select.borrow(), 1);
        assert_eq!(usecase.dao.save.borrow().len(), 0);
        assert_eq!(usecase.dao.delete.borrow().len(), 0);
    }
    #[test]
    fn test_death() {
        let id = Uuid::now_v7();
        let dao = SpyPersonDao {
            insert: RefCell::new(vec![]),
            inserted_id: id, // 使わない
            fetch: RefCell::new(vec![]),
            fetch_result: Ok(Some(PersonDto::new(
                "Alice",
                date(2020, 10, 1),
                None,
                None,
                17,
            ))),
            select: RefCell::new(0),
            save: RefCell::new(vec![]),
            delete: RefCell::new(vec![]),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let rev = 17;
        let expected = (
            id,
            rev,
            PersonDto::new(
                "Alice",
                date(2020, 10, 1),
                Some(date(2100, 9, 8)),
                None,
                rev + 1,
            )
            .into(),
        );
        let _ = usecase.death(id, date(2100, 9, 8)).run(&mut ());

        // DAO のメソッドの呼び出し記録の検証
        assert_eq!(usecase.dao.insert.borrow().len(), 0);
        assert_eq!(*usecase.dao.select.borrow(), 0);
        assert_eq!(usecase.dao.delete.borrow().len(), 0);

        // Usecase の引数が DAO にそのまま渡されていることを検証
        assert_eq!(usecase.dao.fetch.borrow()[0], expected.0);
        assert_eq!(usecase.dao.save.borrow()[0], expected);
    }
    #[test]
    fn test_remove() {
        let id = Uuid::now_v7();
        let dao = SpyPersonDao {
            insert: RefCell::new(vec![]),
            inserted_id: id, // 使わない
            fetch: RefCell::new(vec![]),
            fetch_result: Ok(None),
            select: RefCell::new(0),
            save: RefCell::new(vec![]),
            delete: RefCell::new(vec![]),
        };
        let mut usecase = TargetPersonUsecase { dao };

        let expected = id;
        let _ = usecase.remove(id).run(&mut ());

        // DAO のメソッドの呼び出し記録の検証
        assert_eq!(usecase.dao.insert.borrow().len(), 0);
        assert_eq!(usecase.dao.fetch.borrow().len(), 0);
        assert_eq!(*usecase.dao.select.borrow(), 0);
        assert_eq!(usecase.dao.save.borrow().len(), 0);
        assert_eq!(usecase.dao.delete.borrow().len(), 1);

        // Usecase の引数が DAO にそのまま渡されていることを確認
        assert_eq!(usecase.dao.delete.borrow()[0], expected);
    }
}

// # エラー系スタブテスト
//
// ## 目的
//
//   DAO がエラーを返した場合の Usecase の挙動を保障する
//
// ## 方針
//
//   DAO の各メソッドで任意を結果を返せるようにして Usecase のメソッドを呼び出して Usecase の結果を確認する
//
// ## 実装
//
//                          Test Double
//        +---------+      +------------+
//        | Usecase |      | Stub DAO   |
//        | ======= |      | ========== |
//        |         |      |            |
//   ---->| ------> |----->| --->       |
//        |         |      |            |
//   <-c--| <--c--- |<-----| <--- any error
//     |  +----|----+      +------------+
//     |       |
//     |       +-- テスト対象
//     |
//     +-- ここを確認する
//
//   1. DAO のメソッドが任意の結果を返せる種類の DAO 構造体を用意する
//      この DAO 構造体はスタブであり、Usecase への間接的な入力のみ制御する
//   2. その構造体を Usecase にプラグインする
//   3. Usecase のメソッドを呼び出す
//   4. Usecase のメソッドからの戻り値を確認する
//
// ## 注意
//
#[cfg(test)]
mod error_stub_tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::{date, Revision};
    use crate::dto::PersonDto;

    struct StubPersonDao {
        insert_result: Result<PersonId, DaoError>,
        fetch_result: Result<Option<PersonDto>, DaoError>,
        select_result: Result<Vec<(PersonId, PersonDto)>, DaoError>,
        save_result: Result<(), DaoError>,
        delete_result: Result<(), DaoError>,
    }
    // Ctx 不要なので () にしている
    impl PersonDao<()> for StubPersonDao {
        fn insert(
            &self,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
            tx_rs::with_tx(|()| self.insert_result.clone())
        }
        fn fetch(
            &self,
            _id: PersonId,
        ) -> impl tx_rs::Tx<(), Item = Option<PersonDto>, Err = DaoError> {
            tx_rs::with_tx(|()| self.fetch_result.clone())
        }
        fn select(&self) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, PersonDto)>, Err = DaoError> {
            tx_rs::with_tx(|()| self.select_result.clone())
        }
        fn save(
            &self,
            _id: PersonId,
            _revision: Revision,
            _person: PersonDto,
        ) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            tx_rs::with_tx(move |()| self.save_result.clone())
        }
        fn delete(&self, _id: PersonId) -> impl tx_rs::Tx<(), Item = (), Err = DaoError> {
            tx_rs::with_tx(move |()| self.delete_result.clone())
        }
    }

    struct TargetPersonUsecase {
        dao: StubPersonDao,
    }
    impl HavePersonDao<()> for TargetPersonUsecase {
        fn get_dao(&self) -> &impl PersonDao<()> {
            &self.dao
        }
    }
    impl PersonUsecase<()> for TargetPersonUsecase {}

    #[test]
    fn test_entry() {
        let dao = StubPersonDao {
            insert_result: Err(DaoError::InsertError("valid dao".to_string())),
            fetch_result: Ok(None),    // 使わない
            select_result: Ok(vec![]), // 使わない
            save_result: Ok(()),       // 使わない
            delete_result: Ok(()),     // 使わない
        };
        let expected = UsecaseError::EntryPersonFailed(dao.insert_result.clone().unwrap_err());

        let mut usecase = TargetPersonUsecase { dao };

        let person = PersonDto::new("Alice", date(2012, 11, 2), None, None, 0);
        let result = usecase.entry(person).run(&mut ());

        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), expected);
    }

    #[test]
    fn test_find() {
        let id = Uuid::now_v7();
        let dao = StubPersonDao {
            insert_result: Ok(id), // 使わない
            fetch_result: Err(DaoError::SelectError("valid dao".to_string())),
            select_result: Ok(vec![]), // 使わない
            save_result: Ok(()),       // 使わない
            delete_result: Ok(()),     // 使わない
        };
        let expected = UsecaseError::FindPersonFailed(dao.fetch_result.clone().unwrap_err());

        let mut usecase = TargetPersonUsecase { dao };

        let result = usecase.find(id).run(&mut ());

        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), expected);
    }

    #[test]
    fn test_entry_and_verify_insert_error() {
        let dao = StubPersonDao {
            insert_result: Err(DaoError::InsertError("valid dao".to_string())),
            fetch_result: Ok(None),    // 使わない
            select_result: Ok(vec![]), // 使わない
            save_result: Ok(()),       // 使わない
            delete_result: Ok(()),     // 使わない
        };
        let expected =
            UsecaseError::EntryAndVerifyPersonFailed(dao.insert_result.clone().unwrap_err());

        let mut usecase = TargetPersonUsecase { dao };

        let person = PersonDto::new("Alice", date(2012, 11, 2), None, None, 0);
        let result = usecase.entry_and_verify(person).run(&mut ());

        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), expected);
    }

    #[test]
    fn test_entry_and_verify_fetch_error() {
        let id = Uuid::now_v7();
        let dao = StubPersonDao {
            insert_result: Ok(id),
            fetch_result: Err(DaoError::SelectError("valid dao".to_string())),
            select_result: Ok(vec![]), // 使わない
            save_result: Ok(()),       // 使わない
            delete_result: Ok(()),     // 使わない
        };
        let expected =
            UsecaseError::EntryAndVerifyPersonFailed(dao.fetch_result.clone().unwrap_err());

        let mut usecase = TargetPersonUsecase { dao };

        let person = PersonDto::new("Alice", date(2012, 11, 2), None, None, 0);
        let result = usecase.entry_and_verify(person).run(&mut ());

        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), expected);
    }

    #[test]
    fn test_collect() {
        let id = Uuid::now_v7();
        let dao = StubPersonDao {
            insert_result: Ok(id),  // 使わない
            fetch_result: Ok(None), // 使わない
            select_result: Err(DaoError::SelectError("valid dao".to_string())),
            save_result: Ok(()),   // 使わない
            delete_result: Ok(()), // 使わない
        };
        let expected = UsecaseError::CollectPersonFailed(dao.select_result.clone().unwrap_err());

        let mut usecase = TargetPersonUsecase { dao };

        let result = usecase.collect().run(&mut ());

        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), expected);
    }
    #[test]
    fn test_death_save_error() {
        let id = Uuid::now_v7();
        let dao = StubPersonDao {
            insert_result: Ok(id), // 使わない
            fetch_result: Ok(Some(PersonDto::new(
                "Alice",
                date(2020, 5, 5),
                None,
                None,
                0,
            ))),
            select_result: Ok(vec![]), // 使わない
            save_result: Err(DaoError::UpdateError("valid dao".to_string())),
            delete_result: Ok(()), // 使わない
        };
        let expected =
            UsecaseError::SavePersonFailed(DaoError::UpdateError("valid dao".to_string()));

        let mut usecase = TargetPersonUsecase { dao };

        let result = usecase.death(id, date(2100, 10, 15)).run(&mut ());

        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), expected);
    }
    #[test]
    fn test_death_fetch_error() {
        let id = Uuid::now_v7();
        let dao = StubPersonDao {
            insert_result: Ok(id), // 使わない
            fetch_result: Err(DaoError::SelectError("valid dao".to_string())),
            select_result: Ok(vec![]), // 使わない
            save_result: Ok(()),       // 使わない
            delete_result: Ok(()),     // 使わない
        };
        let expected =
            UsecaseError::FindPersonFailed(DaoError::SelectError("valid dao".to_string()));

        let mut usecase = TargetPersonUsecase { dao };

        let result = usecase.death(id, date(2100, 10, 15)).run(&mut ());

        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), expected);
    }
    #[test]
    fn test_remove() {
        let id = Uuid::now_v7();
        let dao = StubPersonDao {
            insert_result: Ok(id),     // 使わない
            fetch_result: Ok(None),    // 使わない
            select_result: Ok(vec![]), // 使わない
            save_result: Ok(()),       // 使わない
            delete_result: Err(DaoError::DeleteError("valid dao".to_string())),
        };
        let expected = UsecaseError::RemovePersonFailed(dao.delete_result.clone().unwrap_err());

        let mut usecase = TargetPersonUsecase { dao };

        let result = usecase.remove(id).run(&mut ());

        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), expected);
    }
}
