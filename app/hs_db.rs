use log::trace;
use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
};
use uuid::Uuid;

use crate::dao::{DaoError, PersonDao};
use crate::domain::{PersonId, Revision};
use crate::dto::PersonDto;

#[derive(Debug, Clone)]
pub struct HashDB {
    persons: RefCell<HashMap<PersonId, PersonDto>>,
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
                if existing.revision >= revision {
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
