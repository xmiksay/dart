# dartscore

Self-hosted automatic scoring for steel-tip darts, built for a Raspberry Pi wired
to a projector. A YOLOv8 detector (candle, added in a later phase) does one narrow
job — from a single frame return four board calibration points and up to three dart
tips — and **everything downstream is deterministic Rust**: homography, board
mapping, score classification, game logic, and web UI.

## Status

| Phase | Scope | State |
|------|-------|-------|
| 2 | Geometry + homography + scoring (pure Rust) | ✅ implemented |
| 0/1 | DeepDarts training + candle inference parity | planned |
| 3 | Camera capture + trigger | planned |
| 4 | Axum/WebSocket web UI, X01/Cricket, corrections | planned |
| 5 | Retrain loop on own data | planned |

See `../../../.claude/plans/` for the full phased plan.

## Architecture

```
src/
  geometry.rs    board constants, number ring, polar -> (segment, ring) classify
  homography.rs  4-point DLT homography (nalgebra), warp points
  scoring.rs     detection (calib pts + tips) -> Vec<Score>
  detect.rs      candle YOLOv8 inference            (Phase 1, planned)
  capture.rs     camera ingest + trigger            (Phase 3, planned)
  game/          X01 / Cricket engine               (Phase 4, planned)
  web.rs         Axum + WebSocket + SPA serve       (Phase 4, planned)
  main.rs        CLI: score | detect | capture | serve
```

Board geometry uses the WDF/BDO regulation spec: radii measured to the wire,
20 segments of 18° with the segment boundaries offset 9° from the cardinal axes,
number ring `20,1,18,4,13,6,10,15,2,17,3,19,7,16,8,11,14,9,12,5` clockwise from top.

## Usage

Score a detection frame (four calibration points + dart tips, in image pixels):

```sh
make run ARGS="score examples/detection.json"
# dart 1: InnerBull 25 -> 50
# dart 2: Treble 20 -> 60
# dart 3: Single 6 -> 6
# total: 116
```

The JSON schema matches `dartscore::Detection`: `calibration` is four points in
fixed order (5&20 top, 13&6 right, 17&3 bottom, 8&11 left), `tips` is the list of
detected dart tips.

## Commands

| `make` target | What it does |
|---------------|--------------|
| `make build` / `make release` | Compile (debug / release) |
| `make run ARGS="..."` | Run the CLI |
| `make test` | All tests |
| `make unit` | In-crate unit tests only |
| `make integration` | `tests/` integration tests only |
| `make lint` | `cargo clippy --all-targets -D warnings` |
| `make fmt` / `make fmt-check` | Format / check formatting |
| `make cross` | Cross-compile for the Pi (`ARM_CPU=cortex-a53` RPi3 / `cortex-a76` RPi5) |
| `make clean` | Remove build artifacts |

Cross-compilation requires [`cross`](https://github.com/cross-rs/cross). candle on
ARM uses the default CPU backend (NEON via the `gemm` crate) — do **not** enable
`mkl`/`accelerate`/`cuda`/`metal` features (x86/Apple/NVIDIA only).
