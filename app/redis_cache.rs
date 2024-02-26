use redis::{self, Commands, FromRedisValue, ToRedisArgs};

use crate::{
    cache::{CaoError, PersonCao},
    Person, PersonId,
};

// this suppose Person is serde-ized
impl ToRedisArgs for Person {
    fn write_redis_args<W: ?Sized>(&self, out: &mut W)
    where
        W: redis::RedisWrite,
    {
        let s = serde_json::to_string(self).expect("serialize");
        out.write_arg(s.as_bytes());
    }
}
// this suppose Person is serde-ized
impl FromRedisValue for Person {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        let s: String = redis::from_redis_value(v)?;
        let p: Person = serde_json::from_str(&s).expect("deserialize");
        Ok(p)
    }
}
#[derive(Debug, Clone)]
pub struct RedisPersonCao {
    client: redis::Client,
}
impl RedisPersonCao {
    pub fn new(client: redis::Client) -> Self {
        Self { client }
    }
}

impl PersonCao<redis::Connection> for RedisPersonCao {
    fn get_conn(&self) -> Result<redis::Connection, CaoError> {
        self.client
            .get_connection()
            .map_err(|e| CaoError::Unavailable(e.to_string()))
    }

    fn exists(
        &self,
        id: PersonId,
    ) -> impl FnOnce(&mut redis::Connection) -> Result<bool, CaoError> {
        move |conn| {
            let key = format!("person:{}", id);
            let exists: bool = conn
                .exists(&key)
                .map_err(|e| CaoError::Unavailable(e.to_string()))?;

            Ok(exists)
        }
    }
    fn find(
        &self,
        id: PersonId,
    ) -> impl FnOnce(&mut redis::Connection) -> Result<Option<Person>, CaoError> {
        move |conn| {
            let key = format!("person:{}", id);
            let p: Option<Person> = conn
                .get(&key)
                .map_err(|e| CaoError::Unavailable(e.to_string()))?;

            Ok(p)
        }
    }
    fn save(
        &self,
        id: PersonId,
        person: &Person,
    ) -> impl FnOnce(&mut redis::Connection) -> Result<(), CaoError> {
        move |conn| {
            let key = format!("person:{}", id);
            conn.set(&key, person)
                .map_err(|e| CaoError::Unavailable(e.to_string()))?;

            Ok(())
        }
    }
    fn discard(&self, id: PersonId) -> impl FnOnce(&mut redis::Connection) -> Result<(), CaoError> {
        move |conn| {
            let key = format!("person:{}", id);
            conn.del(&key)
                .map_err(|e| CaoError::Unavailable(e.to_string()))?;

            Ok(())
        }
    }
}
