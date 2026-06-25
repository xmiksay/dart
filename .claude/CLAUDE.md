# CLAUDE.md — dartscore

Automatic scoring for steel-tip darts, self-hosted on a **Raspberry Pi** wired to
a **projector**. A YOLOv8 detector (candle) has one narrow job — from a single
frame return four board calibration points + up to three dart tips — and
**everything downstream is deterministic Rust**: homography, board mapping, score
classification, game logic, persistence, web UI. The SPA is embedded into the
binary (rust-embed), so deployment is a single file.

See also: [`../README.md`](../README.md) · [`../docs/protocol.md`](../docs/protocol.md) (WS/REST contract) · [`../python/README.md`](../python/README.md) (training).

## Architecture principles

1. **Narrow ML surface.** The net returns *points only* (4 calibration + ≤3 tips). Homography, scoring, and game logic never touch ML.
2. **Clean artifact boundary.** One `safetensors` file crosses train→runtime. Retrain = swap the file. Training lives offline in `python/`; the Pi only loads the model.
3. **Engine is the single source of truth.** The game engines hold the full per-leg dart history; all derived state (remaining, marks, points, current player, winner) is recomputed by **replaying the dart log**. Edits/undo are just "mutate the log, replay" — this keeps corrections correct by construction.
4. **Frozen detector class layout (5 classes).** 0–3 = the four calibration corners (per-corner classes: 5&20 top, 13&6 right, 17&3 bottom, 8&11 left), 4 = dart tip. candle reimplements the net architecture in Rust, so the head's `nc` MUST match the weights — never change the class count without updating both sides.

## Module layout (single Rust crate `dartscore`, modules only — no workspace)

```
src/
  geometry.rs      board constants (WDF/BDO mm), number ring, polar classify, Score
  homography.rs    4-point DLT homography via nalgebra (smallest eigenvector of AᵀA)
  scoring.rs       Detection (4 calib pts + tips) -> Vec<Score>
  calibration.rs   one-time manual calibration (Calibration: 4 pts -> homography), calibration.json
  game/            X01 + Cricket engines + Roster (pure, replay-based history)
  detect/          candle YOLOv8n inference  (feature `detection`; model.rs vendored from candle)
  capture/         camera ingest + motion-settle trigger  (feature `capture`)
  db/              SeaORM entities, migrations, repository (Postgres)
  web/             Axum HTTP + WebSocket server + REST
  main.rs          CLI: score | calibrate | score-tip | detect | capture | serve
web/               Vue 3 + TS SPA (projector UI); embedded via rust-embed
python/            offline YOLOv8 training + .pt->safetensors conversion (dev GPU)
docs/protocol.md   authoritative WS/REST contract
models/            safetensors artifact (git-ignored)
```

400-line file cap; `game/` and `detect/` are folder modules for that reason.

## Tech stack

- **Rust**: tokio, axum (+ws), tower-http, sea-orm (Postgres, rustls), nalgebra, candle-core/candle-nn (CPU), nokhwa (v4l), image, clap, anyhow, tracing, serde, rust-embed.
- **Frontend**: Vue 3 (Composition API), TypeScript, Vite, Tailwind, Pinia. Node via `nvm` (`.nvmrc` → Node 22; always `nvm use`).
- **Training**: Python + Ultralytics/PyTorch + safetensors (dev machine w/ NVIDIA GPU).

## Feature flags

- `default` — core + web + db (no ML, no camera). Light build.
- `detection` — candle YOLOv8n (`dep:candle-core/candle-nn/image`).
- `capture` — `["detection", "dep:nokhwa"]`: camera + the live scoring loop.

`make serve` builds with `--features capture` so the browser calibration screen can grab frames. `make cross` builds the Pi binary with `--features capture`.

## Commands (use the Makefile, not ad-hoc)

| Target | Purpose |
|---|---|
| `make build` / `release` | compile (release builds the SPA + embeds it) |
| `make serve` | web + WS server (needs `DATABASE_URL`; built w/ capture) |
| `make web` / `web-install` | build / install the Vue SPA |
| `make db-up` / `db-down` | local Postgres via docker-compose |
| `make test` / `unit` / `integration` | tests |
| `make lint` / `fmt` / `fmt-check` | clippy `-D warnings` / rustfmt |
| `make cameras` | list cameras + indices |
| `make snapshot CAMERA=n` | grab one frame → `captures/snapshot.png` |
| `make calibrate POINTS="x1,y1 …×4"` | save manual calibration |
| `make score-tip POINTS="x,y"` | score tip pixels vs `calibration.json` |
| `make capture CAMERA=n [INTERVAL=s] [MOTION_ON=… MOTION_OFF=… SETTLE=…]` | live capture loop |
| `make detect` / `build-capture` | build with detection / capture |
| `make cross` | cross-compile for the Pi (`ARM_CPU=cortex-a53` RPi3 / `cortex-a76` RPi5) |

## Subsystem notes

- **Calibration (fixed camera, one-time).** Mark the 4 board-edge intersections once → homography saved to `calibration.json` (shared by CLI, web, and the capture loop). Two ways: CLI (`calibrate`/`score-tip`) or the **browser Calibrate screen** (`/api/cameras`, `/api/camera/snapshot`, `GET`/`POST /api/calibration`). The detector then only needs dart tips.
- **Capture trigger.** Motion-settle: fire once after a dart lands *and the frame stabilizes*. Motion is a **localized max-block** metric (16px blocks) — a landing dart lights one block even though the whole-frame average is ~0 (a global average only spikes on dart *removal*). Thresholds tunable via `--motion-on/--motion-off/--settle-frames`. With no model present, capture runs in **capture-only** mode (saves settled frames for framing checks + training).
- **Web/game.** One in-memory `GameSession` (roster + active X01/Cricket game) behind a lock + a broadcast channel fanning full `GameState` to all WS clients after every command. `GameState.turns` carries the full leg history; `edit_dart {turn_index, dart_index, dart}` corrects any past turn (engine replays). Finished games + a persistent **players** table are stored in Postgres; the live session is snapshotted to a singleton `live_session` row so an in-progress game **survives a restart** (replay-based restore).
- **Detection/training.** `python/`: DeepDarts → YOLOv8 5-class → train (Ultralytics) → `.pt`→safetensors with the candle layer-name map → `models/dartscore.safetensors`. **Parity gate**: `cargo run --features detection -- detect <img>` must match `verify_detection.py` ±~1px before trusting it.

## Phase status

- ✅ Geometry/homography/scoring · game engine (X01 301–901 + dbl/tpl in/out, Cricket + cut-throat, roster, replay-based edit/undo) · web + WS + SeaORM/Postgres (history, players, live-session restore) · Vue SPA (round-grid scoreboard, manual correction, history, saved players, X01 stats over 7/14/30/90-day windows, historical-game replay onto the grid scoreboard) · rust-embed single binary · manual calibration (CLI + browser) · capture trigger.
- ⏳ Needs the user's GPU/hardware: train weights + Phase-1 parity run; live-capture threshold tuning on the real board; Phase 5 retrain loop.

## Conventions (in addition to the workspace `CLAUDE.md`)

- Tests ship with the change; `cargo test` stays green without a DB (DB tests `#[ignore]`). Lint clean (`cargo clippy --all-targets -- -D warnings`, also `--features capture`) + `cargo fmt`.
- No `unwrap`/`expect`/`panic!` on paths reachable from I/O, network, DB, or input — propagate with `?` + `.context(...)`; never leak raw DB errors to clients.
- Keep `docs/protocol.md` the single source of truth for the WS/REST contract; frontend and backend mirror it. Update docs in the same change as the code.
- Build verified across **default / `detection` / `capture`** before declaring done.

## Key decisions (don't relitigate)

- **YOLOv8n + candle**, not YOLO26. YOLO26 is DFL-free/NMS-free with a different head; candle ships only a yolo-v8 example, so it would need a from-scratch Rust port. Escape hatch if Pi latency ever fails: switch the runtime to ONNX (`ort`) and export YOLO26.
- **One-time manual calibration** for the fixed camera (not per-frame auto-detection) — simpler, no jitter, works before a model exists.
- **Single crate, modules only** (no workspace/subcrates). Vue SPA in its own `web/` dir, embedded at release.
