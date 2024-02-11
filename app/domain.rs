use core::fmt;

pub type PersonId = i32;
#[derive(Debug, Clone)]
pub struct Person {
    pub name: String,
    pub age: i32,
    pub data: Option<String>,
}
impl Person {
    pub fn new(name: &str, age: i32, data: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            age,
            data: data.map(|d| d.to_string()),
        }
    }
}
impl fmt::Display for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Person {{ name: {}, age: {}, data: {:?} }}",
            self.name, self.age, self.data,
        )
    }
}
