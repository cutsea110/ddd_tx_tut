use chrono::NaiveDate;
use log::trace;
use serde::{Deserialize, Serialize};

use crate::domain::{Person, PersonNotification};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonDto {
    pub name: String,
    pub birth_date: NaiveDate,
    pub death_date: Option<NaiveDate>,
    pub data: Option<String>,
}
impl PersonDto {
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

impl PersonNotification for PersonDto {
    fn set_name(&mut self, name: &str) {
        trace!("set_name: {}", name);
        self.name = name.to_string();
    }
    fn set_birth_date(&mut self, birth_date: NaiveDate) {
        trace!("set_birth_date: {}", birth_date);
        self.birth_date = birth_date;
    }
    fn set_death_date(&mut self, death_date: Option<NaiveDate>) {
        trace!("set_death_date: {:?}", death_date);
        self.death_date = death_date;
    }
    fn set_data(&mut self, data: Option<&str>) {
        trace!("set_data: {:?}", data);
        self.data = data.map(|d| d.to_string());
    }
}

impl From<Person> for PersonDto {
    fn from(person: Person) -> Self {
        let mut layout = PersonDto::default();
        person.notify(&mut layout);
        layout
    }
}
impl From<PersonDto> for Person {
    fn from(person: PersonDto) -> Self {
        Self::new(
            &person.name,
            person.birth_date,
            person.death_date,
            person.data.as_deref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::date;

    #[test]
    fn test_person_layout() {
        let person = Person::new(
            "name",
            date(2000, 1, 1),
            Some(date(2100, 12, 31)),
            Some("data"),
        );

        let mut layout = PersonDto::from(person.clone());
        assert_eq!(
            layout,
            PersonDto::new(
                "name",
                date(2000, 1, 1),
                Some(date(2100, 12, 31)),
                Some("data")
            )
        );

        // Layout を変更しても Person には影響しない
        layout.set_name("name2");
        layout.set_birth_date(date(2000, 1, 2));
        layout.set_death_date(None);
        layout.set_data(None);
        assert_eq!(
            person,
            Person::new(
                "name",
                date(2000, 1, 1),
                Some(date(2100, 12, 31)),
                Some("data")
            )
        );
    }

    #[test]
    fn test_layout_person() {
        let mut layout = PersonDto::new(
            "name",
            date(2000, 1, 1),
            Some(date(2100, 12, 31)),
            Some("data"),
        );

        let person = Person::from(layout.clone());
        assert_eq!(
            person,
            Person::new(
                "name",
                date(2000, 1, 1),
                Some(date(2100, 12, 31)),
                Some("data")
            )
        );

        // layout を変更しても person には影響しない
        layout.set_name("name2");
        layout.set_birth_date(date(2000, 1, 2));
        layout.set_death_date(None);
        layout.set_data(None);
        assert_eq!(
            person,
            Person::new(
                "name",
                date(2000, 1, 1),
                Some(date(2100, 12, 31)),
                Some("data")
            )
        );
    }
}
