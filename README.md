# tx-rs

## Usage

```bash
docker-compose up -d
export DATABASE_URL="postgres://admin:adminpass@localhost:15432/sampledb"
```

If you check rdb directly, do like this:

```bash
psql postgres://admin:adminpass@localhost:15432/sampledb
```

## Run

```
cargo run
```

if you want to see log message

```
RUST_LOG=app=debug cargo run
```

## Test

run unit test without rdb.

```
cargo test
```

## More Information

You should export `DATABASE_URL` environment variable on the terminal which you run your editor.
