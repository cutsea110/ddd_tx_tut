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

// # 追跡型のテスト
//
// * 目的
//
//   Usecase の各メソッドが適切に DAO のメソッドを適切に呼び出していることを保障する
//   必要なメソッドを必要回数だけ呼び出していることと、不必要なメソッドを呼び出していないことを保障する
//   Usecase に渡った引数が適切に DAO のメソッドに渡されていることを保障する
//
// * 方針
//
//   DAO のメソッドを呼び出すたびに、その呼び出しを記録する
//   その記録をテストの最後で確認する
//
// * 実装
//
//   1. DAO のメソッド呼び出しを記録する種類の DAO 構造体を用意する
//   2. その構造体を Usecase にプラグインする
//   3. Usecase のメソッドを呼び出す
//   4. その後で DAO 構造体の記録を検証する
//
// * 注意
//
//   1. このテストは Usecase の実装を保障するものであって、DAO の実装を保障するものではない
//   2. このテストは Usecase のメソッドが適切に DAO のメソッドを呼び出していることを保障するものであって、
//      DAO のメソッドが適切にデータベースを操作していることを保障するものではない
//   3. このテストは Usecase のメソッドが不適切な DAO のメソッド呼び出しをしていないことを保障するものであって、
//      DAO のメソッドが不適切なデータベースの操作をしていないことを保障するものではない
//
#[cfg(test)]
mod trace {
    use std::cell::RefCell;

    use super::*;

    struct TracePersonDao {
        insert: RefCell<Vec<Person>>,
        fetch: RefCell<Vec<PersonId>>,
        select: RefCell<i32>,
    }
    // Ctx 不要なので () にしている
    impl PersonDao<()> for TracePersonDao {
        fn insert(&self, person: Person) -> impl tx_rs::Tx<(), Item = PersonId, Err = DaoError> {
            self.insert.borrow_mut().push(person);

            // 返り値には意味なし
            tx_rs::with_tx(|()| Ok(42 as PersonId))
        }
        fn fetch(&self, id: PersonId) -> impl tx_rs::Tx<(), Item = Option<Person>, Err = DaoError> {
            self.fetch.borrow_mut().push(id);

            // 返り値には意味なし
            tx_rs::with_tx(|()| Ok(None))
        }
        fn select(&self) -> impl tx_rs::Tx<(), Item = Vec<(PersonId, Person)>, Err = DaoError> {
            *self.select.borrow_mut() += 1;

            // 返り値には意味なし
            tx_rs::with_tx(|()| Ok(vec![]))
        }
    }

    struct TracePersonUsecase {
        dao: TracePersonDao,
    }
    impl HavePersonDao<()> for TracePersonUsecase {
        fn get_dao(&self) -> Box<&impl PersonDao<()>> {
            Box::new(&self.dao)
        }
    }
    impl PersonUsecase<()> for TracePersonUsecase {}

    #[test]
    fn test_entry() {
        let dao = TracePersonDao {
            insert: RefCell::new(vec![]),
            fetch: RefCell::new(vec![]),
            select: RefCell::new(0),
        };
        let mut usecase = TracePersonUsecase { dao };

        let person = Person::new("Alice", 20, None);
        let expected = person.clone();

        let _ = usecase.entry(person).run(&mut ()).unwrap();

        // DAO のメソッドの呼び出し記録の検証
        assert_eq!(usecase.dao.insert.borrow().len(), 1);
        assert_eq!(usecase.dao.fetch.borrow().len(), 0);
        assert_eq!(*usecase.dao.select.borrow(), 0);

        // Usecase の引数が DAO にそのまま渡されていることを検証
        assert_eq!(usecase.dao.insert.borrow()[0], expected);
    }

    #[test]
    fn test_find() {
        let dao = TracePersonDao {
            insert: RefCell::new(vec![]),
            fetch: RefCell::new(vec![]),
            select: RefCell::new(0),
        };
        let mut usecase = TracePersonUsecase { dao };

        let id: PersonId = 42;
        let expected = id;
        let _ = usecase.find(id).run(&mut ()).unwrap();

        // DAO のメソッドの呼び出し記録の検証
        assert_eq!(usecase.dao.insert.borrow().len(), 0);
        assert_eq!(usecase.dao.fetch.borrow().len(), 1);
        assert_eq!(*usecase.dao.select.borrow(), 0);

        // Usecase の引数が DAO にそのまま渡されていることを確認
        assert_eq!(usecase.dao.fetch.borrow()[0], expected);
    }

    #[test]
    fn test_collect() {
        let dao = TracePersonDao {
            insert: RefCell::new(vec![]),
            fetch: RefCell::new(vec![]),
            select: RefCell::new(0),
        };
        let mut usecase = TracePersonUsecase { dao };

        let _ = usecase.collect().run(&mut ()).unwrap();

        // DAO のメソッドの呼び出し記録の検証
        assert_eq!(usecase.dao.insert.borrow().len(), 0);
        assert_eq!(usecase.dao.fetch.borrow().len(), 0);
        assert_eq!(*usecase.dao.select.borrow(), 1);
    }
}
