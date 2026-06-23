# dartscore — build/run/lint/test orchestration.
# Limit cargo parallelism to keep the build host responsive.
export CARGO_BUILD_JOBS ?= 4

# Cross target for the Raspberry Pi (aarch64). RPi3 = cortex-a53, RPi5 = cortex-a76.
ARM_TARGET ?= aarch64-unknown-linux-gnu
ARM_CPU    ?= cortex-a53

.PHONY: all build release run lint fmt fmt-check test unit integration cross clean

all: lint test

build:
	cargo build

release:
	cargo build --release

# Score a detection JSON frame, e.g. `make run ARGS="score examples/detection.json"`.
run:
	cargo run -- $(ARGS)

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

# Cross-compile for the Pi. Requires `cross` (cargo install cross).
cross:
	RUSTFLAGS="-C target-cpu=$(ARM_CPU)" cross build --release --target $(ARM_TARGET)

clean:
	cargo clean
