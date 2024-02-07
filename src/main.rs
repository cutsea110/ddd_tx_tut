use postgres::{Client, NoTls};

struct Usecase {
    client: Client,
}
impl Usecase {
    pub fn new(url: &str) -> Self {
        let client = Client::connect(url, NoTls).unwrap();
        Self { client }
    }

    pub fn entry(&mut self, name: &str, age: i32, data: Option<&[u8]>) -> i32 {
        let mut transaction = self.client.transaction().unwrap();

        let row = transaction
            .query_one(
                "INSERT INTO person (name, age, data) VALUES ($1, $2, $3) RETURNING id",
                &[&name, &age, &data],
            )
            .unwrap();

        transaction.commit().unwrap();

        return row.get(0);
    }

    pub fn collect(&mut self) -> Vec<(i32, String, i32, Option<String>)> {
        let mut result = vec![];

        let mut transaction = self.client.transaction().unwrap();

        for row in transaction
            .query("SELECT id, name, age, data FROM person", &[])
            .unwrap()
        {
            let id: i32 = row.get(0);
            let name: &str = row.get(1);
            let age: i32 = row.get(2);
            let data: Option<&[u8]> = row.get(3);

            result.push((
                id,
                name.to_string(),
                age,
                data.map(|d| String::from_utf8_lossy(d).to_string()),
            ));
        }

        transaction.rollback().unwrap();

        result
    }
}

fn main() {
    let mut usecase = Usecase::new("postgresql://admin:adminpass@localhost:15432/sampledb");

    let id = usecase.entry("Gauss", 27, Some(b"King of Math"));
    println!("inserted person {}", id);
    let id = usecase.entry("Galois", 20, Some(b"Group Theory"));
    println!("inserted person {}", id);
    let id = usecase.entry("Abel", 26, Some(b"Abelian Group"));
    println!("inserted person {}", id);
    let id = usecase.entry("Euler", 23, Some(b"Euler's Formula"));
    println!("inserted person {}", id);

    let rows = usecase.collect();
    for row in rows {
        println!(
            "found person id={} name='{}' age={} data='{:?}'",
            row.0, row.1, row.2, row.3
        );
    }
}
