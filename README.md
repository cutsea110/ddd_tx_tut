# DDD with tx-rs tutorial

[![Rust](https://github.com/cutsea110/ddd_tx_tut/actions/workflows/rust.yml/badge.svg)](https://github.com/cutsea110/ddd_tx_tut/actions/workflows/rust.yml)

## Usage

```bash
docker compose up -d
export DATABASE_URI="postgres://admin:adminpass@localhost:15432/sampledb"
export CACHE_URI="redis://:adminpass@localhost:16379"
export AMQP_URI="amqp://admin:adminpass@localhost:5672/%2f"
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

The postgres db's named volumes are empty in `docker-compose.yml`, so you lost permanent data after `docker compose down -v`.


If you check nosql(dynamodb-local), do like this:

```bash
export AWS_ACCESS_KEY_ID=dummy
export AWS_SECRET_ACCESS_KEY=dummy
aws dynamodb --endpoint-url http://localhost:18000 list-tables

# person_id の次のカウントをインクリメント
aws dynamodb --endpoint-url http://localhost:18000 \
	update-item \
    --table-name person \
    --key '{"PK": {"S": "person-counter"}, "SK": {"S": "person_id"}}' \
    --update-expression "ADD person_id :incr" \
    --expression-attribute-values '{":incr": {"N": "1"}}' \
    --return-values UPDATED_NEW
					
# 上で返ってきた person_id - 1 を使ってよい
# person_id が 4 なら 3 を使う
aws dynamodb --endpoint-url http://localhost:18000 \
    put-item --table-name person \
	--item '{"PK":{"S":"person#3"},"SK":{"S":"person"},"id":{"N":"3"},"name":{"S":"Abel"},"birth_date":{"S":"1802-08-05"},"death_date":{"S":"1829-04-06"},"data":{"S":"Abel's theorem"}}'
aws dynamodb --endpoint-url http://localhost:18000 \
    scan --table-name person
aws dynamodb --endpoint-url http://localhost:18000 \
    scan --table-name person \
    --filter-expression "SK = :sk" \
    --expression-attribute-values '{":sk":{"S":"person"}}'
aws dynamodb --endpoint-url http://localhost:18000 \
    get-item --table-name person --key '{"PK":{"S":"person#1"},"SK":{"S":"person"}}'
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

For example, in the case you want to look at queue, do like this:

```bash
docker exec $(docker ps -f "name=rabbitmq" --format "{{.ID}}") \
       rabbitmqctl list_queues
```

If you use rabbitmqadmin(administrator control cli), this command is in container too, do like this:

```bash
docker exec $(docker ps -f "name=rabbitmq" --format "{{.ID}}") \
       rabbitmqadmin help subcommands
```

The redis cache's named volumes are empty in `docker-compose.yml`, so you lost cache data after `docker compose down -v` and then, `rm data/shared-local-instance.db`.

## Build

If you want to use postgresql as backend, then you should build with features flag with `use_pq`.

```bash
cargo build --bin app-pq --features=use_pq
```

Alternatively, if you want to use dynamodb as backend, then you should build with features flag with `use_dynamo`.

```bash
cargo build --bin app-dynamo --features=use_dynamo
```

## Run

```bash
cargo run --bin app-pq --features=use_pq
```

if you want to see log message

```bash
RUST_LOG=app=debug cargo run --bin app-pq --features=use_pq
```

## Test

run unit test with rdb.

```bash
cargo test --features=use_pq
```

## More Information

You should export `DATABASE_URL` and `CACHE_URL` environment variables on the terminal which you run your editor.
