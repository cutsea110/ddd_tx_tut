use std::str;

use crate::dao::{DaoError, PersonDao};
use crate::domain::{Person, PersonId};

#[derive(Debug, Clone)]
pub struct PgPersonDao;
impl<'a> PersonDao<postgres::Transaction<'a>> for PgPersonDao {
    fn insert(
        &self,
        person: Person,
    ) -> impl tx_rs::Tx<postgres::Transaction<'a>, Item = PersonId, Err = DaoError> {
        tx_rs::with_tx(move |tx: &mut postgres::Transaction<'_>| {
            tx.query_one(
                "INSERT INTO person (name, age, data) VALUES ($1, $2, $3) RETURNING id",
                &[
                    &person.name,
                    &person.age,
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
    ) -> impl tx_rs::Tx<postgres::Transaction<'a>, Item = Option<Person>, Err = DaoError> {
        tx_rs::with_tx(move |tx: &mut postgres::Transaction<'_>| {
            tx.query_opt("SELECT name, age, data FROM person WHERE id = $1", &[&id])
                .map(|row| {
                    row.map(|row| {
                        let name = row.get::<usize, &str>(0);
                        let age = row.get::<usize, i32>(1);
                        let data = str::from_utf8(row.get::<usize, &[u8]>(2)).ok();

                        Person::new(name, age, data)
                    })
                })
                .map_err(|e| DaoError::SelectError(e.to_string()))
        })
    }
    fn select(
        &self,
    ) -> impl tx_rs::Tx<postgres::Transaction<'a>, Item = Vec<(PersonId, Person)>, Err = DaoError>
    {
        tx_rs::with_tx(|tx: &mut postgres::Transaction<'_>| {
            tx.query("SELECT id, name, age, data FROM person", &[])
                .map(|rows| {
                    rows.iter()
                        .map(|row| {
                            let id = row.get::<usize, PersonId>(0);
                            let name = row.get::<usize, &str>(1);
                            let age = row.get::<usize, i32>(2);
                            let data = str::from_utf8(row.get::<usize, &[u8]>(3)).ok();
                            let person = Person::new(name, age, data);

                            (id, person)
                        })
                        .collect()
                })
                .map_err(|e| DaoError::SelectError(e.to_string()))
        })
    }
}
