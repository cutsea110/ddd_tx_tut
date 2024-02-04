use postgres::{Client, NoTls};

fn main() {
    let mut client = Client::connect(
        "host=localhost port=15432 dbname=sampledb user=admin password=admin",
        NoTls,
    )
    .unwrap();

    let name = "Ferris";
    let data = None::<&[u8]>;
    client
        .execute(
            "INSERT INTO person (name, data) VALUES ($1, $2)",
            &[&name, &data],
        )
        .unwrap();

    for row in client
        .query("SELECT id, name, data FROM person", &[])
        .unwrap()
    {
        let id: i32 = row.get(0);
        let name: &str = row.get(1);
        let data: Option<&[u8]> = row.get(2);
        println!("found person {} {} {:?}", id, name, data);
    }
}
