# dartscore — build/run/lint/test orchestration.
# Limit cargo parallelism to keep the build host responsive.
export CARGO_BUILD_JOBS ?= 4

# Cross target for the Raspberry Pi (aarch64). RPi3 = cortex-a53, RPi5 = cortex-a76.
ARM_TARGET ?= aarch64-unknown-linux-gnu
ARM_CPU    ?= cortex-a53

.PHONY: all build release run lint fmt fmt-check test unit integration \
        web web-install serve db-up db-down detect detect-bg capture cameras calibrate \
        score-tip snapshot build-capture cross pi-package pi-deploy clean

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
# Built with `capture` so the browser calibration screen can grab camera frames.
serve:
	cargo run --features capture -- serve

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

# --- Camera capture (Phase 3, gated; implies detection, needs v4l) ---
# Select a camera with `make capture CAMERA=2` (see `make cameras` for indices).
# With no models/dartscore.safetensors present it runs in capture-only mode:
# saves each settled frame to captures/ (no scoring).
CAMERA ?= 0
capture:
	cargo run --features capture -- capture --camera $(CAMERA) \
		$(if $(INTERVAL),--interval $(INTERVAL)) \
		$(if $(MOTION_ON),--motion-on $(MOTION_ON)) \
		$(if $(MOTION_OFF),--motion-off $(MOTION_OFF)) \
		$(if $(SETTLE),--settle-frames $(SETTLE)) \
		$(if $(BG_THRESHOLD),--bg-threshold $(BG_THRESHOLD)) \
		$(if $(BG_MIN_AREA),--bg-min-area $(BG_MIN_AREA)) \
		$(if $(BG_BLUR),--bg-blur $(BG_BLUR)) \
		$(if $(EXPOSURE),--exposure $(EXPOSURE))

# Tune background-subtraction detection on two saved frames (no camera needed):
#   make detect-bg REFERENCE=captures/empty.png FRAME=captures/throw_0001.png \
#       [THRESHOLD=35] [MIN_AREA=120]
detect-bg:
	cargo run -- detect-bg --reference "$(REFERENCE)" --frame "$(FRAME)" \
		$(if $(THRESHOLD),--threshold $(THRESHOLD)) \
		$(if $(MIN_AREA),--min-area $(MIN_AREA)) \
		$(if $(DILATE),--dilate $(DILATE)) \
		$(if $(BLUR),--blur $(BLUR)) \
		$(if $(MASK),--debug-mask $(MASK))

# List available cameras and their indices.
cameras:
	cargo run --features capture -- capture --list

# One-time manual calibration (fixed camera). Read the 4 board-point pixels off
# a `make snapshot` frame (5&20 top, 13&6 right, 17&3 bottom, 8&11 left):
#   make calibrate POINTS="x1,y1 x2,y2 x3,y3 x4,y4"
calibrate:
	cargo run -- calibrate --points "$(POINTS)"

# Validate a calibration: score dart-tip pixels against calibration.json:
#   make score-tip POINTS="640,360 949,360"
score-tip:
	cargo run -- score-tip --points "$(POINTS)"

# Grab one frame from camera CAMERA and save it (quick camera check):
#   make snapshot CAMERA=2
snapshot:
	cargo run --features capture -- capture --camera $(CAMERA) --snapshot

# Build the capture binary without running it.
build-capture:
	cargo build --features capture

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
	RUSTFLAGS="-C target-cpu=$(ARM_CPU)" cross build --release --features capture --target $(ARM_TARGET)

# --- Raspberry Pi (arm64) deployment ---
PI     ?= pi@raspberrypi.local
PI_DIR ?= /opt/dartscore

# Bundle the Pi release: single binary (SPA embedded) + systemd unit + env example.
pi-package: cross
	rm -rf dist/pi && mkdir -p dist/pi
	cp target/$(ARM_TARGET)/release/dartscore dist/pi/
	cp deploy/dartscore.service deploy/dartscore-kiosk.service \
	   deploy/dartscore.env.example deploy/README.md dist/pi/
	@echo "packaged -> dist/pi/ (binary + systemd units + env example + guide)"

# Copy the bundle to the Pi over SSH, then restart the service.
#   make pi-deploy PI=pi@raspberrypi.local
pi-deploy: pi-package
	rsync -avz dist/pi/ $(PI):$(PI_DIR)/
	@echo "deployed to $(PI):$(PI_DIR) — now: ssh $(PI) 'sudo systemctl restart dartscore'"

clean:
	cargo clean
