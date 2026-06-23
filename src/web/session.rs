//! In-memory game session: a persistent roster plus an optional active game and
//! the command handler that mutates it and builds wire state. The engines own
//! the authoritative whole-leg history; the session keeps no duplicate of it.

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

use crate::db::repository::{NewDart, NewGame, NewPlayer};
use crate::game::cricket::{CricketGame, NUMBERS};
use crate::game::x01::{X01Config, X01Game};
use crate::game::{Player, PlayerId, Roster, TurnRecord, DARTS_PER_TURN};
use crate::geometry::Score;

use super::protocol::{Command, CricketState, GameMode, GameState, X01State};

/// The active game engine, behind a mode-agnostic facade.
pub enum ActiveGame {
    X01(X01Game),
    Cricket(CricketGame),
}

impl ActiveGame {
    fn winner(&self) -> Option<PlayerId> {
        match self {
            ActiveGame::X01(g) => g.winner(),
            ActiveGame::Cricket(g) => g.winner(),
        }
    }

    fn throw(&mut self, dart: Score) -> Result<()> {
        match self {
            ActiveGame::X01(g) => g.throw(dart).map(|_| ()),
            ActiveGame::Cricket(g) => g.throw(dart).map(|_| ()),
        }
    }

    fn undo_last(&mut self) -> Result<Option<Score>> {
        match self {
            ActiveGame::X01(g) => g.undo_last_dart(),
            ActiveGame::Cricket(g) => g.undo_last_dart(),
        }
    }

    fn edit_dart(&mut self, turn_index: usize, dart_index: usize, dart: Score) -> Result<()> {
        match self {
            ActiveGame::X01(g) => g.edit_dart(turn_index, dart_index, dart),
            ActiveGame::Cricket(g) => g.edit_dart(turn_index, dart_index, dart),
        }
    }

    /// The full leg, oldest first; the last entry is the in-progress turn.
    pub(super) fn turns(&self) -> Vec<TurnRecord> {
        match self {
            ActiveGame::X01(g) => g.turns(),
            ActiveGame::Cricket(g) => g.turns(),
        }
    }

    /// Darts thrown so far in the in-progress turn.
    fn live_turn_len(&self) -> usize {
        match self {
            ActiveGame::X01(g) => g.current_turn().len(),
            ActiveGame::Cricket(g) => g.current_turn().len(),
        }
    }
}

/// Snapshot of the config/order captured when a game starts (the engines do not
/// expose their config or remember roster order independently of play).
pub(super) struct GameMeta {
    pub(super) mode: GameMode,
    pub(super) x01: Option<X01Config>,
    pub(super) cut_throat: Option<bool>,
    pub(super) started_at: DateTime<Utc>,
    order: Vec<PlayerId>,
    names: Vec<String>,
}

impl GameMeta {
    /// Build meta from a roster, capturing turn order and names at game start
    /// (or at restore time, derived from the restored roster).
    pub(super) fn new(
        mode: GameMode,
        x01: Option<X01Config>,
        cut_throat: Option<bool>,
        started_at: DateTime<Utc>,
        roster: &Roster,
    ) -> Self {
        Self {
            mode,
            x01,
            cut_throat,
            started_at,
            order: roster.ids(),
            names: roster.players().iter().map(|p| p.name.clone()).collect(),
        }
    }
}

#[derive(Default)]
pub struct GameSession {
    pub(super) roster: Roster,
    pub(super) game: Option<ActiveGame>,
    pub(super) meta: Option<GameMeta>,
    /// Set once the finished game has been handed off for persistence.
    persisted: bool,
}

impl GameSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a command, mutating roster/game/history. Engine and validation
    /// errors propagate to the caller (surfaced to the client as `error`).
    pub fn apply(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::PlayerAdd { name } => {
                self.ensure_idle()?;
                self.roster.add(name);
            }
            Command::PlayerRename { id, name } => {
                self.ensure_idle()?;
                self.roster.rename(id, name)?;
            }
            Command::PlayerRemove { id } => {
                self.ensure_idle()?;
                self.roster.remove(id)?;
            }
            Command::PlayerReorder { id, index } => {
                self.ensure_idle()?;
                self.roster.reorder(id, index)?;
            }
            Command::GameNew { mode, x01, cricket } => self.new_game(mode, x01, cricket)?,
            Command::Throw { dart } => self.record_throw(dart)?,
            Command::UndoLast => {
                self.undo_last()?;
            }
            Command::EditDart {
                turn_index,
                dart_index,
                dart,
            } => self.edit_dart(turn_index, dart_index, dart)?,
            Command::EndTurn => self.end_turn()?,
            Command::Reset => self.reset(),
        }
        Ok(())
    }

    fn ensure_idle(&self) -> Result<()> {
        if self.game.is_some() {
            bail!("cannot edit players while a game is in progress");
        }
        Ok(())
    }

    fn new_game(
        &mut self,
        mode: GameMode,
        x01: Option<X01Config>,
        cricket: Option<super::protocol::CricketConfig>,
    ) -> Result<()> {
        let (game, meta_x01, cut_throat) = match mode {
            GameMode::X01 => {
                let cfg = x01.unwrap_or_default();
                let g = X01Game::new(cfg, &self.roster)?;
                (ActiveGame::X01(g), Some(cfg), None)
            }
            GameMode::Cricket => {
                let cut = cricket.map(|c| c.cut_throat).unwrap_or(false);
                let g = CricketGame::new(&self.roster, cut)?;
                (ActiveGame::Cricket(g), None, Some(cut))
            }
        };
        self.meta = Some(GameMeta::new(
            mode,
            meta_x01,
            cut_throat,
            Utc::now(),
            &self.roster,
        ));
        self.game = Some(game);
        self.persisted = false;
        Ok(())
    }

    /// An idle session that remembers a restored roster but has no active game.
    pub(super) fn idle_with_roster(roster: Roster) -> Self {
        Self {
            roster,
            ..Self::default()
        }
    }

    /// A session restored mid-game: roster, replayed engine, and rebuilt meta.
    pub(super) fn restored(roster: Roster, game: ActiveGame, meta: GameMeta) -> Self {
        Self {
            roster,
            game: Some(game),
            meta: Some(meta),
            persisted: false,
        }
    }

    /// Apply one dart through the engine, which records it in the leg history.
    fn record_throw(&mut self, dart: Score) -> Result<()> {
        self.game
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no active game"))?
            .throw(dart)
    }

    /// Remove the most recent dart anywhere in the leg (the engine reopens the
    /// previous turn if the current one already closed; works after a win).
    fn undo_last(&mut self) -> Result<Option<Score>> {
        self.game
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no active game"))?
            .undo_last()
    }

    /// Correct any dart in any turn; the engine replays the leg to recompute all
    /// state. `turn_index` indexes the engine's `turns()` (last = in-progress).
    fn edit_dart(&mut self, turn_index: usize, dart_index: usize, dart: Score) -> Result<()> {
        self.game
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no active game"))?
            .edit_dart(turn_index, dart_index, dart)
    }

    /// Force-advance to the next player by filling the current turn with misses
    /// (a miss scores nothing and never busts, so this cleanly "skips the rest").
    /// The engine auto-advances once the turn reaches [`DARTS_PER_TURN`] darts.
    fn end_turn(&mut self) -> Result<()> {
        let live_len = {
            let game = self
                .game
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("no active game"))?;
            if game.winner().is_some() {
                bail!("game already won");
            }
            game.live_turn_len()
        };
        for _ in live_len..DARTS_PER_TURN {
            // A win mid-fill ends the leg; stop rather than start a new turn.
            if self.game.as_ref().and_then(|g| g.winner()).is_some() {
                break;
            }
            self.record_throw(Score::miss())?;
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.game = None;
        self.meta = None;
        self.persisted = false;
    }

    /// Whether a game is currently active (used for restore logging).
    pub fn has_active_game(&self) -> bool {
        self.game.is_some()
    }

    /// Build the wire `GameState` from the current session.
    pub fn state(&self) -> GameState {
        let players: Vec<Player> = self.roster.players().to_vec();
        match &self.game {
            None => GameState {
                mode: "idle",
                players,
                current_player_id: None,
                current_turn: Vec::new(),
                turns: Vec::new(),
                winner_id: None,
                x01: None,
                cricket: None,
            },
            Some(ActiveGame::X01(g)) => {
                let remaining = self
                    .roster
                    .players()
                    .iter()
                    .filter_map(|p| g.remaining(p.id).map(|r| (p.id.to_string(), r)))
                    .collect();
                let cur = g.current_player();
                let checkout = g.remaining(cur).and_then(|r| g.suggest_checkout(r));
                let config = self.meta.as_ref().and_then(|m| m.x01).unwrap_or_default();
                GameState {
                    mode: "x01",
                    players,
                    current_player_id: Some(cur),
                    current_turn: g.current_turn().to_vec(),
                    turns: g.turns(),
                    winner_id: g.winner(),
                    x01: Some(X01State {
                        config,
                        remaining,
                        checkout,
                    }),
                    cricket: None,
                }
            }
            Some(ActiveGame::Cricket(g)) => {
                let mut marks = BTreeMap::new();
                let mut points = BTreeMap::new();
                for p in self.roster.players() {
                    let mut nums = BTreeMap::new();
                    for &n in NUMBERS.iter() {
                        if let Some(m) = g.marks(p.id, n) {
                            if m > 0 {
                                nums.insert(n.to_string(), m);
                            }
                        }
                    }
                    marks.insert(p.id.to_string(), nums);
                    if let Some(pts) = g.points(p.id) {
                        points.insert(p.id.to_string(), pts);
                    }
                }
                let cut_throat = self
                    .meta
                    .as_ref()
                    .and_then(|m| m.cut_throat)
                    .unwrap_or(false);
                GameState {
                    mode: "cricket",
                    players,
                    current_player_id: Some(g.current_player()),
                    current_turn: g.current_turn().to_vec(),
                    turns: g.turns(),
                    winner_id: g.winner(),
                    x01: None,
                    cricket: Some(CricketState {
                        cut_throat,
                        marks,
                        points,
                    }),
                }
            }
        }
    }

    /// If the game is freshly won, return its persistence snapshot exactly once.
    pub fn take_finished_if_new(&mut self) -> Option<NewGame> {
        if self.persisted {
            return None;
        }
        let snap = self.finished_snapshot()?;
        self.persisted = true;
        Some(snap)
    }

    fn finished_snapshot(&self) -> Option<NewGame> {
        let game = self.game.as_ref()?;
        let winner = game.winner()?;
        let meta = self.meta.as_ref()?;
        let winner_index = meta.order.iter().position(|&id| id == winner)? as i32;

        let players = meta
            .order
            .iter()
            .enumerate()
            .map(|(pos, &id)| NewPlayer {
                name: meta.names.get(pos).cloned().unwrap_or_default(),
                position: pos as i32,
                final_result: self.final_result(game, id),
            })
            .collect();

        // The engine is the single source of truth for the leg history.
        let mut darts = Vec::new();
        for (turn_no, turn) in game.turns().iter().enumerate() {
            let ppos = meta.order.iter().position(|&id| id == turn.player)? as i32;
            for (dart_no, s) in turn.darts.iter().enumerate() {
                darts.push(NewDart {
                    player_position: ppos,
                    turn_no: turn_no as i32,
                    dart_no: dart_no as i32,
                    ring: format!("{:?}", s.ring),
                    segment: s.segment as i16,
                    value: s.value as i32,
                });
            }
        }

        let (mode, config) = match meta.mode {
            GameMode::X01 => (
                "x01".to_string(),
                serde_json::to_value(meta.x01.unwrap_or_default())
                    .unwrap_or(serde_json::Value::Null),
            ),
            GameMode::Cricket => (
                "cricket".to_string(),
                serde_json::json!({ "cut_throat": meta.cut_throat.unwrap_or(false) }),
            ),
        };

        Some(NewGame {
            mode,
            config,
            cut_throat: meta.cut_throat,
            winner_index: Some(winner_index),
            started_at: meta.started_at,
            finished_at: Utc::now(),
            players,
            darts,
        })
    }

    fn final_result(&self, game: &ActiveGame, id: PlayerId) -> String {
        match game {
            ActiveGame::X01(g) => g.remaining(id).map(|r| r.to_string()).unwrap_or_default(),
            ActiveGame::Cricket(g) => g.points(id).map(|p| p.to_string()).unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::x01::{InRule, OutRule};
    use crate::web::protocol::CricketConfig;
    use serde_json::{json, Value};

    fn state_json(s: &GameSession) -> Value {
        serde_json::to_value(s.state()).expect("state serializes")
    }

    fn add_two(s: &mut GameSession) {
        s.apply(Command::PlayerAdd { name: "Ann".into() }).unwrap();
        s.apply(Command::PlayerAdd { name: "Bob".into() }).unwrap();
    }

    #[test]
    fn idle_state_shape() {
        let s = GameSession::new();
        assert_eq!(
            state_json(&s),
            json!({
                "mode": "idle",
                "players": [],
                "current_player_id": null,
                "current_turn": [],
                "turns": [],
                "winner_id": null
            })
        );
    }

    #[test]
    fn players_added_and_listed() {
        let mut s = GameSession::new();
        add_two(&mut s);
        let v = state_json(&s);
        assert_eq!(
            v["players"],
            json!([{ "id": 0, "name": "Ann" }, { "id": 1, "name": "Bob" }])
        );
    }

    #[test]
    fn x01_new_game_state_shape() {
        let mut s = GameSession::new();
        add_two(&mut s);
        s.apply(Command::GameNew {
            mode: GameMode::X01,
            x01: Some(X01Config {
                start: 501,
                in_rule: InRule::Straight,
                out_rule: OutRule::Double,
            }),
            cricket: None,
        })
        .unwrap();
        let v = state_json(&s);
        assert_eq!(v["mode"], "x01");
        assert_eq!(v["current_player_id"], 0);
        assert_eq!(v["x01"]["config"]["start"], 501);
        assert_eq!(v["x01"]["config"]["in_rule"], "Straight");
        assert_eq!(v["x01"]["config"]["out_rule"], "Double");
        assert_eq!(v["x01"]["remaining"]["0"], 501);
        assert_eq!(v["x01"]["remaining"]["1"], 501);
        assert!(v.get("cricket").is_none());
    }

    #[test]
    fn x01_throw_updates_remaining_and_turn() {
        let mut s = GameSession::new();
        add_two(&mut s);
        s.apply(Command::GameNew {
            mode: GameMode::X01,
            x01: None,
            cricket: None,
        })
        .unwrap();
        s.apply(Command::Throw {
            dart: Score::treble(20),
        })
        .unwrap();
        let v = state_json(&s);
        assert_eq!(v["x01"]["remaining"]["0"], 441);
        assert_eq!(
            v["current_turn"],
            json!([{ "ring": "Treble", "segment": 20, "value": 60 }])
        );
        // The in-progress turn is mirrored in the full leg history.
        assert_eq!(
            v["turns"],
            json!([{ "player": 0, "darts": [{ "ring": "Treble", "segment": 20, "value": 60 }] }])
        );
        // Checkout suggestion is present for the current player (a Vec or null).
        assert!(v["x01"].get("checkout").is_some());
    }

    #[test]
    fn undo_reverts_last_dart() {
        let mut s = GameSession::new();
        add_two(&mut s);
        s.apply(Command::GameNew {
            mode: GameMode::X01,
            x01: None,
            cricket: None,
        })
        .unwrap();
        s.apply(Command::Throw {
            dart: Score::treble(20),
        })
        .unwrap();
        s.apply(Command::Throw {
            dart: Score::treble(19),
        })
        .unwrap();
        s.apply(Command::UndoLast).unwrap();
        let v = state_json(&s);
        assert_eq!(v["x01"]["remaining"]["0"], 441);
        assert_eq!(v["current_turn"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn edit_dart_corrects_in_place() {
        let mut s = GameSession::new();
        add_two(&mut s);
        s.apply(Command::GameNew {
            mode: GameMode::X01,
            x01: None,
            cricket: None,
        })
        .unwrap();
        s.apply(Command::Throw {
            dart: Score::single(5),
        })
        .unwrap();
        s.apply(Command::Throw {
            dart: Score::single(7),
        })
        .unwrap();
        // Correct the first dart of the in-progress turn (5 -> T20).
        // Remaining = 501 - 60 - 7 = 434. The in-progress turn is turn 0.
        s.apply(Command::EditDart {
            turn_index: 0,
            dart_index: 0,
            dart: Score::treble(20),
        })
        .unwrap();
        let v = state_json(&s);
        assert_eq!(v["x01"]["remaining"]["0"], 434);
        assert_eq!(v["current_turn"][0]["value"], 60);
        assert_eq!(v["current_turn"][1]["value"], 7);
    }

    #[test]
    fn edit_previous_turn_recomputes_state() {
        let mut s = GameSession::new();
        add_two(&mut s);
        s.apply(Command::GameNew {
            mode: GameMode::X01,
            x01: None,
            cricket: None,
        })
        .unwrap();
        // Ann's full turn (3 darts) closes, advancing to Bob.
        s.apply(Command::Throw {
            dart: Score::single(5),
        })
        .unwrap();
        s.apply(Command::Throw {
            dart: Score::single(5),
        })
        .unwrap();
        s.apply(Command::Throw {
            dart: Score::single(5),
        })
        .unwrap();
        // Bob throws one dart in the now-current turn.
        s.apply(Command::Throw {
            dart: Score::single(1),
        })
        .unwrap();

        let before = state_json(&s);
        assert_eq!(before["current_player_id"], 1);
        assert_eq!(before["x01"]["remaining"]["0"], 486); // 501 - 15
        assert_eq!(before["turns"].as_array().unwrap().len(), 2);

        // Correct Ann's first dart (committed turn 0): S5 -> T20 (+55).
        s.apply(Command::EditDart {
            turn_index: 0,
            dart_index: 0,
            dart: Score::treble(20),
        })
        .unwrap();

        let v = state_json(&s);
        // Ann recomputed: 501 - 60 - 5 - 5 = 431.
        assert_eq!(v["x01"]["remaining"]["0"], 431);
        // Bob's turn and remaining are untouched; still his turn.
        assert_eq!(v["current_player_id"], 1);
        assert_eq!(v["x01"]["remaining"]["1"], 500);
        assert_eq!(v["turns"][0]["player"], 0);
        assert_eq!(v["turns"][0]["darts"][0]["value"], 60);
        assert_eq!(v["turns"][1]["player"], 1);
        assert_eq!(v["current_turn"][0]["value"], 1);
    }

    #[test]
    fn end_turn_advances_player() {
        let mut s = GameSession::new();
        add_two(&mut s);
        s.apply(Command::GameNew {
            mode: GameMode::X01,
            x01: None,
            cricket: None,
        })
        .unwrap();
        s.apply(Command::Throw {
            dart: Score::single(5),
        })
        .unwrap();
        s.apply(Command::EndTurn).unwrap();
        let v = state_json(&s);
        assert_eq!(v["current_player_id"], 1);
        assert_eq!(v["current_turn"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn roster_edits_rejected_mid_game() {
        let mut s = GameSession::new();
        add_two(&mut s);
        s.apply(Command::GameNew {
            mode: GameMode::X01,
            x01: None,
            cricket: None,
        })
        .unwrap();
        assert!(s.apply(Command::PlayerAdd { name: "Cy".into() }).is_err());
    }

    #[test]
    fn cricket_state_shape() {
        let mut s = GameSession::new();
        add_two(&mut s);
        s.apply(Command::GameNew {
            mode: GameMode::Cricket,
            x01: None,
            cricket: Some(CricketConfig { cut_throat: true }),
        })
        .unwrap();
        s.apply(Command::Throw {
            dart: Score::treble(20),
        })
        .unwrap();
        let v = state_json(&s);
        assert_eq!(v["mode"], "cricket");
        assert_eq!(v["cricket"]["cut_throat"], true);
        assert_eq!(v["cricket"]["marks"]["0"]["20"], 3);
        assert_eq!(v["cricket"]["points"]["0"], 0);
        assert!(v.get("x01").is_none());
    }

    #[test]
    fn x01_win_produces_snapshot_with_darts() {
        let mut s = GameSession::new();
        s.apply(Command::PlayerAdd { name: "Ann".into() }).unwrap();
        s.apply(Command::GameNew {
            mode: GameMode::X01,
            x01: Some(X01Config {
                start: 40,
                in_rule: InRule::Straight,
                out_rule: OutRule::Double,
            }),
            cricket: None,
        })
        .unwrap();
        s.apply(Command::Throw {
            dart: Score::double(20),
        })
        .unwrap();
        let v = state_json(&s);
        assert_eq!(v["winner_id"], 0);

        let snap = s.take_finished_if_new().expect("won game snapshots");
        assert_eq!(snap.mode, "x01");
        assert_eq!(snap.winner_index, Some(0));
        assert_eq!(snap.darts.len(), 1);
        assert_eq!(snap.darts[0].ring, "Double");
        assert_eq!(snap.darts[0].value, 40);
        // Snapshot is handed off only once.
        assert!(s.take_finished_if_new().is_none());
    }

    #[test]
    fn reset_returns_to_idle() {
        let mut s = GameSession::new();
        add_two(&mut s);
        s.apply(Command::GameNew {
            mode: GameMode::X01,
            x01: None,
            cricket: None,
        })
        .unwrap();
        s.apply(Command::Reset).unwrap();
        assert_eq!(state_json(&s)["mode"], "idle");
        // Roster survives a reset.
        assert_eq!(state_json(&s)["players"].as_array().unwrap().len(), 2);
    }
}
