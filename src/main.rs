use core::fmt;
use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

use postgres::{Client, NoTls};
use thiserror::Error;

pub mod tx_rs {
    pub trait Tx<Ctx> {
        type Item;
        type Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err>;

        fn map<F, T>(self, f: F) -> Map<Self, F>
        where
            F: FnOnce(Self::Item) -> T,
            Self: Sized,
        {
            Map { tx1: self, f }
        }
        fn and_then<Tx2, F>(self, f: F) -> AndThen<Self, F>
        where
            Tx2: Tx<Ctx, Err = Self::Err>,
            F: FnOnce(Self::Item) -> Tx2,
            Self: Sized,
        {
            AndThen { tx1: self, f }
        }
        fn then<Tx2, F>(self, f: F) -> Then<Self, F>
        where
            Tx2: Tx<Ctx, Err = Self::Err>,
            F: FnOnce(Result<Self::Item, Self::Err>) -> Tx2,
            Self: Sized,
        {
            Then { tx1: self, f }
        }
        fn or_else<Tx2, F>(self, f: F) -> OrElse<Self, F>
        where
            Tx2: Tx<Ctx, Item = Self::Item, Err = Self::Err>,
            F: FnOnce(Self::Err) -> Tx2,
            Self: Sized,
        {
            OrElse { tx1: self, f }
        }
        fn join<Tx2>(self, tx2: Tx2) -> Join<Self, Tx2>
        where
            Tx2: Tx<Ctx, Item = Self::Item, Err = Self::Err>,
            Self: Sized,
        {
            Join { tx1: self, tx2 }
        }
        fn join3<Tx2, Tx3>(self, tx2: Tx2, tx3: Tx3) -> Join3<Self, Tx2, Tx3>
        where
            Tx2: Tx<Ctx, Item = Self::Item, Err = Self::Err>,
            Tx3: Tx<Ctx, Item = Self::Item, Err = Self::Err>,
            Self: Sized,
        {
            Join3 {
                tx1: self,
                tx2,
                tx3,
            }
        }
        fn join4<Tx2, Tx3, Tx4>(self, tx2: Tx2, tx3: Tx3, tx4: Tx4) -> Join4<Self, Tx2, Tx3, Tx4>
        where
            Tx2: Tx<Ctx, Item = Self::Item, Err = Self::Err>,
            Tx3: Tx<Ctx, Item = Self::Item, Err = Self::Err>,
            Tx4: Tx<Ctx, Item = Self::Item, Err = Self::Err>,
            Self: Sized,
        {
            Join4 {
                tx1: self,
                tx2,
                tx3,
                tx4,
            }
        }
        fn map_err<F, E>(self, f: F) -> MapErr<Self, F>
        where
            F: FnOnce(Self::Err) -> E,
            Self: Sized,
        {
            MapErr { tx1: self, f }
        }
        fn try_map<F, T, E>(self, f: F) -> TryMap<Self, F>
        where
            F: FnOnce(Self::Item) -> Result<T, E>,
            Self: Sized,
        {
            TryMap { tx1: self, f }
        }
        fn recover<F, T, E>(self, f: F) -> Recover<Self, F>
        where
            F: FnOnce(Self::Err) -> Result<T, E>,
            Self: Sized,
        {
            Recover { tx1: self, f }
        }
        fn try_recover<F, T, E>(self, f: F) -> TryRecover<Self, F>
        where
            F: FnOnce(Self::Err) -> Result<T, E>,
            Self: Sized,
        {
            TryRecover { tx1: self, f }
        }
        fn abort<F, T>(self, f: F) -> Abort<Self, F>
        where
            F: FnOnce(Self::Err) -> T,
            Self: Sized,
        {
            Abort { tx1: self, f }
        }
        fn try_abort<F, T, E>(self, f: F) -> TryAbort<Self, F>
        where
            F: FnOnce(Self::Err) -> Result<T, E>,
            Self: Sized,
        {
            TryAbort { tx1: self, f }
        }
    }

    impl<Ctx, T, E, F> Tx<Ctx> for F
    where
        F: FnOnce(&mut Ctx) -> Result<T, E>,
    {
        type Item = T;
        type Err = E;
        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            self(ctx)
        }
    }

    fn map<Ctx, Tx1, F, T>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<T, Tx1::Err>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Item) -> T,
    {
        move |ctx| match tx1.run(ctx) {
            Ok(x) => Ok(f(x)),
            Err(e) => Err(e),
        }
    }

    pub struct Map<Tx1, F> {
        tx1: Tx1,
        f: F,
    }
    impl<Ctx, Tx1, T, F> Tx<Ctx> for Map<Tx1, F>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Item) -> T,
    {
        type Item = T;
        type Err = Tx1::Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            map(self.tx1, self.f)(ctx)
        }
    }

    fn and_then<Ctx, Tx1, Tx2, F>(
        tx1: Tx1,
        f: F,
    ) -> impl FnOnce(&mut Ctx) -> Result<Tx2::Item, Tx1::Err>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Err = Tx1::Err>,
        F: FnOnce(Tx1::Item) -> Tx2,
    {
        move |ctx| match tx1.run(ctx) {
            Ok(x) => f(x).run(ctx),
            Err(e) => Err(e),
        }
    }

    pub struct AndThen<Tx1, F> {
        tx1: Tx1,
        f: F,
    }
    impl<Ctx, Tx1, Tx2, F> Tx<Ctx> for AndThen<Tx1, F>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Err = Tx1::Err>,
        F: FnOnce(Tx1::Item) -> Tx2,
    {
        type Item = Tx2::Item;
        type Err = Tx1::Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            and_then(self.tx1, self.f)(ctx)
        }
    }

    fn then<Ctx, Tx1, Tx2, F>(
        tx1: Tx1,
        f: F,
    ) -> impl FnOnce(&mut Ctx) -> Result<Tx2::Item, Tx1::Err>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Err = Tx1::Err>,
        F: FnOnce(Result<Tx1::Item, Tx1::Err>) -> Tx2,
    {
        move |ctx| f(tx1.run(ctx)).run(ctx)
    }

    pub struct Then<Tx1, F> {
        tx1: Tx1,
        f: F,
    }
    impl<Ctx, Tx1, Tx2, F> Tx<Ctx> for Then<Tx1, F>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Err = Tx1::Err>,
        F: FnOnce(Result<Tx1::Item, Tx1::Err>) -> Tx2,
    {
        type Item = Tx2::Item;
        type Err = Tx1::Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            then(self.tx1, self.f)(ctx)
        }
    }

    fn or_else<Ctx, Tx1, Tx2, F>(
        tx1: Tx1,
        f: F,
    ) -> impl FnOnce(&mut Ctx) -> Result<Tx2::Item, Tx1::Err>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Item = Tx1::Item, Err = Tx1::Err>,
        F: FnOnce(Tx1::Err) -> Tx2,
    {
        move |ctx| match tx1.run(ctx) {
            Ok(t) => Ok(t),
            Err(e) => f(e).run(ctx),
        }
    }

    pub struct OrElse<Tx1, F> {
        tx1: Tx1,
        f: F,
    }
    impl<Ctx, Tx1, Tx2, F> Tx<Ctx> for OrElse<Tx1, F>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Item = Tx1::Item, Err = Tx1::Err>,
        F: FnOnce(Tx1::Err) -> Tx2,
    {
        type Item = Tx1::Item;
        type Err = Tx1::Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            or_else(self.tx1, self.f)(ctx)
        }
    }

    fn join<Ctx, Tx1, Tx2>(
        tx1: Tx1,
        tx2: Tx2,
    ) -> impl FnOnce(&mut Ctx) -> Result<(Tx1::Item, Tx2::Item), Tx1::Err>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Err = Tx1::Err>,
    {
        move |ctx| match (tx1.run(ctx), tx2.run(ctx)) {
            (Ok(t), Ok(u)) => Ok((t, u)),
            (Err(e), _) | (_, Err(e)) => Err(e),
        }
    }

    pub struct Join<Tx1, Tx2> {
        tx1: Tx1,
        tx2: Tx2,
    }
    impl<Ctx, Tx1, Tx2> Tx<Ctx> for Join<Tx1, Tx2>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Err = Tx1::Err>,
    {
        type Item = (Tx1::Item, Tx2::Item);
        type Err = Tx1::Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            join(self.tx1, self.tx2)(ctx)
        }
    }

    fn join3<Ctx, Tx1, Tx2, Tx3>(
        tx1: Tx1,
        tx2: Tx2,
        tx3: Tx3,
    ) -> impl FnOnce(&mut Ctx) -> Result<(Tx1::Item, Tx2::Item, Tx3::Item), Tx1::Err>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Err = Tx1::Err>,
        Tx3: Tx<Ctx, Err = Tx1::Err>,
    {
        move |ctx| match (tx1.run(ctx), tx2.run(ctx), tx3.run(ctx)) {
            (Ok(t), Ok(u), Ok(v)) => Ok((t, u, v)),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
        }
    }

    pub struct Join3<Tx1, Tx2, Tx3> {
        tx1: Tx1,
        tx2: Tx2,
        tx3: Tx3,
    }
    impl<Ctx, Tx1, Tx2, Tx3> Tx<Ctx> for Join3<Tx1, Tx2, Tx3>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Err = Tx1::Err>,
        Tx3: Tx<Ctx, Err = Tx1::Err>,
    {
        type Item = (Tx1::Item, Tx2::Item, Tx3::Item);
        type Err = Tx1::Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            join3(self.tx1, self.tx2, self.tx3)(ctx)
        }
    }

    fn join4<Ctx, Tx1, Tx2, Tx3, Tx4>(
        tx1: Tx1,
        tx2: Tx2,
        tx3: Tx3,
        tx4: Tx4,
    ) -> impl FnOnce(&mut Ctx) -> Result<(Tx1::Item, Tx2::Item, Tx3::Item, Tx4::Item), Tx1::Err>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Err = Tx1::Err>,
        Tx3: Tx<Ctx, Err = Tx1::Err>,
        Tx4: Tx<Ctx, Err = Tx1::Err>,
    {
        move |ctx| match (tx1.run(ctx), tx2.run(ctx), tx3.run(ctx), tx4.run(ctx)) {
            (Ok(t), Ok(u), Ok(v), Ok(w)) => Ok((t, u, v, w)),
            (Err(e), _, _, _) | (_, Err(e), _, _) | (_, _, Err(e), _) | (_, _, _, Err(e)) => Err(e),
        }
    }

    pub struct Join4<Tx1, Tx2, Tx3, Tx4> {
        tx1: Tx1,
        tx2: Tx2,
        tx3: Tx3,
        tx4: Tx4,
    }
    impl<Ctx, Tx1, Tx2, Tx3, Tx4> Tx<Ctx> for Join4<Tx1, Tx2, Tx3, Tx4>
    where
        Tx1: Tx<Ctx>,
        Tx2: Tx<Ctx, Err = Tx1::Err>,
        Tx3: Tx<Ctx, Err = Tx1::Err>,
        Tx4: Tx<Ctx, Err = Tx1::Err>,
    {
        type Item = (Tx1::Item, Tx2::Item, Tx3::Item, Tx4::Item);
        type Err = Tx1::Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            join4(self.tx1, self.tx2, self.tx3, self.tx4)(ctx)
        }
    }

    fn map_err<Ctx, Tx1, F, E>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx1::Item, E>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Err) -> E,
    {
        move |ctx| match tx1.run(ctx) {
            Ok(t) => Ok(t),
            Err(e) => Err(f(e)),
        }
    }

    pub struct MapErr<Tx1, F> {
        tx1: Tx1,
        f: F,
    }
    impl<Ctx, Tx1, F, E> Tx<Ctx> for MapErr<Tx1, F>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Err) -> E,
    {
        type Item = Tx1::Item;
        type Err = E;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            map_err(self.tx1, self.f)(ctx)
        }
    }

    fn try_map<Ctx, Tx1, F, T>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<T, Tx1::Err>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Item) -> Result<T, Tx1::Err>,
    {
        move |ctx| match tx1.run(ctx) {
            Ok(t) => f(t),
            Err(e) => Err(e),
        }
    }

    pub struct TryMap<Tx1, F> {
        tx1: Tx1,
        f: F,
    }
    impl<Ctx, Tx1, F, T> Tx<Ctx> for TryMap<Tx1, F>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Item) -> Result<T, Tx1::Err>,
    {
        type Item = T;
        type Err = Tx1::Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            try_map(self.tx1, self.f)(ctx)
        }
    }

    fn recover<Ctx, Tx1, F>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx1::Item, Tx1::Err>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Err) -> Tx1::Item,
    {
        move |ctx| match tx1.run(ctx) {
            Ok(t) => Ok(t),
            Err(e) => Ok(f(e)),
        }
    }

    pub struct Recover<Tx1, F> {
        tx1: Tx1,
        f: F,
    }
    impl<Ctx, Tx1, F> Tx<Ctx> for Recover<Tx1, F>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Err) -> Tx1::Item,
    {
        type Item = Tx1::Item;
        type Err = Tx1::Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            recover(self.tx1, self.f)(ctx)
        }
    }

    fn try_recover<Ctx, Tx1, F, E>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx1::Item, E>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Err) -> Result<Tx1::Item, E>,
    {
        move |ctx| match tx1.run(ctx) {
            Ok(t) => Ok(t),
            Err(e) => f(e),
        }
    }

    pub struct TryRecover<Tx1, F> {
        tx1: Tx1,
        f: F,
    }
    impl<Ctx, Tx1, F, E> Tx<Ctx> for TryRecover<Tx1, F>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Err) -> Result<Tx1::Item, E>,
    {
        type Item = Tx1::Item;
        type Err = E;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            try_recover(self.tx1, self.f)(ctx)
        }
    }

    fn abort<Ctx, Tx1, F>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx1::Item, Tx1::Err>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Item) -> Tx1::Err,
    {
        move |ctx| match tx1.run(ctx) {
            Ok(t) => Err(f(t)),
            Err(e) => Err(e),
        }
    }

    pub struct Abort<Tx1, F> {
        tx1: Tx1,
        f: F,
    }
    impl<Ctx, Tx1, F> Tx<Ctx> for Abort<Tx1, F>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Item) -> Tx1::Err,
    {
        type Item = Tx1::Item;
        type Err = Tx1::Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            abort(self.tx1, self.f)(ctx)
        }
    }

    fn try_abort<Ctx, Tx1, F>(
        tx1: Tx1,
        f: F,
    ) -> impl FnOnce(&mut Ctx) -> Result<Tx1::Item, Tx1::Err>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Item) -> Result<Tx1::Item, Tx1::Err>,
    {
        move |ctx| match tx1.run(ctx) {
            Ok(t) => f(t),
            Err(e) => Err(e),
        }
    }

    pub struct TryAbort<Tx1, F> {
        tx1: Tx1,
        f: F,
    }
    impl<Ctx, Tx1, F> Tx<Ctx> for TryAbort<Tx1, F>
    where
        Tx1: Tx<Ctx>,
        F: FnOnce(Tx1::Item) -> Result<Tx1::Item, Tx1::Err>,
    {
        type Item = Tx1::Item;
        type Err = Tx1::Err;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            try_abort(self.tx1, self.f)(ctx)
        }
    }

    pub fn with_tx<Ctx, F, T, E>(f: F) -> WithTx<F>
    where
        F: FnOnce(&mut Ctx) -> Result<T, E>,
    {
        WithTx { f }
    }
    pub struct WithTx<F> {
        f: F,
    }
    impl<Ctx, F, T, E> Tx<Ctx> for WithTx<F>
    where
        F: FnOnce(&mut Ctx) -> Result<T, E>,
    {
        type Item = T;
        type Err = E;

        fn run(self, ctx: &mut Ctx) -> Result<Self::Item, Self::Err> {
            (self.f)(ctx)
        }
    }
}
use tx_rs::Tx;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("dummy error")]
    Dummy,
}

type PersonId = i32;
#[derive(Debug)]
struct Person {
    name: String,
    age: i32,
    data: Option<String>,
}
impl Person {
    pub fn new(name: &str, age: i32, data: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            age,
            data: data.map(|d| d.to_string()),
        }
    }
}
impl fmt::Display for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Person {{ name: {}, age: {}, data: {:?} }}",
            self.name, self.age, self.data,
        )
    }
}

#[derive(Error, Debug)]
pub enum DaoError {
    #[error("insert error")]
    InsertError,
    #[error("select error")]
    SelectError,
}
trait PersonDao<Ctx> {
    fn insert(&self, person: Person) -> impl tx_rs::Tx<Ctx, Item = PersonId, Err = DaoError>;
    fn fetch(&self, id: PersonId) -> impl tx_rs::Tx<Ctx, Item = Option<Person>, Err = DaoError>;
    fn select(&self) -> impl tx_rs::Tx<Ctx, Item = Vec<(PersonId, Person)>, Err = DaoError>;
}
#[derive(Debug, Clone)]
struct PgPersonDao;
impl<'a> PersonDao<postgres::Transaction<'a>> for PgPersonDao {
    fn insert(
        &self,
        person: Person,
    ) -> impl tx_rs::Tx<postgres::Transaction<'a>, Item = PersonId, Err = DaoError> {
        tx_rs::with_tx(move |tx: &mut postgres::Transaction<'_>| {
            tx.query_one(
                "INSERT INTO person (name, age, data) VALUES ($1, $2, $3) RETURNING id",
                &[
                    &person.name,
                    &person.age,
                    &person.data.map(|d| d.as_str().as_bytes().to_vec()),
                ],
            )
            .map(|row| row.get::<usize, PersonId>(0))
            .map_err(|_| DaoError::InsertError)
        })
    }
    fn fetch(
        &self,
        id: PersonId,
    ) -> impl tx_rs::Tx<postgres::Transaction<'a>, Item = Option<Person>, Err = DaoError> {
        tx_rs::with_tx(move |tx: &mut postgres::Transaction<'_>| {
            tx.query_opt("SELECT name, age, data FROM person WHERE id = $1", &[&id])
                .map(|row| {
                    row.map(|row| {
                        let name = row.get::<usize, &str>(0);
                        let age = row.get::<usize, i32>(1);
                        let data = std::str::from_utf8(row.get::<usize, &[u8]>(2)).ok();

                        Person::new(name, age, data)
                    })
                })
                .map_err(|_| DaoError::SelectError)
        })
    }
    fn select(
        &self,
    ) -> impl tx_rs::Tx<postgres::Transaction<'a>, Item = Vec<(PersonId, Person)>, Err = DaoError>
    {
        tx_rs::with_tx(|tx: &mut postgres::Transaction<'_>| {
            tx.query("SELECT id, name, age, data FROM person", &[])
                .map(|rows| {
                    rows.iter()
                        .map(|row| {
                            let id = row.get::<usize, PersonId>(0);
                            let name = row.get::<usize, &str>(1);
                            let age = row.get::<usize, i32>(2);
                            let data = std::str::from_utf8(row.get::<usize, &[u8]>(3)).ok();
                            let person = Person::new(name, age, data);

                            (id, person)
                        })
                        .collect()
                })
                .map_err(|_| DaoError::SelectError)
        })
    }
}

trait HavePersonDao<Ctx> {
    fn get_dao(&self) -> Box<&impl PersonDao<Ctx>>;
}
trait PersonUsecase<Ctx>: HavePersonDao<Ctx> {
    fn entry<'a>(
        &'a mut self,
        person: Person,
    ) -> impl tx_rs::Tx<Ctx, Item = PersonId, Err = MyError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        dao.insert(person).map_err(|_| MyError::Dummy)
    }
    fn find<'a>(
        &'a mut self,
        id: PersonId,
    ) -> impl tx_rs::Tx<Ctx, Item = Option<Person>, Err = MyError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        dao.fetch(id).map_err(|_| MyError::Dummy)
    }
    fn entry_and_verify<'a>(
        &'a mut self,
        person: Person,
    ) -> impl tx_rs::Tx<Ctx, Item = (PersonId, Person), Err = MyError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        dao.insert(person)
            .and_then(move |id| dao.fetch(id).map(move |p| (id, p.unwrap())))
            .map_err(|_| MyError::Dummy)
    }
    fn collect<'a>(
        &'a mut self,
    ) -> impl tx_rs::Tx<Ctx, Item = Vec<(PersonId, Person)>, Err = MyError>
    where
        Ctx: 'a,
    {
        let dao = self.get_dao();
        dao.select().map_err(|_| MyError::Dummy)
    }
}
#[derive(Debug, Clone)]
struct PersonUsecaseImpl {
    dao: Rc<PgPersonDao>,
}
impl PersonUsecaseImpl {
    pub fn new(dao: Rc<PgPersonDao>) -> Self {
        Self { dao }
    }
}
impl HavePersonDao<postgres::Transaction<'_>> for PersonUsecaseImpl {
    fn get_dao<'a>(&'a self) -> Box<&PgPersonDao> {
        Box::new(&self.dao)
    }
}
impl<'a> PersonUsecase<postgres::Transaction<'a>> for PersonUsecaseImpl {}

struct PersonApi {
    db_client: Client,
    usecase: Rc<RefCell<PersonUsecaseImpl>>,
}
impl PersonApi {
    pub fn new(db_url: &str) -> Self {
        let dao = PgPersonDao;
        let usecase = PersonUsecaseImpl::new(Rc::new(dao));
        let db_client = Client::connect(db_url, NoTls).unwrap();

        Self {
            db_client,
            usecase: Rc::new(RefCell::new(usecase)),
        }
    }

    // api is responsible for transaction management
    fn run_tx<T, F>(&mut self, f: F) -> Result<T, MyError>
    where
        F: FnOnce(
            &mut RefMut<'_, PersonUsecaseImpl>,
            &mut postgres::Transaction<'_>,
        ) -> Result<T, MyError>,
    {
        let mut usecase = self.usecase.borrow_mut();
        let mut ctx = self.db_client.transaction().unwrap();

        let res = f(&mut usecase, &mut ctx);

        match res {
            Ok(v) => {
                ctx.commit().unwrap();
                Ok(v)
            }
            Err(_) => {
                ctx.rollback().unwrap();
                Err(MyError::Dummy)
            }
        }
    }

    // api: register person
    pub fn register(
        &mut self,
        name: &str,
        age: i32,
        data: &str,
    ) -> Result<(PersonId, Person), MyError> {
        self.run_tx(|usecase, ctx| {
            usecase
                .entry_and_verify(Person::new(name, age, Some(data)))
                .run(ctx)
        })
    }

    // api: batch import
    pub fn batch_import(&mut self, persons: Vec<Person>) -> Result<(), MyError> {
        self.run_tx(|usecase, ctx| {
            for person in persons {
                let res = usecase.entry(person).run(ctx);
                if res.is_err() {
                    return Err(MyError::Dummy);
                }
            }
            Ok(())
        })
    }

    // api: list all persons
    pub fn list_all(&mut self) -> Result<Vec<(PersonId, Person)>, MyError> {
        self.run_tx(|usecase, ctx| usecase.collect().run(ctx))
    }
}

fn main() {
    let mut api = PersonApi::new("postgres://admin:adminpass@localhost:15432/sampledb");

    // call api
    let (id, person) = api.register("cutsea", 53, "rustacean").unwrap();
    println!("id:{} {}", id, person);

    let persons = vec![
        Person::new("Gauss", 34, Some("King of Math")),
        Person::new("Galois", 20, Some("Group Theory")),
        Person::new("Euler", 76, Some("Euler's identity")),
        Person::new("Abel", 26, Some("Abel's theorem")),
    ];
    api.batch_import(persons).unwrap();

    let persons = api.list_all().expect("list all");
    for (id, person) in persons {
        println!("id:{} {}", id, person);
    }
}
