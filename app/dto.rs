use chrono::NaiveDate;
use log::trace;
use serde::{Deserialize, Serialize};

use crate::domain::{Person, PersonNotification, Revision};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonDto {
    pub name: String,
    pub birth_date: NaiveDate,
    pub death_date: Option<NaiveDate>,
    pub data: Option<String>,

    pub revision: Revision,
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
            revision: 0,
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
    fn set_revision(&mut self, revision: Revision) {
        trace!("set_revision: {}", revision);
        self.revision = revision;
    }
}

impl From<Person> for PersonDto {
    fn from(person: Person) -> Self {
        let mut dto = PersonDto::default();
        person.notify(&mut dto);
        dto
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::date;

    #[test]
    fn test_person_dto() {
        let person = Person::new(
            "name",
            date(2000, 1, 1),
            Some(date(2100, 12, 31)),
            Some("data"),
        );

        let mut dto = PersonDto::from(person.clone());
        assert_eq!(
            dto,
            PersonDto::new(
                "name",
                date(2000, 1, 1),
                Some(date(2100, 12, 31)),
                Some("data")
            )
        );

        // DTO を変更しても Person には影響しない
        dto.set_name("name2");
        dto.set_birth_date(date(2000, 1, 2));
        dto.set_death_date(None);
        dto.set_data(None);
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
    fn test_dto_person() {
        let mut dto = PersonDto::new(
            "name",
            date(2000, 1, 1),
            Some(date(2100, 12, 31)),
            Some("data"),
        );

        let person = Person::from(dto.clone());
        assert_eq!(
            person,
            Person::new(
                "name",
                date(2000, 1, 1),
                Some(date(2100, 12, 31)),
                Some("data")
            )
        );

        // DTO を変更しても person には影響しない
        dto.set_name("name2");
        dto.set_birth_date(date(2000, 1, 2));
        dto.set_death_date(None);
        dto.set_data(None);
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
