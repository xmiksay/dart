# dartscore — web SPA

Projector-facing Vue 3 SPA for the `dartscore` automatic dart-scoring system.
It talks to the Rust/Axum backend over a WebSocket for live play and REST for
game history, and is served in production as static files from `web/dist`
(SPA fallback to `index.html`).

## Stack

Vue 3 (`<script setup>`, Composition API) · TypeScript · Vite · Tailwind CSS ·
Pinia · Vue Router · ESLint + `vue-tsc`.

## Commands

```bash
nvm use            # Node version from .nvmrc (22)
npm install
npm run dev        # dev server on :5173, proxies /ws and /api to :8080
npm run build      # type-check (vue-tsc) + production build into dist/
npm run preview    # preview the production build
npm run lint       # eslint --fix
npm run typecheck  # vue-tsc --noEmit
```

## Backend it expects

A running Axum backend on `http://localhost:8080` exposing:

- `GET /ws` — WebSocket live-play channel (commands up, `state`/`error` down).
- `GET /api/games`, `GET /api/games/{id}`, `GET /api/health` — REST history.

In dev, `vite.config.ts` proxies `/ws` and `/api` to that origin. In
production the SPA is served from the same origin as the backend, so no proxy
is needed. The wire contract is defined in [`../docs/protocol.md`](../docs/protocol.md)
and mirrored in `src/api/protocol.ts`.

## Layout

| Path | Role |
|------|------|
| `src/api/protocol.ts` | Typed mirror of the protocol (commands, events, `GameState`). |
| `src/api/ws.ts` | WebSocket client (reconnect + JSON framing). |
| `src/api/client.ts` | REST wrapper for `/api`. |
| `src/stores/game.ts` | Pinia store: live `GameState` + command senders. |
| `src/stores/history.ts` | Pinia store: finished-game history. |
| `src/views/` | `ScoreboardView`, `SetupView`, `HistoryView`. |
| `src/components/setup/` | Roster + game configuration. |
| `src/components/scoreboard/` | X01 / Cricket boards, dart slots, banners. |
| `src/components/correction/` | Operator dart picker + correction panel. |

## Screens

- **Setup** (`/setup`) — manage roster (add/rename/remove/reorder) and pick the
  game: X01 (start/in/out rules) or Cricket (cut-throat).
- **Scoreboard** (`/`) — the projector view: per-player scores, current turn's
  three darts, X01 checkout hint or Cricket marks grid, and win/bust banners.
  An operator side panel does manual `throw` / `edit_dart` / `undo_last`.
- **History** (`/history`) — finished games with winner, date and turn detail.
