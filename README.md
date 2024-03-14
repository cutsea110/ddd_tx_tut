# tx-rs

[![Rust](https://github.com/cutsea110/tx-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/cutsea110/tx-rs/actions/workflows/rust.yml)

## Usage

```bash
docker-compose up -d
export DATABASE_URI="postgres://admin:adminpass@localhost:15432/sampledb"
export CACHE_URI="redis://:adminpass@localhost:16379"
export AMQP_URI="amqp:://admin:adminpass@localhost:5672/%2f"
```

If you check rdb directly, do like this:

```bash
psql postgres://admin:adminpass@localhost:15432/sampledb -c "select * from person"
```

or do like this to call prompt, and input command interactovely.

```bash
psql postgres://admin:adminpass@localhost:15432/sampledb
psql (16.2 (Debian 16.2-1)、サーバー 15.5)
"help"でヘルプを表示します。

sampledb=#
```

The postgres db's named volumes are empty in `docker-compose.yml`, so you lost permanent data after `docker-compose down -v`.


If you check nosql(dynamodb-local), do like this:

```bash
export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy
aws dynamodb --endpoint-url http://localhost:18000 list-tables

aws dynamodb --endpoint-url http://localhost:18000 create-table --cli-input-json file://person.json

aws dynamodb --endpoint-url http://localhost:18000 \
    put-item --table-name cities \
	--item '{"population":{"N":38164},"date_mod":{"S":"1950-6-22"},"key":{"S":"t0924"},"name":{"S":"足利"}}'

aws dynamodb --endpoint-url http://localhost:18000 \
    scan --table-name cities
aws dynamodb --endpoint-url http://localhost:18000 \
    get-item --table-name cities --key '{"key":{"S":"t0925"}}'
```

If you check cache(redis), do like this:

```bash
redis-cli -p 16379 --pass adminpass ping
```

or do like this to call prompt, and input command interactovely.

```bash
redis-cli -p 16379 --pass adminpass
Warning: Using a password with '-a' or '-u' option on the command line interface may not be safe.
127.0.0.1:16379>
```

If you use rabbitmqctl(rabbitmq-cli), this command is in container, do like this:

```bash
docker exec $(docker ps -f "name=rabbitmq" --format "{{.ID}}") \
       rabbitmqctl help
```

If you use rabbitmqadmin(administrator control cli), this command is in container too, do like this:

```
docker exec $(docker ps -f "name=rabbitmq" --format "{{.ID}}") \
       rabbitmqadmin help subcommands
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
