use chrono::NaiveDate;
use core::fmt;
use serde::{Deserialize, Serialize};

pub fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("create date")
}

pub type PersonId = i32;
/// actually, this is a layout. This means that the data is represented in a certain way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
impl fmt::Display for PersonLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Person {{ name: {}, birth_date: {}, death_date: {:?} data: {:?} }}",
            self.name, self.birth_date, self.death_date, self.data,
        )
    }
}
