use postgres::{Client, Transaction};
use std::fmt;

use crate::domain::person::{self, Person, PersonId, PersonRepositoryError};
use tx_rs;

pub struct PgPersonRepository<'a> {
    conn_str: &'a str,
    client: Client,
}
impl fmt::Display for PgPersonRepository<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PgPersonRepository {{ conn_str: {} }}", self.conn_str)
    }
}
impl<'a> PgPersonRepository<'a> {
    pub fn new(conn_str: &'a str) -> Self {
        let client = Client::connect(conn_str, postgres::NoTls)
            .map_err(|_| PersonRepositoryError::Dummy)
            .expect("connect to database");

        Self { conn_str, client }
    }
}
impl<'a> person::PersonRepository<'a> for PgPersonRepository<'a> {
    type Ctx = Transaction<'a>;

    fn run_tx<Tx, T>(&'a mut self, tx: Tx) -> person::Result<T>
    where
        Tx: tx_rs::Tx<Self::Ctx, Item = T, Err = PersonRepositoryError>,
    {
        let mut ctx = self
            .client
            .transaction()
            .map_err(|_| PersonRepositoryError::Dummy)?;

        let result = tx.run(&mut ctx);

        if result.is_ok() {
            ctx.commit().map_err(|_| PersonRepositoryError::Dummy)?;
        } else {
            ctx.rollback().map_err(|_| PersonRepositoryError::Dummy)?;
        }

        result
    }

    fn create(
        person: &Person,
    ) -> impl tx_rs::Tx<Self::Ctx, Item = PersonId, Err = PersonRepositoryError> {
        tx_rs::with_tx(move |tx: &mut Self::Ctx| {
            let row = tx
                .query_one(
                    "INSERT INTO person (name, age, data) VALUES ($1, $2, $3) RETURNING id",
                    &[&person.name, &person.age, &person.data],
                )
                .map_err(|_| PersonRepositoryError::Dummy)?;

            Ok(row.get(0))
        })
    }

    fn fetch(
        id: PersonId,
    ) -> impl tx_rs::Tx<Self::Ctx, Item = Option<Person>, Err = PersonRepositoryError> {
        tx_rs::with_tx(move |tx: &mut Self::Ctx| {
            tx.query_opt("SELECT name, age, data FROM person WHERE id = $1", &[&id])
                .map(|row| row.map(|row| Person::new(row.get(0), row.get(1), row.get(2))))
                .map_err(|_| PersonRepositoryError::Dummy)
        })
    }

    fn collect(
    ) -> impl tx_rs::Tx<Self::Ctx, Item = Vec<(PersonId, Person)>, Err = PersonRepositoryError>
    {
        tx_rs::with_tx(move |tx: &mut Self::Ctx| {
            let rows = tx
                .query("SELECT id, name, age, data FROM person", &[])
                .map_err(|_| PersonRepositoryError::Dummy)?
                .iter()
                .map(|row| (row.get(0), Person::new(row.get(1), row.get(2), row.get(3))))
                .collect();

            Ok(rows)
        })
    }

    fn update(
        id: PersonId,
        person: &Person,
    ) -> impl tx_rs::Tx<Self::Ctx, Item = (), Err = PersonRepositoryError> {
        tx_rs::with_tx(move |tx: &mut Self::Ctx| {
            tx.execute(
                "UPDATE person SET name = $1, age = $2, data = $3 WHERE id = $4",
                &[&person.name, &person.age, &person.data, &id],
            )
            .map_err(|_| PersonRepositoryError::Dummy)?;

            Ok(())
        })
    }

    fn delete(id: PersonId) -> impl tx_rs::Tx<Self::Ctx, Item = (), Err = PersonRepositoryError> {
        tx_rs::with_tx(move |tx: &mut Self::Ctx| {
            tx.execute("DELETE FROM person WHERE id = $1", &[&id])
                .map_err(|_| PersonRepositoryError::Dummy)?;

            Ok(())
        })
    }
}
