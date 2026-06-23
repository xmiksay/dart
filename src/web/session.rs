//! In-memory game session: a persistent roster plus an optional active game,
//! the authoritative turn/dart history (the engines keep only the live turn),
//! and the command handler that mutates both and builds wire state.

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

use crate::db::repository::{NewDart, NewGame, NewPlayer};
use crate::game::cricket::{CricketGame, NUMBERS};
use crate::game::x01::{X01Config, X01Game};
use crate::game::{Player, PlayerId, Roster};
use crate::geometry::Score;

use super::protocol::{Command, CricketState, GameMode, GameState, X01State};

/// The active game engine, behind a mode-agnostic facade.
pub enum ActiveGame {
    X01(X01Game),
    Cricket(CricketGame),
}

impl ActiveGame {
    fn current_player(&self) -> PlayerId {
        match self {
            ActiveGame::X01(g) => g.current_player(),
            ActiveGame::Cricket(g) => g.current_player(),
        }
    }

    fn current_turn(&self) -> &[Score] {
        match self {
            ActiveGame::X01(g) => g.current_turn(),
            ActiveGame::Cricket(g) => g.current_turn(),
        }
    }

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
}

/// A finalized turn: all darts belong to one player.
struct TurnRec {
    player_id: PlayerId,
    darts: Vec<Score>,
}

/// Snapshot of the config/order captured when a game starts (the engines do not
/// expose their config or remember roster order independently of play).
struct GameMeta {
    mode: GameMode,
    x01: Option<X01Config>,
    cut_throat: Option<bool>,
    started_at: DateTime<Utc>,
    order: Vec<PlayerId>,
    names: Vec<String>,
}

#[derive(Default)]
pub struct GameSession {
    roster: Roster,
    game: Option<ActiveGame>,
    meta: Option<GameMeta>,
    /// Completed turns of the current game.
    committed: Vec<TurnRec>,
    /// Darts of the in-progress turn (mirrors the engine's live turn).
    hist_cur: Vec<Score>,
    /// Player of the in-progress turn.
    hist_cur_player: Option<PlayerId>,
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
            Command::EditDart { index, dart } => self.edit_dart(index, dart)?,
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
        self.meta = Some(GameMeta {
            mode,
            x01: meta_x01,
            cut_throat,
            started_at: Utc::now(),
            order: self.roster.ids(),
            names: self
                .roster
                .players()
                .iter()
                .map(|p| p.name.clone())
                .collect(),
        });
        self.game = Some(game);
        self.committed.clear();
        self.hist_cur.clear();
        self.hist_cur_player = None;
        self.persisted = false;
        Ok(())
    }

    /// Apply one dart through the engine and record it in the history,
    /// finalizing the turn when the engine advances or the game is won.
    fn record_throw(&mut self, dart: Score) -> Result<()> {
        let Self {
            game,
            committed,
            hist_cur,
            hist_cur_player,
            ..
        } = self;
        let game = game
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no active game"))?;

        if hist_cur.is_empty() {
            *hist_cur_player = Some(game.current_player());
        }
        let player = hist_cur_player.expect("turn player set on the first dart of a turn");

        game.throw(dart)?;
        hist_cur.push(dart);

        // The engine clears its live turn on advance; a win keeps the darts but
        // ends the game. Either way the turn is now finalized.
        let won = game.winner().is_some();
        let advanced = game.current_turn().is_empty();
        if won || advanced {
            committed.push(TurnRec {
                player_id: player,
                darts: std::mem::take(hist_cur),
            });
            *hist_cur_player = None;
        }
        Ok(())
    }

    fn undo_last(&mut self) -> Result<Option<Score>> {
        let Self {
            game,
            hist_cur,
            hist_cur_player,
            ..
        } = self;
        let game = game
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no active game"))?;
        let removed = game.undo_last()?;
        if removed.is_some() {
            hist_cur.pop();
            if hist_cur.is_empty() {
                *hist_cur_player = None;
            }
        }
        Ok(removed)
    }

    /// Correct dart `index` of the in-progress turn: roll the turn back to that
    /// dart, replay the correction, then replay the darts that followed it.
    fn edit_dart(&mut self, index: usize, dart: Score) -> Result<()> {
        let turn = {
            let game = self
                .game
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("no active game"))?;
            game.current_turn().to_vec()
        };
        if index >= turn.len() {
            bail!("dart index {index} out of range for the current turn");
        }
        let tail: Vec<Score> = turn[index + 1..].to_vec();
        for _ in index..turn.len() {
            self.undo_last()?;
        }
        self.record_throw(dart)?;
        for d in tail {
            self.record_throw(d)?;
        }
        Ok(())
    }

    /// Force-advance to the next player by filling the turn with misses (a miss
    /// scores nothing and never busts, so this is a clean "skip the rest").
    fn end_turn(&mut self) -> Result<()> {
        {
            let game = self
                .game
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("no active game"))?;
            if game.winner().is_some() {
                bail!("game already won");
            }
        }
        let target = self.committed.len() + 1;
        while self.committed.len() < target {
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
        self.committed.clear();
        self.hist_cur.clear();
        self.hist_cur_player = None;
        self.persisted = false;
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

        let mut darts = Vec::new();
        for (turn_no, turn) in self.committed.iter().enumerate() {
            let ppos = meta.order.iter().position(|&id| id == turn.player_id)? as i32;
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
        // Correct the first dart (5 -> T20). Remaining = 501 - 60 - 7 = 434.
        s.apply(Command::EditDart {
            index: 0,
            dart: Score::treble(20),
        })
        .unwrap();
        let v = state_json(&s);
        assert_eq!(v["x01"]["remaining"]["0"], 434);
        assert_eq!(v["current_turn"][0]["value"], 60);
        assert_eq!(v["current_turn"][1]["value"], 7);
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
