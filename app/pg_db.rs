use chrono::NaiveDate;
use log::trace;
use std::str;

use crate::dao::{DaoError, PersonDao};
use crate::domain::{PersonId, Revision};
use crate::dto::PersonDto;

#[derive(Debug, Clone)]
pub struct PgPersonDao;
impl<'a> PersonDao<postgres::Transaction<'a>> for PgPersonDao {
    fn insert(
        &self,
        person: PersonDto,
    ) -> impl tx_rs::Tx<postgres::Transaction<'a>, Item = PersonId, Err = DaoError> {
        trace!("inserting person: {:?}", person);
        tx_rs::with_tx(move |tx: &mut postgres::Transaction<'_>| {
            tx.query_one(
                "INSERT INTO person (name, birth_date, death_date, data) VALUES ($1, $2, $3, $4) RETURNING id",
                &[
                    &person.name,
                    &person.birth_date,
		    &person.death_date,
                    &person.data.map(|d| d.as_str().as_bytes().to_vec()),
                ],
            )
            .map(|row| row.get::<usize, PersonId>(0))
            .map_err(|e| DaoError::InsertError(e.to_string()))
        })
    }
    fn fetch(
        &self,
        id: PersonId,
    ) -> impl tx_rs::Tx<postgres::Transaction<'a>, Item = Option<PersonDto>, Err = DaoError> {
        trace!("fetching person: {:?}", id);
        tx_rs::with_tx(move |tx: &mut postgres::Transaction<'_>| {
            tx.query_opt(
                "SELECT name, birth_date, death_date, data FROM person WHERE id = $1",
                &[&id],
            )
            .map(|row| {
                row.map(|row| {
                    let name = row.get::<usize, &str>(0);
                    let birth_date = row.get::<usize, NaiveDate>(1);
                    let death_date = row.get::<usize, Option<NaiveDate>>(2);
                    let data = str::from_utf8(row.get::<usize, &[u8]>(3)).ok();

                    PersonDto::new(name, birth_date, death_date, data)
                })
            })
            .map_err(|e| DaoError::SelectError(e.to_string()))
        })
    }
    fn select(
        &self,
    ) -> impl tx_rs::Tx<postgres::Transaction<'a>, Item = Vec<(PersonId, PersonDto)>, Err = DaoError>
    {
        trace!("selecting all persons");
        tx_rs::with_tx(|tx: &mut postgres::Transaction<'_>| {
            tx.query(
                "SELECT id, name, birth_date, death_date, data FROM person",
                &[],
            )
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        let id = row.get::<usize, PersonId>(0);
                        let name = row.get::<usize, &str>(1);
                        let birth_date = row.get::<usize, NaiveDate>(2);
                        let death_date = row.get::<usize, Option<NaiveDate>>(3);
                        let data = str::from_utf8(row.get::<usize, &[u8]>(4)).ok();
                        let person = PersonDto::new(name, birth_date, death_date, data);

                        (id, person)
                    })
                    .collect()
            })
            .map_err(|e| DaoError::SelectError(e.to_string()))
        })
    }
    fn save(
        &self,
        id: PersonId,
        revision: Revision,
        person: PersonDto,
    ) -> impl tx_rs::Tx<postgres::Transaction<'a>, Item = (), Err = DaoError> {
        trace!("saving person: {:?}", id);
        tx_rs::with_tx(move |tx: &mut postgres::Transaction<'_>| {
            tx.execute(
		"UPDATE person SET name = $1, birth_date = $2, death_date = $3, data = $4 WHERE id = $5 AND revision = $6",
		&[
		    &person.name,
		    &person.birth_date,
		    &person.death_date,
		    &person.data.map(|d| d.as_str().as_bytes().to_vec()),
		    &id,
		    &revision,
		],
	    )
            .map(|_| ())
            .map_err(|e| DaoError::UpdateError(e.to_string()))
        })
    }
    fn delete(
        &self,
        id: PersonId,
    ) -> impl tx_rs::Tx<postgres::Transaction<'a>, Item = (), Err = DaoError> {
        trace!("deleting person: {:?}", id);
        tx_rs::with_tx(move |tx: &mut postgres::Transaction<'_>| {
            tx.execute("DELETE FROM person WHERE id = $1", &[&id])
                .map(|_| ())
                .map_err(|e| DaoError::DeleteError(e.to_string()))
        })
    }
}
