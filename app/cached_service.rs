use chrono::NaiveDate;
use log::trace;

pub use crate::cache::PersonCao;
pub use crate::domain::{Person, PersonId};
pub use crate::service::{PersonService, ServiceError};

pub trait PersonCachedService<'a, Conn, Ctx>: PersonService<'a, Ctx> {
    type C: PersonCao<Conn>;

    fn get_cao(&self) -> Self::C;

    fn cached_register(
        &'a mut self,
        name: &str,
        birth_date: NaiveDate,
        death_date: Option<NaiveDate>,
        data: &str,
    ) -> Result<(PersonId, Person), ServiceError> {
        let cao = self.get_cao();
        let mut con = cao.get_conn().expect("get cache connection");
        trace!("cache connection obtained");

        let result = self.register(name, birth_date, death_date, data);
        trace!("register person to db: {:?}", result);

        if let Ok((id, person)) = &result {
            let _: () = (cao.save(*id, person))(&mut con).expect("save cache");
            trace!("save person to cache: {}", person);
        }

        result
    }

    fn cached_find(&'a mut self, id: PersonId) -> Result<Option<Person>, ServiceError> {
        let cao = self.get_cao();
        let mut con = cao.get_conn().expect("get cache connection");
        trace!("cache connection obtained");

        // if the person is found in the cache, return it
        if let Some(p) = cao.find(id)(&mut con).expect("find cache") {
            trace!("cache hit!: {}", id);
            return Ok(Some(p));
        }
        trace!("cache miss!: {}", id);

        let result = self.find(id)?;
        trace!("find person in db: {:?}", result);

        // if the person is found in the db, save it to the cache
        if let Some(person) = &result {
            let _: () = (cao.save(id, person))(&mut con).expect("save cache");
            trace!("save person to cache: {}", person);
        }

        Ok(result)
    }

    fn cached_batch_import(
        &'a mut self,
        persons: Vec<Person>,
    ) -> Result<Vec<PersonId>, ServiceError> {
        let cao = self.get_cao();
        let mut con = cao.get_conn().expect("get cache connection");
        trace!("cache connection obtained");

        let ids = self.batch_import(persons.clone())?;

        // save all persons to the cache
        ids.iter().zip(persons.iter()).for_each(|(id, person)| {
            let _: () = (cao.save(*id, person))(&mut con).expect("save cache");
        });
        trace!("save persons to cache: {:?}", ids);

        Ok(ids)
    }

    fn cached_list_all(&'a mut self) -> Result<Vec<(PersonId, Person)>, ServiceError> {
        let cao = self.get_cao();
        let mut con = cao.get_conn().expect("get cache connection");
        trace!("cache connection obtained");

        let result = self.list_all()?;

        // save all persons to the cache
        result.iter().for_each(|(id, person)| {
            let _: () = (cao.save(*id, person))(&mut con).expect("save cache");
        });
        trace!("save all persons to cache");

        Ok(result)
    }

    fn cached_unregister(&'a mut self, id: PersonId) -> Result<(), ServiceError> {
        let cao = self.get_cao();
        let mut con = cao.get_conn().expect("get cache connection");
        trace!("cache connection obtained");

        // even if delete from db failed below, this cache clear is not a matter.
        let _: () = (cao.discard(id))(&mut con).expect("delete cache");
        trace!("cache cleared: {}", id);

        let result = self.unregister(id);
        trace!("delete from db: {}", id);

        result
    }
}
