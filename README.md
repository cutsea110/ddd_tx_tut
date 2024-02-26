# tx-rs

[![Rust](https://github.com/cutsea110/tx-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/cutsea110/tx-rs/actions/workflows/rust.yml)

## Usage

```bash
docker-compose up -d
export DATABASE_URL="postgres://admin:adminpass@localhost:15432/sampledb"
export CACHE_URL="redis://:adminpass@localhost:16379"
```

If you check rdb directly, do like this:

```bash
psql postgres://admin:adminpass@localhost:15432/sampledb
```

The postgres db's named volumes are empty in `docker-compose.yml`, so you lost permanent data after `docker-compose down -v`.

If you check cache(redis), do like this:

```bash
redis-cli -p 16379
```

The redis cache's named volumes are empty in `docker-compose.yml`, so you lost cache data after `docker-compose down -v`.

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

You should export `DATABASE_URL` and `CACHE_URL` environment variables on the terminal which you run your editor.
