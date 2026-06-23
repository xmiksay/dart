//! Durable snapshot of the live session: the roster plus, if a game is active,
//! its mode/config and the full ordered dart log. The engines are replay-based,
//! so replaying that log through a fresh engine reproduces all state exactly —
//! we never serialize engine internals, only the inputs that drive them.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::game::cricket::CricketGame;
use crate::game::x01::{X01Config, X01Game};
use crate::game::Roster;
use crate::geometry::Score;

use super::protocol::GameMode;
use super::session::{ActiveGame, GameMeta, GameSession};

/// Persisted form of an active game: mode-specific config plus the flat,
/// throw-ordered dart log to replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum GameSnapshot {
    X01 {
        config: X01Config,
        darts: Vec<Score>,
    },
    Cricket {
        cut_throat: bool,
        darts: Vec<Score>,
    },
}

/// Durable snapshot of the whole session. A `None` game restores an idle
/// session that still remembers the roster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub roster: Roster,
    pub game: Option<GameSnapshot>,
    /// Preserved so a restored game that then finishes records its true start
    /// time; absent for snapshots taken while idle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
}

impl GameSession {
    /// Capture the session into a durable, replay-able snapshot.
    pub fn snapshot(&self) -> SessionSnapshot {
        let game = self.game.as_ref().map(|g| {
            let darts: Vec<Score> = g.turns().into_iter().flat_map(|t| t.darts).collect();
            match g {
                ActiveGame::X01(_) => GameSnapshot::X01 {
                    // The session captured the config when it created the game.
                    config: self.meta.as_ref().and_then(|m| m.x01).unwrap_or_default(),
                    darts,
                },
                ActiveGame::Cricket(_) => GameSnapshot::Cricket {
                    cut_throat: self
                        .meta
                        .as_ref()
                        .and_then(|m| m.cut_throat)
                        .unwrap_or(false),
                    darts,
                },
            }
        });
        SessionSnapshot {
            roster: self.roster.clone(),
            game,
            started_at: self.meta.as_ref().map(|m| m.started_at),
        }
    }

    /// Rebuild a session from a snapshot: restore the roster, and if a game was
    /// active, recreate the engine and replay every dart in order to reconstruct
    /// all derived state exactly.
    pub fn from_snapshot(snap: SessionSnapshot) -> Result<Self> {
        let roster = snap.roster;
        let Some(game) = snap.game else {
            return Ok(Self::idle_with_roster(roster));
        };

        let started_at = snap.started_at.unwrap_or_else(Utc::now);
        let (active, meta) = match game {
            GameSnapshot::X01 { config, darts } => {
                let mut g = X01Game::new(config, &roster).context("rebuilding X01 game")?;
                for (i, &dart) in darts.iter().enumerate() {
                    g.throw(dart)
                        .with_context(|| format!("replaying X01 dart {i}"))?;
                }
                let meta = GameMeta::new(GameMode::X01, Some(config), None, started_at, &roster);
                (ActiveGame::X01(g), meta)
            }
            GameSnapshot::Cricket { cut_throat, darts } => {
                let mut g =
                    CricketGame::new(&roster, cut_throat).context("rebuilding cricket game")?;
                for (i, &dart) in darts.iter().enumerate() {
                    g.throw(dart)
                        .with_context(|| format!("replaying cricket dart {i}"))?;
                }
                let meta = GameMeta::new(
                    GameMode::Cricket,
                    None,
                    Some(cut_throat),
                    started_at,
                    &roster,
                );
                (ActiveGame::Cricket(g), meta)
            }
        };

        Ok(Self::restored(roster, active, meta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::x01::{InRule, OutRule};
    use crate::web::protocol::{Command, CricketConfig};
    use serde_json::Value;

    fn state_json(s: &GameSession) -> Value {
        serde_json::to_value(s.state()).expect("state serializes")
    }

    /// Snapshot then rebuild, asserting the wire state is byte-identical.
    fn assert_round_trip(s: &GameSession) -> GameSession {
        let snap = s.snapshot();
        // The snapshot itself must survive a JSON round-trip (the DB stores it
        // as jsonb), so go through serde_json::Value here too.
        let value = serde_json::to_value(&snap).expect("snapshot serializes");
        let snap: SessionSnapshot = serde_json::from_value(value).expect("snapshot deserializes");
        let restored = GameSession::from_snapshot(snap).expect("rebuild from snapshot");
        assert_eq!(state_json(&restored), state_json(s));
        restored
    }

    fn add_two(s: &mut GameSession) {
        s.apply(Command::PlayerAdd { name: "Ann".into() }).unwrap();
        s.apply(Command::PlayerAdd { name: "Bob".into() }).unwrap();
    }

    #[test]
    fn idle_session_round_trips() {
        let mut s = GameSession::new();
        add_two(&mut s);
        let restored = assert_round_trip(&s);
        assert_eq!(state_json(&restored)["mode"], "idle");
        assert_eq!(
            state_json(&restored)["players"].as_array().unwrap().len(),
            2
        );
    }

    #[test]
    fn x01_mid_game_round_trips() {
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
        // Several darts spanning a turn boundary, so current player advances.
        for dart in [
            Score::treble(20),
            Score::single(5),
            Score::single(1),
            Score::treble(19),
        ] {
            s.apply(Command::Throw { dart }).unwrap();
        }

        let v = state_json(&s);
        assert_eq!(v["current_player_id"], 1);
        assert_eq!(v["x01"]["remaining"]["0"], 435); // 501 - 60 - 5 - 1
        assert_eq!(v["x01"]["remaining"]["1"], 444); // 501 - 57
        assert_eq!(v["turns"].as_array().unwrap().len(), 2);

        assert_round_trip(&s);
    }

    #[test]
    fn cricket_mid_game_round_trips() {
        let mut s = GameSession::new();
        add_two(&mut s);
        s.apply(Command::GameNew {
            mode: GameMode::Cricket,
            x01: None,
            cricket: Some(CricketConfig { cut_throat: true }),
        })
        .unwrap();
        for dart in [Score::treble(20), Score::single(20), Score::treble(19)] {
            s.apply(Command::Throw { dart }).unwrap();
        }

        let v = state_json(&s);
        assert_eq!(v["cricket"]["cut_throat"], true);
        // Marks cap at 3 (a number is "closed" at 3); 20 is fully closed here.
        assert_eq!(v["cricket"]["marks"]["0"]["20"], 3);
        assert_eq!(v["cricket"]["marks"]["0"]["19"], 3);

        assert_round_trip(&s);
    }

    #[test]
    fn won_x01_game_round_trips_as_finished() {
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
        assert_eq!(state_json(&s)["winner_id"], 0);

        let restored = assert_round_trip(&s);
        // A restored finished game still reports its winner.
        assert_eq!(state_json(&restored)["winner_id"], 0);
    }
}
