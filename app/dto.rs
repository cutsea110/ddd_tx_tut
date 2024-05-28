use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::domain::{Person, PersonNotification};

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
