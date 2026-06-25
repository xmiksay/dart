# dartscore — Web protocol contract

The web backend (Axum) and the Vue SPA communicate over a single **WebSocket**
at `GET /ws` for real-time play, plus a small **REST** surface for game history.
This document is the authoritative contract; the Rust backend defines the
serde types and the TS frontend mirrors them.

All WebSocket messages are JSON objects with a `"type"` discriminator
(`#[serde(tag = "type", rename_all = "snake_case")]` on the Rust side).

## Shared value types

A **Dart** is a scored throw (`crate::geometry::Score`), serialized as:

```json
{ "ring": "Treble", "segment": 20, "value": 60 }
```

`ring` ∈ `"InnerBull" | "OuterBull" | "Single" | "Treble" | "Double" | "Miss"`
(default serde casing). `segment` is 1–20 (25 for bull, 0 for miss). `value` is
the point total.

A **Player** is `{ "id": 0, "name": "Ann" }` (`id` is a u32).

## Client → Server commands

| `type` | fields | meaning |
|--------|--------|---------|
| `player_add` | `name: string` | add a player |
| `player_rename` | `id, name` | rename |
| `player_remove` | `id` | remove |
| `player_reorder` | `id, index` | move in turn order |
| `game_new` | `mode`, `x01?`, `cricket?` | start a new game (see below) |
| `throw` | `dart: Dart` | apply a dart for the current player |
| `undo_last` | — | remove the most recent dart anywhere in the leg (reopens the previous turn if the current one already closed; works after a win) |
| `edit_dart` | `turn_index: number, dart_index: number, dart: Dart` | correct any dart in any turn — the engine replays the leg to recompute. `turn_index` indexes `GameState.turns` (the last entry is the in-progress turn) |
| `end_turn` | — | force-advance to the next player |
| `reset` | — | clear the game back to idle |

`game_new` payloads:

```json
{ "type": "game_new", "mode": "x01",
  "x01": { "start": 501, "in_rule": "Straight", "out_rule": "Double" } }

{ "type": "game_new", "mode": "cricket",
  "cricket": { "cut_throat": false } }
```

`start` ∈ {301, 501, 701, 901}. `in_rule` ∈ `"Straight" | "Double"`.
`out_rule` ∈ `"Straight" | "Double" | "Triple" | "Master"`.

## Server → Client events

After **every** accepted command the server broadcasts the full state:

```json
{ "type": "state", "state": { /* GameState, see below */ } }
```

Errors:

```json
{ "type": "error", "message": "leaving 1 on a double-out is a bust" }
```

### GameState

```jsonc
{
  "mode": "x01" | "cricket" | "idle",
  "players": [ { "id": 0, "name": "Ann" } ],
  "current_player_id": 0,            // null when idle
  "current_turn": [ /* Dart */ ],     // darts thrown so far this turn
  "turns": [                          // full leg history, oldest first; the last
    { "player": 0, "darts": [ /* Dart */ ] }   // entry is the in-progress turn
  ],
  "winner_id": null,                  // PlayerId when the game is won
  "x01": {                            // present iff mode == "x01"
    "config": { "start": 501, "in_rule": "Straight", "out_rule": "Double" },
    "remaining": { "0": 360, "1": 501 },   // keyed by player id
    "checkout": [ /* Dart */ ] | null      // suggested finish for current player
  },
  "cricket": {                        // present iff mode == "cricket"
    "cut_throat": false,
    "marks":  { "0": { "20": 3, "19": 1 } },  // player -> number -> marks (0..3)
    "points": { "0": 60, "1": 0 }
  }
}
```

## REST (persisted via SeaORM/Postgres)

### Game history
| method · path | returns |
|---------------|---------|
| `GET /api/games` | list of finished games — `{ id, mode, players: string[], winner: string \| null, finished_at }` (names are the persisted roster snapshot) |
| `GET /api/games/{id}` | one raw game record with its `players`/`darts` rows |
| `GET /api/games/{id}/state` | the finished game reconstructed as a full [`GameState`](#gamestate) by replaying its stored dart log — lets the SPA render any historical game on the same round-grid scoreboard as a live one |
| `GET /api/health` | `{ "status": "ok" }` |

### Statistics
`GET /api/stats` returns X01 statistics over rolling windows (Cricket excluded):

```jsonc
{ "windows": [ {
  "days": 7,                      // also 14, 30, 90
  "games_played": 12,
  "sixties": 41,                  // single darts worth 60 (treble 20)
  "best_average_player":  { "name": "Ann", "average": 64.2, "games": 8, "darts": 207 } | null,
  "best_winrate_player":  { "name": "Ann", "win_rate": 0.75, "wins": 6, "games": 8 } | null,
  "best_game_average":    { "game_id": "uuid", "player": "Ann", "average": 91.0, "start": 501, "finished_at": "..." } | null,
  "fewest_darts_game":    { "game_id": "uuid", "player": "Ann", "darts": 15, "start": 501, "finished_at": "..." } | null
} ] }
```

Averages are the canonical 3-dart figure `scored / darts * 3` where
`scored = start - remaining` (busts count their darts but credit nothing).
`best_*_player` consider only saved players (linked `players.id`); `best_game_*`
include guests.

### Persistent players ("player table")
A DB-backed roster of known players that the setup screen picks from (separate
from the in-memory per-game roster). A `Player` here is
`{ "id": "uuid", "name": "Ann", "created_at": "..." }`.

| method · path | body | returns |
|---------------|------|---------|
| `GET /api/players` | — | list of saved players |
| `POST /api/players` | `{ "name": "Ann" }` | created player (409 on duplicate name) |
| `PATCH /api/players/{id}` | `{ "name": "Anna" }` | updated player |
| `DELETE /api/players/{id}` | — | 204 |

Starting a game: the SPA fetches `/api/players`, the operator selects/adds
people, and the chosen names are pushed into the in-memory game roster via the
`player_add` WS command. A newly typed name should also be POSTed to
`/api/players` so it persists for next time. Finished-game records link each
participant back to `players.id` (nullable FK) for future per-player stats.

### Camera calibration (browser, one-time)

A fixed camera is calibrated once by marking the four board points in a live
frame. Camera endpoints require the server to be built with `--features capture`;
without it they return 501. The calibration is stored on disk as
`calibration.json` (the same file the CLI `calibrate`/`score-tip` and the capture
loop use) — one shared source of truth.

| method · path | body / query | returns |
|---------------|--------------|---------|
| `GET /api/cameras` | — | `[{ "index": 0, "name": "HD 1080P" }]` (501 if not built with capture) |
| `GET /api/camera/snapshot?camera=N` | — | `image/png` of one frame (501 if no capture) |
| `GET /api/calibration` | — | `{ "points": [[x,y],[x,y],[x,y],[x,y]] }` or `null` if none saved |
| `POST /api/calibration` | `{ "points": [[x,y]×4] }` | `{ "ok": true, "centre": [bx,by] }` (board-mm centre, ≈ bull as a sanity check; 400 if the quad is degenerate) |

The four points are in **canonical order**: 5&20 (top), 13&6 (right), 17&3
(bottom), 8&11 (left), in image pixels (origin top-left). Build the homography
with `dartscore::calibration::Calibration::new(points)` and persist via `.save()`.

## Static SPA serving (rust-embed, single binary)

The built SPA (`web/dist`) is **embedded into the Rust binary at compile time
via the `rust-embed` crate** and served from memory — no files on disk on the
Pi. Requests under `/assets/*` and other static paths are served from the
embedded bundle; any unmatched non-`/api`, non-`/ws` route falls back to the
embedded `index.html` for client-side routing. Build order for release:
`npm run build` (→ `web/dist`) **before** `cargo build --release`.

## Notes for implementers

- The backend holds one in-memory `GameSession` (current game + roster) behind a
  shared lock, and a broadcast channel fanning `state` events to all WS clients.
- A finished game (winner set) is written to Postgres; live play stays in memory.
- The detector/capture pipeline feeds darts in via the same `throw` path, so the
  manual `throw`/`edit_dart`/`undo_last` commands and auto-detection share logic.
