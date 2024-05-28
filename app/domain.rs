use chrono::NaiveDate;
use core::fmt;
use serde::{Deserialize, Serialize};

pub fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("create date")
}

pub type PersonId = i32;
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

    pub fn notify(&self, dto: &mut impl PersonNotification) {
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
pub trait PersonNotification {
    fn set_name(&mut self, name: &str);
    fn set_birth_date(&mut self, birth_date: NaiveDate);
    fn set_death_date(&mut self, death_date: Option<NaiveDate>);
    fn set_data(&mut self, data: Option<&str>);
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonLayout {
    pub name: String,
    pub birth_date: NaiveDate,
    pub death_date: Option<NaiveDate>,
    pub data: Option<String>,
}
impl PersonLayout {
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
}

impl PersonNotification for PersonLayout {
    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }
    fn set_birth_date(&mut self, birth_date: NaiveDate) {
        self.birth_date = birth_date;
    }
    fn set_death_date(&mut self, death_date: Option<NaiveDate>) {
        self.death_date = death_date;
    }
    fn set_data(&mut self, data: Option<&str>) {
        self.data = data.map(|d| d.to_string());
    }
}

impl From<Person> for PersonLayout {
    fn from(person: Person) -> Self {
        let mut layout = PersonLayout::default();
        person.notify(&mut layout);
        layout
    }
}
impl From<PersonLayout> for Person {
    fn from(person: PersonLayout) -> Self {
        Self::new(
            &person.name,
            person.birth_date,
            person.death_date,
            person.data.as_deref(),
        )
    }
}
