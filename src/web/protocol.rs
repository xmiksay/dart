//! Serde wire types for the WS/REST contract. The authoritative shapes live in
//! `docs/protocol.md`; this module must mirror them exactly.

use crate::game::x01::X01Config;
use crate::game::{Player, PlayerId, TurnRecord};
use crate::geometry::Score;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Game mode selector in `game_new` and `GameState.mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    X01,
    Cricket,
}

/// Cricket-specific `game_new` payload.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CricketConfig {
    pub cut_throat: bool,
}

/// Client → server commands.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    PlayerAdd {
        name: String,
    },
    PlayerRename {
        id: PlayerId,
        name: String,
    },
    PlayerRemove {
        id: PlayerId,
    },
    PlayerReorder {
        id: PlayerId,
        index: usize,
    },
    GameNew {
        mode: GameMode,
        #[serde(default)]
        x01: Option<X01Config>,
        #[serde(default)]
        cricket: Option<CricketConfig>,
    },
    Throw {
        dart: Score,
    },
    UndoLast,
    EditDart {
        turn_index: usize,
        dart_index: usize,
        dart: Score,
    },
    EndTurn,
    Reset,
}

/// Server → client events.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    State { state: GameState },
    Error { message: String },
}

/// Full game snapshot broadcast after every accepted command.
#[derive(Debug, Clone, Serialize)]
pub struct GameState {
    pub mode: &'static str,
    pub players: Vec<Player>,
    pub current_player_id: Option<PlayerId>,
    pub current_turn: Vec<Score>,
    /// Full leg history, oldest first; the last entry is the in-progress turn.
    /// Reuses the engine's [`TurnRecord`], which serializes as `{ player, darts }`.
    pub turns: Vec<TurnRecord>,
    pub winner_id: Option<PlayerId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x01: Option<X01State>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cricket: Option<CricketState>,
}

/// X01 sub-state (present iff `mode == "x01"`).
#[derive(Debug, Clone, Serialize)]
pub struct X01State {
    pub config: X01Config,
    /// Remaining score keyed by player id (as string).
    pub remaining: BTreeMap<String, u32>,
    /// Suggested checkout for the current player, if any.
    pub checkout: Option<Vec<Score>>,
}

/// Cricket sub-state (present iff `mode == "cricket"`).
#[derive(Debug, Clone, Serialize)]
pub struct CricketState {
    pub cut_throat: bool,
    /// player id -> number -> marks (only numbers with marks > 0).
    pub marks: BTreeMap<String, BTreeMap<String, u8>>,
    /// player id -> points.
    pub points: BTreeMap<String, u32>,
}
