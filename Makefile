.PHONY: all

all:
	cargo build --bin app-hs --features use_hash
	cargo build --bin app-pq --features use_pq
	cargo build --bin app-dynamo --features use_dynamo

test:
	cargo test --features use_hash
	cargo test --features use_pq
	cargo test --features use_dynamo

clean:
	cargo clean
	rm -f data/shared-local-instance.db
