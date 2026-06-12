WASM_TARGET := wasm32-unknown-unknown
WASM := target/$(WASM_TARGET)/release/streampay_contract.wasm

.PHONY: build
build:
	cargo build --target $(WASM_TARGET) --release

.PHONY: test
test:
	cargo test

.PHONY: fmt
fmt:
	cargo fmt --all

.PHONY: fmt-check
fmt-check:
	cargo fmt --all -- --check

.PHONY: clippy
clippy:
	cargo clippy --all-targets -- -D warnings
