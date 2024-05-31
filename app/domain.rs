use chrono::NaiveDate;
use core::fmt;
use log::{trace, warn};
use thiserror::Error;

pub fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("create date")
}

type FieldName = String;
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum PersonDomainError {
    #[error("invalid field value: {0}={1}")]
    InvalidFieldValue(FieldName, String),
    #[error("already dead")]
    AlreadyDead,
}

pub type PersonId = i32;
/// Person entity (as domain object)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    name: String,
    birth_date: NaiveDate,
    death_date: Option<NaiveDate>,
    data: Option<String>,
}

impl Person {
    pub fn new(
        name: &str,
        birth_date: NaiveDate,
        death_date: Option<NaiveDate>,
        data: Option<&str>,
    ) -> Self {
        Self {
            name: name.to_string(),
            birth_date,
            death_date,
            data: data.map(|d| d.to_string()),
        }
    }

    pub fn dead_at(&mut self, date: NaiveDate) -> Result<(), PersonDomainError> {
        if self.death_date.is_some() {
            warn!("person is already dead: {}", self);
            return Err(PersonDomainError::AlreadyDead);
        }
        if date < self.birth_date {
            warn!("death date must be before birth date: {}", self);
            return Err(PersonDomainError::InvalidFieldValue(
                "death_date".into(),
                "must be after birth date".into(),
            ));
        }

        self.death_date = Some(date);

        Ok(())
    }

    pub fn notify(&self, dto: &mut impl PersonNotification) {
        trace!("notifying to dto: {}", self);
        dto.set_name(&self.name);
        dto.set_birth_date(self.birth_date);
        dto.set_death_date(self.death_date);
        dto.set_data(self.data.as_deref());
    }
}

impl fmt::Display for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Person {{ name: {}, birth_date: {}, death_date: {:?} data: {:?} }}",
            self.name, self.birth_date, self.death_date, self.data,
        )
    }
}

/// For DTO like structs
pub trait PersonNotification {
    fn set_name(&mut self, name: &str);
    fn set_birth_date(&mut self, birth_date: NaiveDate);
    fn set_death_date(&mut self, death_date: Option<NaiveDate>);
    fn set_data(&mut self, data: Option<&str>);
}
