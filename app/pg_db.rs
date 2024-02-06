use postgres::{Client, Transaction};
use std::fmt;
use thiserror::Error;

use crate::domain::{self, Person, PersonId};
use tx_rs::tx;

#[derive(Debug, Error)]
pub enum PgDbError {
    #[error("query failed: {0:?}")]
    QueryFailed(#[from] postgres::Error),
    #[error("failed to connect to database")]
    ConnectionFailed,
    #[error("failed to start transaction")]
    TransactionFailed,
    #[error("failed to commit")]
    CommitFailed,
    #[error("failed to rollback")]
    RollbackFailed,
}

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
            .map_err(|_| PgDbError::ConnectionFailed)
            .expect("connect to database");

        Self { conn_str, client }
    }
}
impl<'a> domain::PersonRepository<'a> for PgPersonRepository<'a> {
    type Ctx = Transaction<'a>;
    type Err = PgDbError;

    fn run_tx<Tx, T>(&'a mut self, tx: Tx) -> Result<T, Self::Err>
    where
        Tx: tx::Tx<Self::Ctx, Item = T, Err = Self::Err>,
    {
        let mut ctx = self
            .client
            .transaction()
            .map_err(|_| PgDbError::TransactionFailed)?;

        let result = tx.run(&mut ctx);

        if result.is_ok() {
            ctx.commit().map_err(|_| PgDbError::CommitFailed)?;
        } else {
            ctx.rollback().map_err(|_| PgDbError::RollbackFailed)?;
        }

        result
    }

    fn insert_person(person: &Person) -> impl tx::Tx<Self::Ctx, Item = PersonId, Err = Self::Err> {
        tx::with_tx(move |tx: &mut Self::Ctx| {
            let row = tx
                .query_one(
                    "INSERT INTO person (name, age, data) VALUES ($1, $2, $3) RETURNING id",
                    &[&person.name, &person.age, &person.data],
                )
                .map_err(|e| PgDbError::QueryFailed(e))?;

            Ok(row.get(0))
        })
    }

    fn fetch_person(
        id: PersonId,
    ) -> impl tx::Tx<Self::Ctx, Item = Option<Person>, Err = Self::Err> {
        tx::with_tx(move |tx: &mut Self::Ctx| {
            tx.query_opt("SELECT name, age, data FROM person WHERE id = $1", &[&id])
                .map(|row| row.map(|row| Person::new(row.get(0), row.get(1), row.get(2))))
                .map_err(|e| PgDbError::QueryFailed(e))
        })
    }

    fn collect_persons() -> impl tx::Tx<Self::Ctx, Item = Vec<(PersonId, Person)>, Err = Self::Err>
    {
        tx::with_tx(move |tx: &mut Self::Ctx| {
            let rows = tx
                .query("SELECT id, name, age, data FROM person", &[])
                .map_err(|e| PgDbError::QueryFailed(e))?
                .iter()
                .map(|row| (row.get(0), Person::new(row.get(1), row.get(2), row.get(3))))
                .collect();

            Ok(rows)
        })
    }

    fn update_person(
        id: PersonId,
        person: &Person,
    ) -> impl tx::Tx<Self::Ctx, Item = (), Err = Self::Err> {
        tx::with_tx(move |tx: &mut Self::Ctx| {
            tx.execute(
                "UPDATE person SET name = $1, age = $2, data = $3 WHERE id = $4",
                &[&person.name, &person.age, &person.data, &id],
            )
            .map_err(|e| PgDbError::QueryFailed(e))?;

            Ok(())
        })
    }

    fn delete_person(id: PersonId) -> impl tx::Tx<Self::Ctx, Item = (), Err = Self::Err> {
        tx::with_tx(move |tx: &mut Self::Ctx| {
            tx.execute("DELETE FROM person WHERE id = $1", &[&id])
                .map_err(|e| PgDbError::QueryFailed(e))?;

            Ok(())
        })
    }
}
