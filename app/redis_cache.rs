use redis::{self, Commands, FromRedisValue, ToRedisArgs};

use crate::{
    cache::{CaoError, PersonCao},
    Person,
};

// this suppose the Person is serde-able.
impl ToRedisArgs for Person {
    fn write_redis_args<W: ?Sized>(&self, out: &mut W)
    where
        W: redis::RedisWrite,
    {
        let s = serde_json::to_string(self).expect("serialize person");
        out.write_arg(s.as_bytes());
    }
}
// this suppose the Person is serde-able.
impl FromRedisValue for Person {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        let s: String = redis::from_redis_value(v)?;
        let p: Person = serde_json::from_str(&s).expect("deserialize person");
        Ok(p)
    }
}

#[derive(Debug, Clone)]
pub struct RedisPersonCao;
impl PersonCao<redis::Connection> for RedisPersonCao {
    fn exists(
        &self,
        key: &crate::PersonId,
    ) -> impl tx_rs::Tx<redis::Connection, Item = bool, Err = CaoError> {
        tx_rs::with_tx(move |con: &mut redis::Connection| {
            con.exists(key)
                .map_err(|e| CaoError::Unavailable(e.to_string()))
        })
    }
    fn find(
        &self,
        key: &crate::PersonId,
    ) -> impl tx_rs::Tx<redis::Connection, Item = Option<crate::Person>, Err = CaoError> {
        tx_rs::with_tx(move |con: &mut redis::Connection| {
            con.get(key)
                .map_err(|e| CaoError::Unavailable(e.to_string()))
        })
    }
    fn save(
        &self,
        key: &crate::PersonId,
        value: &crate::Person,
    ) -> impl tx_rs::Tx<redis::Connection, Item = (), Err = CaoError> {
        tx_rs::with_tx(move |con: &mut redis::Connection| {
            con.set(key, value)
                .map_err(|e| CaoError::Unavailable(e.to_string()))
        })
    }
    fn discard(
        &self,
        key: &crate::PersonId,
    ) -> impl tx_rs::Tx<redis::Connection, Item = (), Err = crate::cache::CaoError> {
        tx_rs::with_tx(move |con: &mut redis::Connection| {
            con.del(key)
                .map_err(|e| CaoError::Unavailable(e.to_string()))
        })
    }
}
