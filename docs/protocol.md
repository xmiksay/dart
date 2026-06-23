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
| `undo_last` | — | remove last dart of the in-progress turn |
| `edit_dart` | `index: number, dart: Dart` | correct a dart in the current turn |
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
| `GET /api/games` | list of finished games (id, mode, players, winner, finished_at) |
| `GET /api/games/{id}` | one game with its turns/darts |
| `GET /api/health` | `{ "status": "ok" }` |

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
