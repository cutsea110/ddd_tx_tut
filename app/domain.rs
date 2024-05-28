use chrono::NaiveDate;
use core::fmt;

pub fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("create date")
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

/// For DTO like structs
pub trait PersonNotification {
    fn set_name(&mut self, name: &str);
    fn set_birth_date(&mut self, birth_date: NaiveDate);
    fn set_death_date(&mut self, death_date: Option<NaiveDate>);
    fn set_data(&mut self, data: Option<&str>);
}
