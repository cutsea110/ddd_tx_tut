//! transaction library
//!
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
        Tx2: Tx<Ctx, Err = Self::Err>,
        Self: Sized,
    {
        Join { tx1: self, tx2 }
    }
    fn join3<Tx2, Tx3>(self, tx2: Tx2, tx3: Tx3) -> Join3<Self, Tx2, Tx3>
    where
        Tx2: Tx<Ctx, Err = Self::Err>,
        Tx3: Tx<Ctx, Err = Self::Err>,
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
        Tx2: Tx<Ctx, Err = Self::Err>,
        Tx3: Tx<Ctx, Err = Self::Err>,
        Tx4: Tx<Ctx, Err = Self::Err>,
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
    fn recover<F>(self, f: F) -> Recover<Self, F>
    where
        F: FnOnce(Self::Err) -> Self::Item,
        Self: Sized,
    {
        Recover { tx1: self, f }
    }
    fn try_recover<F>(self, f: F) -> TryRecover<Self, F>
    where
        F: FnOnce(Self::Err) -> Result<Self::Item, Self::Err>,
        Self: Sized,
    {
        TryRecover { tx1: self, f }
    }
    fn abort<F>(self, f: F) -> Abort<Self, F>
    where
        F: FnOnce(Self::Item) -> Self::Err,
        Self: Sized,
    {
        Abort { tx1: self, f }
    }
    fn try_abort<F>(self, f: F) -> TryAbort<Self, F>
    where
        F: FnOnce(Self::Item) -> Result<Self::Item, Self::Err>,
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

fn _map<Ctx, Tx1, F, T>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<T, Tx1::Err>
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
        _map(self.tx1, self.f)(ctx)
    }
}
#[cfg(test)]
mod test_map {
    use super::*;

    #[test]
    fn test_map() {
        let tx = with_tx(|_| Ok::<i32, ()>(21));
        assert_eq!(tx.map(|v| v * 2).run(&mut ()), Ok(42));

        let tx = with_tx(|_| Err::<i32, ()>(()));
        assert_eq!(tx.map(|v| v * 2).run(&mut ()), Err(()));
    }
}

fn _and_then<Ctx, Tx1, Tx2, F>(
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
        _and_then(self.tx1, self.f)(ctx)
    }
}
#[cfg(test)]
mod test_and_then {
    use super::*;

    #[test]
    fn test_and_then() {
        let tx1 = with_tx(|_| Ok::<i32, ()>(21));
        let f = |v| with_tx(move |_| Ok::<i32, ()>(v * 2));
        assert_eq!(tx1.and_then(f).run(&mut ()), Ok(42));

        let tx1 = with_tx(|_| Err::<i32, ()>(()));
        let f = |v| with_tx(move |_| Ok::<i32, ()>(v * 2));
        assert_eq!(tx1.and_then(f).run(&mut ()), Err(()));
    }
}

fn _then<Ctx, Tx1, Tx2, F>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx2::Item, Tx1::Err>
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
        _then(self.tx1, self.f)(ctx)
    }
}
#[cfg(test)]
mod test_then {
    use super::*;

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
}

fn _or_else<Ctx, Tx1, Tx2, F>(
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
        _or_else(self.tx1, self.f)(ctx)
    }
}
#[cfg(test)]
mod test_or_else {
    use super::*;

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

fn _join<Ctx, Tx1, Tx2>(
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
        _join(self.tx1, self.tx2)(ctx)
    }
}
#[cfg(test)]
mod test_join {
    use super::*;

    #[test]
    fn test_join() {
        let tx1 = with_tx(|_| Ok::<i32, ()>(42));
        let tx2 = with_tx(|_| Ok::<&str, ()>("ok"));
        assert_eq!(tx1.join(tx2).run(&mut ()), Ok((42, "ok")));

        let tx1 = with_tx(|_| Err::<i32, ()>(()));
        let tx2 = with_tx(|_| Ok::<&str, ()>("ng"));
        assert_eq!(tx1.join(tx2).run(&mut ()), Err(()));

        let tx1 = with_tx(|_| Ok::<i32, ()>(42));
        let tx2 = with_tx(|_| Err::<&str, ()>(()));
        assert_eq!(tx1.join(tx2).run(&mut ()), Err(()));
    }
}

fn _join3<Ctx, Tx1, Tx2, Tx3>(
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
        _join3(self.tx1, self.tx2, self.tx3)(ctx)
    }
}
#[cfg(test)]
mod test_join3 {
    use super::*;

    #[test]
    fn test_join3() {
        let tx1 = with_tx(|_| Ok::<i32, ()>(42));
        let tx2 = with_tx(|_| Ok::<&str, ()>("ok"));
        let tx3 = with_tx(|_| Ok::<bool, ()>(true));
        assert_eq!(tx1.join3(tx2, tx3).run(&mut ()), Ok((42, "ok", true)));

        let tx1 = with_tx(|_| Err::<i32, ()>(()));
        let tx2 = with_tx(|_| Ok::<&str, ()>("ng"));
        let tx3 = with_tx(|_| Ok::<bool, ()>(false));
        assert_eq!(tx1.join3(tx2, tx3).run(&mut ()), Err(()));

        let tx1 = with_tx(|_| Ok::<i32, ()>(42));
        let tx2 = with_tx(|_| Err::<&str, ()>(()));
        let tx3 = with_tx(|_| Ok::<bool, ()>(false));
        assert_eq!(tx1.join3(tx2, tx3).run(&mut ()), Err(()));

        let tx1 = with_tx(|_| Ok::<i32, ()>(42));
        let tx2 = with_tx(|_| Ok::<&str, ()>("ok"));
        let tx3 = with_tx(|_| Err::<bool, ()>(()));
        assert_eq!(tx1.join3(tx2, tx3).run(&mut ()), Err(()));
    }
}

fn _join4<Ctx, Tx1, Tx2, Tx3, Tx4>(
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
        _join4(self.tx1, self.tx2, self.tx3, self.tx4)(ctx)
    }
}
#[cfg(test)]
mod test_join4 {
    use super::*;

    #[test]
    fn test_join4() {
        let tx1 = with_tx(|_| Ok::<i32, ()>(42));
        let tx2 = with_tx(|_| Ok::<&str, ()>("ok"));
        let tx3 = with_tx(|_| Ok::<bool, ()>(true));
        let tx4 = with_tx(|_| Ok::<f32, ()>(3.14));
        assert_eq!(
            tx1.join4(tx2, tx3, tx4).run(&mut ()),
            Ok((42, "ok", true, 3.14))
        );

        let tx1 = with_tx(|_| Err::<i32, ()>(()));
        let tx2 = with_tx(|_| Ok::<&str, ()>("ng"));
        let tx3 = with_tx(|_| Ok::<bool, ()>(false));
        let tx4 = with_tx(|_| Ok::<f32, ()>(3.14));
        assert_eq!(tx1.join4(tx2, tx3, tx4).run(&mut ()), Err(()));

        let tx1 = with_tx(|_| Ok::<i32, ()>(42));
        let tx2 = with_tx(|_| Err::<&str, ()>(()));
        let tx3 = with_tx(|_| Ok::<bool, ()>(false));
        let tx4 = with_tx(|_| Ok::<f32, ()>(3.14));
        assert_eq!(tx1.join4(tx2, tx3, tx4).run(&mut ()), Err(()));

        let tx1 = with_tx(|_| Ok::<i32, ()>(42));
        let tx2 = with_tx(|_| Ok::<&str, ()>("ok"));
        let tx3 = with_tx(|_| Err::<bool, ()>(()));
        let tx4 = with_tx(|_| Ok::<f32, ()>(3.14));
        assert_eq!(tx1.join4(tx2, tx3, tx4).run(&mut ()), Err(()));

        let tx1 = with_tx(|_| Ok::<i32, ()>(42));
        let tx2 = with_tx(|_| Ok::<&str, ()>("ok"));
        let tx3 = with_tx(|_| Ok::<bool, ()>(true));
        let tx4 = with_tx(|_| Err::<i32, ()>(()));
        assert_eq!(tx1.join4(tx2, tx3, tx4).run(&mut ()), Err(()));
    }
}

fn _map_err<Ctx, Tx1, F, E>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx1::Item, E>
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
        _map_err(self.tx1, self.f)(ctx)
    }
}
#[cfg(test)]
mod test_map_err {
    use super::*;

    #[test]
    fn test_map_err() {
        let tx1 = with_tx(|_| Err::<i32, ()>(()));
        let f = |_: ()| "ng";
        assert_eq!(tx1.map_err(f).run(&mut ()), Err("ng"));

        let tx1 = with_tx(|_| Ok::<i32, ()>(42));
        let f = |_: ()| "ng";
        assert_eq!(tx1.map_err(f).run(&mut ()), Ok(42));
    }
}

fn _try_map<Ctx, Tx1, F, T>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<T, Tx1::Err>
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
        _try_map(self.tx1, self.f)(ctx)
    }
}
#[cfg(test)]
mod test_try_map {
    use super::*;

    #[test]
    fn test_try_map() {
        let tx1 = with_tx(|_| Ok::<i32, &str>(10));
        let f = |_| Err::<i32, &str>("too small");
        assert_eq!(tx1.try_map(f).run(&mut ()), Err("too small"));

        let tx1 = with_tx(|_| Ok::<i32, &str>(21));
        let f = |v| Ok::<i32, &str>(v * 2);
        assert_eq!(tx1.try_map(f).run(&mut ()), Ok(42));
    }
}

fn _recover<Ctx, Tx1, F>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx1::Item, Tx1::Err>
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
        _recover(self.tx1, self.f)(ctx)
    }
}
#[cfg(test)]
mod test_recover {
    use super::*;

    #[test]
    fn test_recover() {
        let tx1 = with_tx(|_| Err::<i32, &str>("error"));
        let f = |_: &str| 42;
        assert_eq!(tx1.recover(f).run(&mut ()), Ok(42));

        let tx1 = with_tx(|_| Ok::<i32, &str>(21));
        let f = |_: &str| 42;
        assert_eq!(tx1.recover(f).run(&mut ()), Ok(21));
    }
}

fn _try_recover<Ctx, Tx1, F, E>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx1::Item, E>
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
        _try_recover(self.tx1, self.f)(ctx)
    }
}
#[cfg(test)]
mod test_try_recover {
    use super::*;

    #[test]
    fn test_try_recover() {
        let tx1 = with_tx(|_| Err::<i32, &str>("error"));
        let f = |_: &str| Ok::<i32, &str>(42);
        assert_eq!(tx1.try_recover(f).run(&mut ()), Ok(42));

        let tx1 = with_tx(|_| Ok::<i32, &str>(21));
        let f = |_: &str| Ok::<i32, &str>(42);
        assert_eq!(tx1.try_recover(f).run(&mut ()), Ok(21));

        let tx1 = with_tx(|_| Ok::<i32, &str>(21));
        let f = |_: &str| Err::<i32, &str>("error again");
        assert_eq!(tx1.try_recover(f).run(&mut ()), Ok(21));

        let tx1 = with_tx(|_| Err::<i32, &str>("error"));
        let f = |_: &str| Err::<i32, &str>("error again");
        assert_eq!(tx1.try_recover(f).run(&mut ()), Err("error again"));
    }
}

fn _abort<Ctx, Tx1, F>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx1::Item, Tx1::Err>
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
        _abort(self.tx1, self.f)(ctx)
    }
}
#[cfg(test)]
mod test_abort {
    use super::*;

    #[test]
    fn test_abort() {
        let tx1 = with_tx(|_| Ok::<i32, &str>(42));
        let f = |_| "abort";
        assert_eq!(tx1.abort(f).run(&mut ()), Err("abort"));

        let tx1 = with_tx(|_| Err::<i32, &str>("error"));
        let f = |_| "abort";
        assert_eq!(tx1.abort(f).run(&mut ()), Err("error"));
    }
}

fn _try_abort<Ctx, Tx1, F>(tx1: Tx1, f: F) -> impl FnOnce(&mut Ctx) -> Result<Tx1::Item, Tx1::Err>
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
        _try_abort(self.tx1, self.f)(ctx)
    }
}
#[cfg(test)]
mod test_try_abort {
    use super::*;

    #[test]
    fn test_try_abort() {
        let tx1 = with_tx(|_| Ok::<i32, &str>(42));
        let f = |_| Err::<i32, &str>("try abort");
        assert_eq!(tx1.try_abort(f).run(&mut ()), Err("try abort"));

        let tx1 = with_tx(|_| Err::<i32, &str>("error"));
        let f = |_| Err::<i32, &str>("try abort");
        assert_eq!(tx1.try_abort(f).run(&mut ()), Err("error"));
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
