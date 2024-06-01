use log::trace;
use redis::{self, Commands, FromRedisValue, ToRedisArgs};
use std::time::Duration;

use crate::cache::{CaoError, PersonCao};
use crate::domain::PersonId;
use crate::dto::PersonDto;

// this suppose PersonDto is serde-ized
impl ToRedisArgs for PersonDto {
    fn write_redis_args<W: ?Sized>(&self, out: &mut W)
    where
        W: redis::RedisWrite,
    {
        let s = serde_json::to_string(self).expect("serialize");
        out.write_arg(s.as_bytes());
    }
}
// this suppose PersonDto is serde-ized
impl FromRedisValue for PersonDto {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        let s: String = redis::from_redis_value(v)?;
        let p: PersonDto = serde_json::from_str(&s).expect("deserialize");
        Ok(p)
    }
}
#[derive(Debug, Clone)]
pub struct RedisPersonCao {
    client: redis::Client,
    connect_timeout: Duration,
}
impl RedisPersonCao {
    pub fn new(client: redis::Client, connect_timeout: Duration) -> Self {
        Self {
            client,
            connect_timeout,
        }
    }
}

impl PersonCao<redis::Connection> for RedisPersonCao {
    fn get_conn(&self) -> Result<redis::Connection, CaoError> {
        self.client
            .get_connection_with_timeout(self.connect_timeout)
            .map_err(|e| CaoError::Unavailable(e.to_string()))
    }

    fn run_tx<T, F>(&self, f: F) -> Result<T, CaoError>
    where
        F: tx_rs::Tx<redis::Connection, Item = T, Err = CaoError>,
    {
        let mut conn = self.get_conn()?;
        trace!("redis connection obtained");

        f.run(&mut conn)
    }

    fn find(
        &self,
        id: PersonId,
    ) -> impl tx_rs::Tx<redis::Connection, Item = Option<PersonDto>, Err = CaoError> {
        trace!("find person: {}", id);
        tx_rs::with_tx(move |conn: &mut redis::Connection| {
            let key = format!("person:{}", id);
            let p: Option<PersonDto> = conn
                .get(&key)
                .map_err(|e| CaoError::Unavailable(e.to_string()))?;
            trace!("found person in cache: {:?}", p);
            Ok(p.into())
        })
    }
    fn load(
        &self,
        id: PersonId,
        person: &PersonDto,
    ) -> impl tx_rs::Tx<redis::Connection, Item = (), Err = CaoError> {
        trace!("load person: {}", id);
        tx_rs::with_tx(move |conn: &mut redis::Connection| {
            let key = format!("person:{}", id);
            conn.set(&key, &person)
                .map_err(|e| CaoError::Unavailable(e.to_string()))?;
            trace!("person loaded into cache: {:?}", person);
            Ok(())
        })
    }
    fn unload(&self, id: PersonId) -> impl tx_rs::Tx<redis::Connection, Item = (), Err = CaoError> {
        trace!("unload person: {}", id);
        tx_rs::with_tx(move |conn: &mut redis::Connection| {
            let key = format!("person:{}", id);
            conn.del(&key)
                .map_err(|e| CaoError::Unavailable(e.to_string()))?;
            trace!("person unloaded from cache: {}", id);
            Ok(())
        })
    }
}
