use log::trace;
use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
    rc::Rc,
};
use uuid::Uuid;

use crate::dao::{DaoError, PersonDao};
use crate::domain::{PersonId, Revision};
use crate::dto::PersonDto;

#[derive(Debug, Clone)]
pub struct HashDB {
    pub persons: Rc<RefCell<HashMap<PersonId, PersonDto>>>,
}
impl HashDB {
    pub fn new() -> Self {
        Self {
            persons: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}
impl<'a> PersonDao<RefMut<'a, HashMap<PersonId, PersonDto>>> for HashDB {
    fn insert(
        &self,
        person: PersonDto,
    ) -> impl tx_rs::Tx<RefMut<'a, HashMap<PersonId, PersonDto>>, Item = PersonId, Err = DaoError>
    {
        trace!("inserting person: {:?}", person);
        tx_rs::with_tx(move |ctx: &mut RefMut<'a, HashMap<PersonId, PersonDto>>| {
            let id = Uuid::now_v7();
            ctx.insert(id, person);
            Ok(id)
        })
    }

    fn fetch(
        &self,
        id: PersonId,
    ) -> impl tx_rs::Tx<RefMut<'a, HashMap<PersonId, PersonDto>>, Item = Option<PersonDto>, Err = DaoError>
    {
        trace!("fetching person: {:?}", id);
        tx_rs::with_tx(move |ctx: &mut RefMut<'a, HashMap<PersonId, PersonDto>>| {
            Ok(ctx.get(&id).cloned())
        })
    }

    fn select(
        &self,
    ) -> impl tx_rs::Tx<
        RefMut<'a, HashMap<PersonId, PersonDto>>,
        Item = Vec<(PersonId, PersonDto)>,
        Err = DaoError,
    > {
        trace!("selecting persons");
        tx_rs::with_tx(move |ctx: &mut RefMut<'a, HashMap<PersonId, PersonDto>>| {
            Ok(ctx
                .iter()
                .map(|(id, person)| (*id, person.clone()))
                .collect())
        })
    }

    fn save(
        &self,
        id: PersonId,
        revision: Revision,
        person: PersonDto,
    ) -> impl tx_rs::Tx<RefMut<'a, HashMap<PersonId, PersonDto>>, Item = (), Err = DaoError> {
        trace!("saving person: {:?}", person);
        tx_rs::with_tx(move |ctx: &mut RefMut<'a, HashMap<PersonId, PersonDto>>| {
            if let Some(existing) = ctx.get_mut(&id) {
                // `revision` is the revision the caller read (optimistic lock),
                // so the stored one must be exactly the same.
                if existing.revision != revision {
                    return Err(DaoError::UpdateError(format!(
                        "revision mismatch: expected {}, found {}",
                        revision, existing.revision
                    )));
                }
                ctx.insert(id, person);
                return Ok(());
            } else {
                return Err(DaoError::UpdateError(format!(
                    "person with id {} not found",
                    id
                )));
            }
        })
    }

    fn delete(
        &self,
        id: PersonId,
    ) -> impl tx_rs::Tx<RefMut<'a, HashMap<PersonId, PersonDto>>, Item = (), Err = DaoError> {
        trace!("deleting person: {:?}", id);
        tx_rs::with_tx(move |ctx: &mut RefMut<'a, HashMap<PersonId, PersonDto>>| {
            ctx.remove(&id);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use tx_rs::Tx;

    use super::*;
    use crate::domain::date;

    fn person(revision: Revision) -> PersonDto {
        PersonDto::new("poor man", date(2001, 9, 11), None, None, revision)
    }

    #[test]
    fn test_save_with_current_revision() {
        let db = HashDB::new();
        let mut ctx = db.persons.borrow_mut();
        let id = db.insert(person(0)).run(&mut ctx).unwrap();

        let result = db.save(id, 0, person(1)).run(&mut ctx);
        assert!(result.is_ok());
        assert_eq!(ctx.get(&id).unwrap().revision, 1);
    }

    #[test]
    fn test_save_with_stale_revision() {
        let db = HashDB::new();
        let mut ctx = db.persons.borrow_mut();
        let id = db.insert(person(0)).run(&mut ctx).unwrap();
        db.save(id, 0, person(1)).run(&mut ctx).unwrap();

        let result = db.save(id, 0, person(1)).run(&mut ctx);
        assert!(matches!(result, Err(DaoError::UpdateError(_))));
        assert_eq!(ctx.get(&id).unwrap().revision, 1);
    }

    #[test]
    fn test_save_not_found() {
        let db = HashDB::new();
        let mut ctx = db.persons.borrow_mut();

        let result = db.save(Uuid::now_v7(), 0, person(1)).run(&mut ctx);
        assert!(matches!(result, Err(DaoError::UpdateError(_))));
    }
}
