// Typed mirror of docs/protocol.md. The Rust backend defines the serde types;
// this file MUST match their wire shape exactly. Enum casing follows the
// default serde casing used on the Rust side (PascalCase variants).

export type PlayerId = number

/** Which scoring ring a dart landed in (`crate::geometry::Ring`). */
export type Ring =
  | 'InnerBull'
  | 'OuterBull'
  | 'Single'
  | 'Treble'
  | 'Double'
  | 'Miss'

/** A scored throw (`crate::geometry::Score`). */
export interface Dart {
  ring: Ring
  /** 1–20 for ring hits, 25 for any bull, 0 for a miss. */
  segment: number
  /** Final point value. */
  value: number
}

export interface Player {
  id: PlayerId
  name: string
}

export type GameMode = 'x01' | 'cricket' | 'idle'

export type InRule = 'Straight' | 'Double'
export type OutRule = 'Straight' | 'Double' | 'Triple' | 'Master'

export type X01Start = 301 | 501 | 701 | 901

export interface X01Config {
  start: number
  in_rule: InRule
  out_rule: OutRule
}

export interface X01State {
  config: X01Config
  /** remaining score keyed by player id (string keys in JSON). */
  remaining: Record<string, number>
  /** suggested finish for the current player, or null. */
  checkout: Dart[] | null
}

export interface CricketConfig {
  cut_throat: boolean
}

export interface CricketState {
  cut_throat: boolean
  /** player id -> number ("20".."15","25" for bull) -> marks (0..3). */
  marks: Record<string, Record<string, number>>
  /** points keyed by player id. */
  points: Record<string, number>
}

/** Full game snapshot broadcast after every accepted command. */
export interface GameState {
  mode: GameMode
  players: Player[]
  current_player_id: PlayerId | null
  current_turn: Dart[]
  winner_id: PlayerId | null
  x01?: X01State
  cricket?: CricketState
}

// --- Client -> Server commands ---------------------------------------------

export type NewGamePayload =
  | { mode: 'x01'; x01: X01Config }
  | { mode: 'cricket'; cricket: CricketConfig }

export type ClientCommand =
  | { type: 'player_add'; name: string }
  | { type: 'player_rename'; id: PlayerId; name: string }
  | { type: 'player_remove'; id: PlayerId }
  | { type: 'player_reorder'; id: PlayerId; index: number }
  | ({ type: 'game_new' } & NewGamePayload)
  | { type: 'throw'; dart: Dart }
  | { type: 'undo_last' }
  | { type: 'edit_dart'; index: number; dart: Dart }
  | { type: 'end_turn' }
  | { type: 'reset' }

// --- Server -> Client events -----------------------------------------------

export interface StateEvent {
  type: 'state'
  state: GameState
}

export interface ErrorEvent {
  type: 'error'
  message: string
}

export type ServerEvent = StateEvent | ErrorEvent

// --- REST (game history) ---------------------------------------------------

export interface GameSummary {
  id: number
  mode: GameMode
  players: Player[]
  winner: Player | null
  finished_at: string
}

export interface GameTurn {
  player_id: PlayerId
  darts: Dart[]
}

export interface GameDetail extends GameSummary {
  turns: GameTurn[]
}

// --- REST (persistent players) ---------------------------------------------

/** A DB-backed saved player, distinct from the in-memory game roster. */
export interface SavedPlayer {
  /** uuid string (not the numeric in-game PlayerId). */
  id: string
  name: string
  created_at: string
}
