use std::env;
use std::rc::Rc;

mod cache;
mod cached_service;
mod dao;
mod domain;
mod dto;
#[macro_use]
mod location;
mod dynamodb;
mod pg_db;
mod rabbitmq;
mod redis_cache;
mod reporter;
mod service;
mod service_impl;
mod syslog;
mod usecase;

use crate::dto::PersonDto;
use cached_service::PersonCachedService;
use dao::PersonDao;
use domain::date;
use service_impl::{PersonBatchImportPresenterImpl, PersonServiceImpl};
use tx_rs::Tx;

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe {
            std::env::set_var("RUST_LOG", "info");
        };
    }
    env_logger::init();

    // multi-thread runtime
    let runtime: Rc<tokio::runtime::Runtime> = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .into();

    // now on testing!!
    let dynamo = dynamodb::DynamoDbPersonDao::new(runtime.clone(), "http://localhost:18000");
    let _ = dynamo
        .insert(PersonDto::new(
            "Bob",
            date(1970, 11, 6),
            None,
            Some("Rust,Haskell,Scheme"),
            1,
        ))
        .run(&mut runtime.clone()); // TODO: tx_run provide ctx got from dynamo

    let new_id = dynamo
        .insert(PersonDto::new("Alice", date(2012, 11, 2), None, None, 1))
        .run(&mut runtime.clone()); // TODO: tx_run provide ctx got from dynamo

    let id = match new_id.clone() {
        Ok(new_id) => {
            println!("Put OK! {}", new_id);
            new_id
        }
        Err(e) => {
            eprintln!("Put ERR! {:?}", e);
            panic!("couldn't continue!");
        }
    };
    match dynamo.fetch(id.clone()).run(&mut runtime.clone()) {
        Ok(p) => {
            println!("Fetch OK! {:#?}", p);
        }
        Err(e) => {
            eprintln!("Fetch ERR! {:?}", e);
        }
    }
    match dynamo.select().run(&mut runtime.clone()) {
        Ok(ps) => {
            println!("Select OK! {:#?}", ps);
        }
        Err(e) => {
            eprintln!("Select ERR! {:?}", e);
        }
    }
    match dynamo.delete(id.clone()).run(&mut runtime.clone()) {
        Ok(p) => {
            println!("Delete OK! {:#?}", p);
        }
        Err(e) => {
            eprintln!("Delete ERR! {:?}", e);
        }
    }

    // Initialize service
    let mut service = {
        let cache_uri =
            env::var("CACHE_URI").unwrap_or("redis://:adminpass@localhost:16379".to_string());
        let db_uri = env::var("DATABASE_URI").unwrap_or(
            // connect_timeout is in seconds
            "postgres://admin:adminpass@localhost:15432/sampledb?connect_timeout=2".to_string(),
        );
        let mq_uri = env::var("AMQP_URI").unwrap_or(
            // connection_timeout is in milliseconds
            "amqp://admin:adminpass@localhost:5672/%2f?connection_timeout=2000".to_string(),
        );

        PersonServiceImpl::new(runtime.clone(), &db_uri, &cache_uri, &mq_uri)
    };

    // register, find and death, then unregister
    {
        let (id, person) = service
            .register("poor man", date(2001, 9, 11), None, "one person")
            .expect("register one person");
        println!("id:{} {:?}", id, person);

        if let Some(p) = service.find(id).expect("find person") {
            println!("cache hit:{:?}", p);

            let death_date = date(2011, 3, 11);
            println!("death {} at:{:?}", id, death_date);
            service.death(id, death_date).expect("kill person");

            if let Some(p) = service.find(id).expect("find dead person") {
                println!("dead person: {:?}", p);
            }
        }
        service.unregister(id).expect("delete person");
    }

    // batch import
    let ids = {
        let persons = vec![
            (
                "Abel",
                date(1802, 8, 5)..=date(1829, 4, 6),
                "Abel's theorem",
            ),
            (
                "Euler",
                date(1707, 4, 15)..=date(1783, 9, 18),
                "Euler's identity",
            ),
            (
                "Galois",
                date(1811, 10, 25)..=date(1832, 5, 31),
                "Group Theory",
            ),
            (
                "Gauss",
                date(1777, 4, 30)..=date(1855, 2, 23),
                "King of Math",
            ),
        ]
        .into_iter()
        .map(|(name, life, desc)| {
            PersonDto::new(name, *life.start(), Some(*life.end()), Some(desc), 0)
        })
        .collect::<Vec<_>>();

        let ids = service
            .batch_import(persons.clone(), Rc::new(PersonBatchImportPresenterImpl))
            .expect("batch import");
        println!("batch import done");

        ids
    };

    // list all
    {
        let persons = service.list_all().expect("list all");
        for (id, _) in &persons {
            if let Some(p) = service.find(*id).expect("find person") {
                println!("cache hit:{} {:?}", id, p);
            }
        }
    }

    // unregister
    {
        for id in ids {
            println!("unregister id:{}", id);
            service.unregister(id).expect("unregister");
        }
    }

    println!("done everything!");
}
