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
| 3 | Camera capture + motion-settle trigger | ✅ trigger (tested); ⏳ live run needs camera |
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
  capture/           camera ingest + motion-settle trigger (feature `capture`)
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

### Live camera scoring into the web game

When built with `--features capture` (which `make serve` does) and **`DART_CAMERA`
is set**, `serve` runs the capture loop in the background and feeds each detected
dart into the live game as a throw (broadcast to the scoreboard):

```sh
make db-up
DART_CAMERA=0 DART_EXPOSURE=157 \
  DATABASE_URL=postgres://dartscore:dartscore@localhost:5432/dartscore make serve
```
Env: `DART_CAMERA` (required to enable), `DART_EXPOSURE`, `DART_BG_THRESHOLD`
(35), `DART_BG_MIN_AREA` (120). Requirements for darts to register: a
`calibration.json` for that camera, an **empty board when serve starts** (the
reference frame), and **a game started in the UI** — darts thrown with no active
game are dropped (logged at `debug`). With `DART_CAMERA` unset, the server runs
normally with no camera.

## Calibration (fixed camera, one-time)

The board is calibrated by marking the four outer-double-edge intersections —
**5&20 (top), 13&6 (right), 17&3 (bottom), 8&11 (left)** — once:

```sh
make cameras                         # find your camera index
make snapshot CAMERA=2               # -> captures/snapshot.png
# open snapshot.png, read the 4 board-point pixel coords (in that order), then:
make calibrate POINTS="x1,y1 x2,y2 x3,y3 x4,y4"   # -> calibration.json
make score-tip POINTS="640,360"      # validate: poke known board spots, check the score
```

`calibrate` prints a sanity check (the points' centre should map near the bull).
Re-run only if the camera moves. The homography is then fixed; the detector only
needs to find dart tips.

### Lock the camera (stable background for subtraction)

Auto-exposure/white-balance make the whole frame drift, which floods background
subtraction. dartscore locks the camera itself **after opening the stream** (the
open resets controls, so setting them externally beforehand has no effect):

```sh
make capture CAMERA=0 EXPOSURE=157     # manual exposure; tune the value by eye
# extra controls: cargo run --features capture -- capture --camera 0 \
#     --exposure 157 --ctrl gamma=120 --ctrl sharpness=3
```

`--exposure` sets `auto_exposure=1` (manual) + `exposure_time_absolute` and
disables dynamic-framerate / auto-WB / gain / backlight via `v4l2-ctl` (needs
`v4l-utils`). Requires a camera that exposes manual exposure (check
`make cameras` / `v4l2-ctl --list-ctrls-menus`; some cheap cams don't). For
50 Hz mains, use an exposure that's a multiple of 10 ms (units are 100 µs, so
`100`/`200`) to avoid flicker banding.

**Auto-calibration:** the trigger gates (`--motion-on`/`--motion-off`) and the
bgsub cutoff (`--bg-threshold`) are **measured automatically** at startup from
the empty-board noise floor — just omit them and keep the board empty + still for
~1 s while it calibrates (it logs the chosen `thresholds: …`). Pass any of them
explicitly to override.

### Scoring without a trained model (background subtraction)

Once `calibration.json` exists, `make capture` scores **without any ML**: the
first frame is taken as the empty-board reference (start with an empty board),
and each settled throw is diffed against it to find the new dart, whose tip is
mapped through the calibration and scored. Detection precedence in the capture
loop: trained model → background-subtraction (calibration) → capture-only (save
frames). Background subtraction is a bridge for a fixed camera; tune
`threshold`/`min_area` and use the web manual-correction UI for misreads.

## Commands

| `make` target | What it does |
|---------------|--------------|
| `build` / `release` | Compile (release builds the SPA + embeds it) |
| `run ARGS="..."` / `serve` | Run the CLI / the web server |
| `web` / `web-install` | Build / install the Vue SPA |
| `db-up` / `db-down` | Start / stop local Postgres |
| `detect` | Build with the candle detection feature |
| `cameras` | List available cameras and their indices |
| `calibrate POINTS="..."` | Save a one-time manual calibration from the 4 marked board points |
| `score-tip POINTS="..."` | Score dart-tip pixels against `calibration.json` (validate the board) |
| `snapshot [CAMERA=n]` | Grab one frame from camera `n` → `captures/snapshot.png` (quick check that an index delivers video) |
| `capture [CAMERA=n] [INTERVAL=s] [EXPOSURE=e]` | Run live capture on camera `n` (default 0); `EXPOSURE` locks manual exposure (stable background); saves frames to `captures/` on motion-settle or every `s` seconds; scores via calibration/model |
| `test` / `unit` / `integration` | Tests (all / lib / `tests/`) |
| `lint` / `fmt` / `fmt-check` | clippy `-D warnings` / rustfmt |
| `cross` | Cross-compile for the Pi (`ARM_CPU=cortex-a53` RPi3 / `cortex-a76` RPi5) |

Detection training/conversion lives in `python/` (see `python/README.md`); the
single artifact crossing to the Pi is `models/dartscore.safetensors`. candle on
ARM uses the default CPU backend (NEON via `gemm`) — do **not** enable
`mkl`/`accelerate`/`cuda`/`metal` features.
