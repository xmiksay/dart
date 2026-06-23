# dartscore

Self-hosted automatic scoring for steel-tip darts, built for a Raspberry Pi wired
to a projector. A YOLOv8 detector (candle) does one narrow job — from a single
frame return four board calibration points and up to three dart tips — and
**everything downstream is deterministic Rust**: homography, board mapping, score
classification, game logic, persistence, and a web UI served from the same binary.

## Status

| Phase | Scope | State |
|------|-------|-------|
| 2 | Geometry + homography + scoring (pure Rust) | ✅ |
| 0/1 | DeepDarts training (Python) + candle inference | ✅ pipeline; ⏳ weights/parity |
| 4 | Axum/WebSocket server, X01/Cricket engine, SeaORM/Postgres history, Vue SPA | ✅ |
| 3 | Camera capture + trigger | planned |
| 5 | Retrain loop on own data | planned |

Detector: **YOLOv8n via candle** (5 frozen classes: 4 calibration corners + dart).
ONNX/`ort` with YOLO26 is the documented fallback if Pi latency ever demands it.

## Components

```
src/                 Rust crate `dartscore`
  geometry.rs        board constants, number ring, polar -> (segment, ring)
  homography.rs      4-point DLT homography (nalgebra)
  scoring.rs         detection (calib pts + tips) -> Vec<Score>
  game/              X01 + Cricket engines, roster (pure Rust, fully tested)
  detect/            candle YOLOv8n inference (feature `detection`)
  db/                SeaORM entities, migrations, repository (Postgres)
  web/               Axum HTTP + WebSocket server (real-time game state)
  main.rs            CLI: score | detect | capture | serve
web/                 Vue 3 + TS SPA (projector UI); embedded via rust-embed
python/              offline training + .pt->safetensors conversion (dev GPU)
docs/protocol.md     authoritative WS/REST contract
```

Board geometry: WDF/BDO spec, 20 segments of 18° with boundaries offset 9° from
the cardinal axes, ring `20,1,18,4,13,6,10,15,2,17,3,19,7,16,8,11,14,9,12,5`.

## Quick start

```sh
# 1. Score a detection frame offline (no model/camera needed)
make run ARGS="score examples/detection.json"     # -> total: 116

# 2. Run the full app (web UI + WebSocket + history)
make db-up                                          # local Postgres via docker
make web                                            # build the SPA to web/dist
DATABASE_URL=postgres://dartscore:dartscore@localhost:5432/dartscore make serve
# open http://localhost:8080
```

The web client (`docs/protocol.md`) drives play over a WebSocket and persists
finished games + a saved-player roster to Postgres. The release binary embeds the
SPA (rust-embed), so deployment to the Pi is a single file — build the frontend
*before* `make release`/`make cross`.

## Commands

| `make` target | What it does |
|---------------|--------------|
| `build` / `release` | Compile (release builds the SPA + embeds it) |
| `run ARGS="..."` / `serve` | Run the CLI / the web server |
| `web` / `web-install` | Build / install the Vue SPA |
| `db-up` / `db-down` | Start / stop local Postgres |
| `detect` | Build with the candle detection feature |
| `test` / `unit` / `integration` | Tests (all / lib / `tests/`) |
| `lint` / `fmt` / `fmt-check` | clippy `-D warnings` / rustfmt |
| `cross` | Cross-compile for the Pi (`ARM_CPU=cortex-a53` RPi3 / `cortex-a76` RPi5) |

Detection training/conversion lives in `python/` (see `python/README.md`); the
single artifact crossing to the Pi is `models/dartscore.safetensors`. candle on
ARM uses the default CPU backend (NEON via `gemm`) — do **not** enable
`mkl`/`accelerate`/`cuda`/`metal` features.
