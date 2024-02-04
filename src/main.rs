use postgres::{Client, NoTls};

fn main() {
    let mut client = Client::connect(
        "postgresql://admin:adminpass@localhost:15432/sampledb",
        NoTls,
    )
    .unwrap();

    let name = "Ferris";
    let age = 42i32;
    let data = None::<&[u8]>;
    client
        .execute(
            "INSERT INTO person (name, age, data) VALUES ($1, $2, $3)",
            &[&name, &age, &data],
        )
        .unwrap();

    for row in client
        .query("SELECT id, name, age, data FROM person", &[])
        .unwrap()
    {
        let id: i32 = row.get(0);
        let name: &str = row.get(1);
        let age: i32 = row.get(2);
        let data: Option<&[u8]> = row.get(3);
        println!("found person {} {}({}) {:?}", id, name, age, data);
    }
}
