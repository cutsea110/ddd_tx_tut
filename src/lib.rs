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

fn then<Ctx, Tx1, Tx2, F>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx2::Item, Tx1::Err>
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

fn or_else<Ctx, Tx1, Tx2, F>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx2::Item, Tx1::Err>
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

fn try_abort<Ctx, Tx1, F>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx1::Item, Tx1::Err>
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

#[cfg(test)]
mod test {
    use super::*;

    struct Transaction(i32);

    impl<Ctx> Tx<Ctx> for Transaction {
        type Item = i32;
        type Err = ();

        fn run(self, _: &mut Ctx) -> Result<Self::Item, Self::Err> {
            Ok(self.0)
        }
    }

    #[test]
    fn test_map() {
        let tx = with_tx(|_| Ok::<i32, ()>(21));
        assert_eq!(tx.map(|v| v * 2).run(&mut ()), Ok(42));

        let tx = with_tx(|_| Err::<i32, ()>(()));
        assert_eq!(tx.map(|v| v * 2).run(&mut ()), Err(()));
    }

    #[test]
    fn test_and_then() {
        let tx1 = with_tx(|_| Ok::<i32, ()>(21));
        let f = |v| with_tx(move |_| Ok::<i32, ()>(v * 2));
        assert_eq!(tx1.and_then(f).run(&mut ()), Ok(42));

        let tx1 = with_tx(|_| Err::<i32, ()>(()));
        let f = |v| with_tx(move |_| Ok::<i32, ()>(v * 2));
        assert_eq!(tx1.and_then(f).run(&mut ()), Err(()));
    }

    #[test]
    fn test_then() {
        let tx1 = with_tx(|_| Ok::<i32, ()>(21));
        let f = |v| {
            with_tx(move |_| match v {
                Ok(v) => Ok::<i32, ()>(v * 2),
                Err(e) => Err::<i32, ()>(e),
            })
        };
        assert_eq!(tx1.then(f).run(&mut ()), Ok(42));

        let tx1 = with_tx(|_| Err::<i32, ()>(()));
        let f = |v| {
            with_tx(move |_| match v {
                Ok(v) => Ok::<i32, ()>(v * 2),
                Err(e) => Err::<i32, ()>(e),
            })
        };
        assert_eq!(tx1.then(f).run(&mut ()), Err(()));
    }

    #[test]
    fn test_or_else() {
        let tx1 = with_tx(|_| Ok::<i32, ()>(21));
        let f = |_: ()| with_tx(|_| Ok::<i32, ()>(42));
        assert_eq!(tx1.or_else(f).run(&mut ()), Ok(21));

        let tx1 = with_tx(|_| Err::<i32, ()>(()));
        let f = |_: ()| with_tx(|_| Ok::<i32, ()>(42));
        assert_eq!(tx1.or_else(f).run(&mut ()), Ok(42));
    }
}
