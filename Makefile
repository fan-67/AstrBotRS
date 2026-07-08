.PHONY: build test docker run

build:
	cargo build --release -p astrbot-cli

test:
	cargo test --test config_test --test provider_test --test pipeline_test

docker:
	docker compose build

run:
	cargo run --release -p astrbot-cli
