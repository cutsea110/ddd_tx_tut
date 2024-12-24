use aws_sdk_dynamodb::types::{AttributeValue, AttributeValueUpdate, ReturnValue};
use chrono::NaiveDate;
use log::{debug, trace};
use std::{collections::HashMap, rc::Rc};

use crate::dao::{DaoError, PersonDao};
use crate::domain::{PersonId, Revision};
use crate::dto::PersonDto;

#[derive(Debug, Clone)]
pub struct DynamoDbPersonDao {
    async_runtime: Rc<tokio::runtime::Runtime>,
    client: aws_sdk_dynamodb::Client,
}
impl DynamoDbPersonDao {
    pub fn new(runtime: Rc<tokio::runtime::Runtime>, endpoint_url: &str) -> Self {
        let config = runtime.block_on(async {
            aws_config::from_env()
                .endpoint_url(endpoint_url)
                .load()
                .await
        });
        trace!("SdkConfig: {:?}", config);
        let client = aws_sdk_dynamodb::Client::new(&config);
        Self {
            async_runtime: runtime,
            client,
        }
    }
}

fn convert(hm: HashMap<String, AttributeValue>) -> Result<(PersonId, PersonDto), DaoError> {
    debug!("found person: {:?}", hm);
    let id = hm
        .get("id")
        .ok_or(DaoError::SelectError("not found id attr in person".into()))?
        .as_n()
        .map_err(|e| DaoError::SelectError(format!("invalid N value: {:?}", e)))
        .and_then(|d| {
            d.parse::<i32>()
                .map_err(|e| DaoError::SelectError(format!("failed to parse as i32: {:?}", e)))
        })?;
    let name = hm
        .get("name")
        .ok_or(DaoError::SelectError(
            "not found name attr in person".into(),
        ))?
        .as_s()
        .map_err(|e| DaoError::SelectError(format!("invalid S value: {:?}", e)))?;
    let birth_date = hm
        .get("birth_date")
        .ok_or(DaoError::SelectError(
            "not found birth_date attr in person".into(),
        ))?
        .as_s()
        .map_err(|e| DaoError::SelectError(format!("invalid S value: {:?}", e)))
        .and_then(|d| {
            d.parse::<NaiveDate>().map_err(|e| {
                DaoError::SelectError(format!("failed to parse as NaiveDate: {:?}", e))
            })
        })?;
    let death_date = hm
        .get("birth_date")
        .map(|d| d.as_s().unwrap().parse::<NaiveDate>().unwrap());
    let data = hm.get("data").map(|d| d.as_s().unwrap().as_str());
    let revision = hm
        .get("revision")
        .ok_or(DaoError::SelectError(
            "not found revision attr in person".into(),
        ))?
        .as_n()
        .map(|r| r.parse::<i32>().unwrap())
        .map_err(|e| DaoError::SelectError(format!("invalid N value: {:?}", e)))?;

    Ok((
        id,
        PersonDto::new(name, birth_date, death_date, data, revision),
    ))
}

impl PersonDao<Rc<tokio::runtime::Runtime>> for DynamoDbPersonDao {
    fn insert(
        &self,
        person: PersonDto,
    ) -> impl tx_rs::Tx<Rc<tokio::runtime::Runtime>, Item = PersonId, Err = DaoError> {
        trace!("inserting person: {:?}", person);
        tx_rs::with_tx(move |tx: &mut Rc<tokio::runtime::Runtime>| {
            tx.block_on(async {
                let req = self
                    .client
                    .update_item()
                    .table_name("counter")
                    .key("table_name", AttributeValue::S("person".into()))
                    .update_expression("SET id = if_not_exists(id, :start) + :incr")
                    .expression_attribute_values(":start", AttributeValue::N(0.to_string()))
                    .expression_attribute_values(":incr", AttributeValue::N(1.to_string()))
                    .return_values(ReturnValue::AllNew);
                trace!("request to update-item counter: {:?}", req);

                let resp = req
                    .send()
                    .await
                    .map_err(|e| DaoError::InsertError(e.to_string()))?;
                debug!("response of update-item counter: {:?}", resp);

                let new_id = resp
                    .attributes
                    .ok_or(DaoError::InsertError("not found person counter".into()))?
                    .get("id")
                    .ok_or(DaoError::InsertError(
                        "not found id attr in person counter".into(),
                    ))?
                    .clone();
                debug!("new id: {:?}", new_id);

                let mut item = HashMap::from([
                    ("id".into(), new_id.clone()),
                    ("name".into(), AttributeValue::S(person.name)),
                    (
                        "birth_date".into(),
                        AttributeValue::S(person.birth_date.to_string()),
                    ),
                    (
                        "revision".into(),
                        AttributeValue::N(person.revision.to_string()),
                    ),
                ]);
                if let Some(death_date) = person.death_date {
                    item.insert(
                        "death_date".into(),
                        AttributeValue::S(death_date.to_string()),
                    );
                }
                if let Some(data) = person.data {
                    item.insert("death_date".into(), AttributeValue::S(data));
                }
                debug!("new person: {:?}", item);

                let req = self
                    .client
                    .put_item()
                    .table_name("person")
                    .set_item(Some(item))
                    .return_values(ReturnValue::None);
                trace!("request to put-item person: {:?}", req);

                let resp = req
                    .send()
                    .await
                    .map_err(|e| DaoError::InsertError(e.to_string()))?;
                debug!("response of put-item person: {:?}", resp);

                let id = new_id
                    .as_n()
                    .map_err(|e| DaoError::InsertError(format!("invalid N value: {:?}", e)))?
                    .parse::<i32>()
                    .map_err(|e| {
                        DaoError::InsertError(format!("failed to parse as i32: {:?}", e))
                    })?;

                Ok(id)
            })
        })
    }
    fn fetch(
        &self,
        id: PersonId,
    ) -> impl tx_rs::Tx<Rc<tokio::runtime::Runtime>, Item = Option<PersonDto>, Err = DaoError> {
        trace!("fetching person: {:?}", id);
        tx_rs::with_tx(move |tx: &mut Rc<tokio::runtime::Runtime>| {
            tx.block_on(async {
                let req = self
                    .client
                    .get_item()
                    .table_name("person")
                    .key("id", AttributeValue::N(id.to_string()));
                trace!("request to get-item person: {:?}", req);

                let resp = req
                    .send()
                    .await
                    .map_err(|e| DaoError::SelectError(e.to_string()))?;
                debug!("response of get-item person: {:?}", resp);

                match resp.item {
                    None => Ok(None),
                    Some(hm) => Ok(convert(hm).map(|(_, p)| Some(p))?),
                }
            })
        })
    }
    fn select(
        &self,
    ) -> impl tx_rs::Tx<Rc<tokio::runtime::Runtime>, Item = Vec<(PersonId, PersonDto)>, Err = DaoError>
    {
        trace!("selecting all persons");
        tx_rs::with_tx(move |tx: &mut Rc<tokio::runtime::Runtime>| {
            tx.block_on(async {
                let req = self
                    .client
                    .scan()
                    .table_name("person")
                    .limit(100)
                    .into_paginator()
                    .items();

                let resp: Vec<_> = req
                    .send()
                    .collect::<Result<Vec<_>, _>>()
                    .await
                    .map_err(|e| DaoError::SelectError(e.to_string()))?;
                debug!("response of query person: {:?}", resp);

                let mut ret_val = vec![];
                for p in resp {
                    ret_val.push(convert(p)?);
                }

                Ok(ret_val)
            })
        })
    }
    fn save(
        &self,
        id: PersonId,
        revision: Revision,
        person: PersonDto,
    ) -> impl tx_rs::Tx<Rc<tokio::runtime::Runtime>, Item = (), Err = DaoError> {
        trace!("saving person: {:?}", id);
        tx_rs::with_tx(move |tx: &mut Rc<tokio::runtime::Runtime>| {
            tx.block_on(async {
                let attr_name_upd = AttributeValueUpdate::builder()
                    .action(aws_sdk_dynamodb::types::AttributeAction::Put)
                    .value(AttributeValue::S(person.name))
                    .build();
                let attr_birth_upd = AttributeValueUpdate::builder()
                    .action(aws_sdk_dynamodb::types::AttributeAction::Put)
                    .value(AttributeValue::S(person.birth_date.to_string()))
                    .build();
                let attr_death_upd = match person.death_date {
                    Some(d) => AttributeValueUpdate::builder()
                        .action(aws_sdk_dynamodb::types::AttributeAction::Put)
                        .value(AttributeValue::S(d.to_string()))
                        .build(),
                    None => AttributeValueUpdate::builder()
                        .action(aws_sdk_dynamodb::types::AttributeAction::Delete)
                        .build(),
                };
                let attr_data_upd = match person.data {
                    Some(d) => AttributeValueUpdate::builder()
                        .action(aws_sdk_dynamodb::types::AttributeAction::Put)
                        .value(AttributeValue::S(d.to_string()))
                        .build(),
                    None => AttributeValueUpdate::builder()
                        .action(aws_sdk_dynamodb::types::AttributeAction::Delete)
                        .build(),
                };
                let attr_revision_upd = AttributeValueUpdate::builder()
                    .action(aws_sdk_dynamodb::types::AttributeAction::Put)
                    .value(AttributeValue::N(revision.to_string()))
                    .build();

                let req = self
                    .client
                    .update_item()
                    .table_name("person")
                    .key("id", AttributeValue::N(id.to_string()))
                    .attribute_updates("name", attr_name_upd)
                    .attribute_updates("birth_date", attr_birth_upd)
                    .attribute_updates("death_date", attr_death_upd)
                    .attribute_updates("data", attr_data_upd)
                    .attribute_updates("revision", attr_revision_upd)
                    .return_values(ReturnValue::None);
                debug!("request for update-item person: {:?}", req);

                let resp = req
                    .send()
                    .await
                    .map_err(|e| DaoError::UpdateError(e.to_string()))?;
                debug!("response of update-item person: {:?}", resp);

                Ok(())
            })
        })
    }
    fn delete(
        &self,
        id: PersonId,
    ) -> impl tx_rs::Tx<Rc<tokio::runtime::Runtime>, Item = (), Err = DaoError> {
        trace!("deleting person: {:?}", id);
        tx_rs::with_tx(move |tx: &mut Rc<tokio::runtime::Runtime>| {
            tx.block_on(async {
                let req = self
                    .client
                    .delete_item()
                    .table_name("person")
                    .key("id", AttributeValue::N(id.to_string()));
                trace!("request to delete-item person: {:?}", req);

                let resp = req
                    .send()
                    .await
                    .map_err(|e| DaoError::DeleteError(e.to_string()))?;
                debug!("response of delete-item person: {:?}", resp);

                Ok(())
            })
        })
    }
}