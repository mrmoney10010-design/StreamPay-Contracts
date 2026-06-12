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

.PHONY: check
check:
	cargo check --all-targets

# Run formatting, lint, and test checks the way CI would.
.PHONY: lint
lint: fmt-check clippy test

.PHONY: optimize
optimize: build
	stellar contract optimize --wasm $(WASM)

# Deploy to a network. Override NETWORK and SOURCE as needed, e.g.:
#   make deploy NETWORK=testnet SOURCE=alice
NETWORK ?= testnet
SOURCE ?= default

.PHONY: deploy
deploy: build
	stellar contract deploy \
		--wasm $(WASM) \
		--source $(SOURCE) \
		--network $(NETWORK)

.PHONY: clean
clean:
	cargo clean
