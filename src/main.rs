use postgres::{Client, NoTls, Transaction};

pub mod tx {
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

    /*
     * TODO: 現時点では FnOnce(&mut Ctx) -> Result<T, E> という型のクロージャのみ Tx として扱える
     * Tx をライブラリ化するにあたっては、以下の各ライブラリ関数に対して各々に対応する型を定義し、Tx トレイトを実装する
     * これによりライブラリ利用者が Tx を自由に設計できるようになる
     */

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

type PersonId = i32;
#[derive(Debug, Clone, Eq, PartialEq)]
struct Person {
    name: String,
    age: i32,
    data: Option<Vec<u8>>,
}
impl Person {
    fn new(name: &str, age: i32, data: Option<&[u8]>) -> Self {
        Self {
            name: name.to_string(),
            age,
            data: data.map(|d| d.to_vec()),
        }
    }
}

trait PersonRepository {
    type Tx;

    fn insert_person(tx: &mut Self::Tx, person: &Person) -> PersonId;
    fn fetch_person(tx: &mut Self::Tx, id: PersonId) -> Option<Person>;
}

struct PersonRepositoryImpl<'a> {
    conn_str: &'a str,
    client: Client,
}
impl<'a> PersonRepositoryImpl<'a> {
    fn new(conn_str: &'a str) -> Self {
        let client = Client::connect(conn_str, NoTls).unwrap();
        Self { conn_str, client }
    }

    fn with_tx<F, T, E>(&mut self, q: F) -> Result<T, E>
    where
        F: FnOnce(&mut Transaction<'_>) -> Result<T, E>,
    {
        let mut tx = self.client.transaction().unwrap();

        match q(&mut tx) {
            Ok(v) => {
                tx.commit().unwrap();
                Ok(v)
            }

            Err(e) => {
                tx.rollback().unwrap();
                Err(e)
            }
        }
    }
}
impl<'a> PersonRepository for PersonRepositoryImpl<'a> {
    type Tx = Transaction<'a>;

    fn insert_person(tx: &mut Self::Tx, person: &Person) -> PersonId {
        let row = tx
            .query_one(
                "INSERT INTO person (name, age, data) VALUES ($1, $2, $3) RETURNING id",
                &[&person.name, &person.age, &person.data],
            )
            .unwrap();

        row.get(0)
    }

    fn fetch_person(tx: &mut Self::Tx, id: PersonId) -> Option<Person> {
        match tx.query_one("SELECT name, age, data FROM person WHERE id = $1", &[&id]) {
            Ok(row) => Some(Person::new(row.get(0), row.get(1), row.get(2))),
            Err(e) => {
                eprintln!("error fetching person: {}", e);
                None
            }
        }
    }
}

fn insert_person(tx: &mut Transaction<'_>, person: &Person) -> PersonId {
    // execute ではなく query を使うことで id を取得できる
    let row = tx
        .query_one(
            "INSERT INTO person (name, age, data) VALUES ($1, $2, $3) RETURNING id",
            &[&person.name, &person.age, &person.data],
        )
        .unwrap();

    row.get(0)
}

fn fetch_person(tx: &mut Transaction<'_>, id: PersonId) -> Option<Person> {
    match tx.query_one("SELECT name, age, data FROM person WHERE id = $1", &[&id]) {
        Ok(row) => Some(Person::new(row.get(0), row.get(1), row.get(2))),
        Err(e) => {
            eprintln!("error fetching person: {}", e);
            None
        }
    }
}

fn with_tx<F, T, E>(client: &mut Client, q: F) -> Result<T, E>
where
    F: FnOnce(&mut Transaction<'_>) -> Result<T, E>,
{
    let mut tx = client.transaction().unwrap();

    match q(&mut tx) {
        Ok(ret) => {
            tx.commit().unwrap();
            Ok(ret)
        }
        Err(e) => {
            tx.rollback().unwrap();
            Err(e)
        }
    }
}

fn main() {
    let mut client = Client::connect(
        "postgresql://admin:adminpass@localhost:15432/sampledb",
        NoTls,
    )
    .unwrap();

    let person = with_tx(&mut client, |tx| {
        let person = Person::new("Ferris", 42, None);
        let id = insert_person(tx, &person);
        fetch_person(tx, id).ok_or(())
    });

    match person {
        Ok(p) => println!("found person {:?}", p),
        Err(e) => println!("no person found: {:?}", e),
    }
}
