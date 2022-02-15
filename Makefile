#!/usr/bin/make -f
CONTAINER_RUNTIME := $(shell which docker 2>/dev/null || which podman 2>/dev/null)

.PHONY: all
all: clean fmt lint test schema optimize

.PHONY: clean
clean:
	@cargo clean

.PHONY: fmt
fmt:
	@cargo fmt --all -- --check

.PHONY: lint
lint:
	@cargo clippy

.PHONY: build
build:
	@cargo build

.PHONY: test
test:
	@cargo test --verbose

.PHONY: schema
schema:
	@cargo run --example schema

.PHONY: coverage
coverage:
	@cargo tarpaulin --ignore-tests --out Html

.PHONY: optimize
optimize:
	$(CONTAINER_RUNTIME) run --rm -v $(CURDIR):/code:Z \
		--mount type=volume,source=ats-smart-contract_cache,target=/code/target \
		--mount type=volume,source=ats-smart-contract_registry_cache,target=/usr/local/cargo/registry \
		cosmwasm/rust-optimizer:0.12.5

.PHONY: install
install: optimize
	@cp artifacts/ats_smart_contract.wasm $(PIO_HOME)
