# dartscore — build/run/lint/test orchestration.
# Limit cargo parallelism to keep the build host responsive.
export CARGO_BUILD_JOBS ?= 4

# Cross target for the Raspberry Pi (aarch64). RPi3 = cortex-a53, RPi5 = cortex-a76.
ARM_TARGET ?= aarch64-unknown-linux-gnu
ARM_CPU    ?= cortex-a53

.PHONY: all build release run lint fmt fmt-check test unit integration \
        web web-install serve db-up db-down detect cross clean

all: lint test

build:
	cargo build

# Release build embeds the SPA via rust-embed, so the frontend must be built first.
release: web
	cargo build --release

# Score a detection JSON frame, e.g. `make run ARGS="score examples/detection.json"`.
run:
	cargo run -- $(ARGS)

# Run the web + WebSocket server (needs DATABASE_URL; see `make db-up`).
serve:
	cargo run -- serve

# --- Frontend (Vue SPA, embedded into the release binary) ---
web-install:
	cd web && npm install

# Build the SPA to web/dist (consumed by rust-embed at release time).
web:
	cd web && npm run build

# --- Database (local Postgres for development) ---
db-up:
	docker compose up -d postgres

db-down:
	docker compose down

# --- Detection (Phase 1: candle YOLOv8, gated feature) ---
detect:
	cargo build --features detection

lint:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

test:
	cargo test

# Pure-logic unit tests (in-crate `#[cfg(test)]` modules).
unit:
	cargo test --lib

# Integration tests (the `tests/` directory).
integration:
	cargo test --test '*'

# Cross-compile for the Pi (builds the SPA first for embedding).
# Requires `cross` (cargo install cross).
cross: web
	RUSTFLAGS="-C target-cpu=$(ARM_CPU)" cross build --release --features detection --target $(ARM_TARGET)

clean:
	cargo clean
