use chrono::NaiveDate;
use core::fmt;

pub type PersonId = i32;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub name: String,
    pub birth_date: NaiveDate,
    pub data: Option<String>,
}
impl Person {
    pub fn new(name: &str, birth_date: NaiveDate, data: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            birth_date,
            data: data.map(|d| d.to_string()),
        }
    }
}
impl fmt::Display for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Person {{ name: {}, birth_date: {}, data: {:?} }}",
            self.name, self.birth_date, self.data,
        )
    }
}
