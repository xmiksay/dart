//! Reconstruct a finished game's full `GameState` from its stored dart log, so
//! the SPA can render any historical game on the same round-grid scoreboard as a
//! live one. Reuses the replay-based [`GameSession::from_snapshot`]: the engines
//! are the single source of truth, so replaying the log reproduces all derived
//! state (remaining, marks, winner) exactly — we never store engine internals.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;

use crate::db::entities::dart;
use crate::db::repository::GameDetail;
use crate::game::x01::X01Config;
use crate::game::Roster;
use crate::geometry::Score;

use super::protocol::GameState;
use super::session::GameSession;
use super::snapshot::{GameSnapshot, SessionSnapshot};

/// Build a wire `GameState` for a finished game by replaying its stored darts.
pub fn reconstruct_state(detail: &GameDetail) -> Result<GameState> {
    // Roster in turn order; `add` assigns ids 0,1,2,… matching the stored
    // `position`/`winner_index`, so replayed state lines up with persistence.
    let mut roster = Roster::new();
    for p in &detail.players {
        roster.add(p.name.clone());
    }

    // Darts come back ordered by (turn_no, dart_no), i.e. true throw order.
    let darts = detail
        .darts
        .iter()
        .map(score_from_row)
        .collect::<Result<Vec<_>>>()?;

    let game = match detail.game.mode.as_str() {
        "x01" => {
            let config: X01Config = serde_json::from_value(detail.game.config.clone())
                .context("parsing stored X01 config")?;
            GameSnapshot::X01 { config, darts }
        }
        "cricket" => GameSnapshot::Cricket {
            cut_throat: detail.game.cut_throat.unwrap_or(false),
            darts,
        },
        other => return Err(anyhow!("unknown game mode {other:?}")),
    };

    let snapshot = SessionSnapshot {
        roster,
        game: Some(game),
        started_at: Some(detail.game.started_at.with_timezone(&Utc)),
    };
    Ok(GameSession::from_snapshot(snapshot)?.state())
}

/// Rebuild a [`Score`] from a stored dart row — the inverse of how the session
/// serializes darts (ring as its `Debug` name, plus segment/value).
fn score_from_row(d: &dart::Model) -> Result<Score> {
    serde_json::from_value(serde_json::json!({
        "ring": d.ring,
        "segment": d.segment,
        "value": d.value,
    }))
    .with_context(|| format!("parsing stored dart (ring {:?})", d.ring))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::{game, game_player};
    use crate::game::x01::{InRule, OutRule};

    fn now() -> chrono::DateTime<chrono::FixedOffset> {
        Utc::now().fixed_offset()
    }

    #[test]
    fn reconstructs_finished_x01_game() {
        let config = X01Config {
            start: 40,
            in_rule: InRule::Straight,
            out_rule: OutRule::Double,
        };
        let game_id = uuid::Uuid::new_v4();
        let detail = GameDetail {
            game: game::Model {
                id: game_id,
                mode: "x01".into(),
                config: serde_json::to_value(config).unwrap(),
                cut_throat: None,
                winner_index: Some(0),
                started_at: now(),
                finished_at: now(),
            },
            players: vec![game_player::Model {
                id: 1,
                game_id,
                name: "Ann".into(),
                position: 0,
                final_result: "0".into(),
                player_id: None,
            }],
            darts: vec![dart::Model {
                id: 1,
                game_id,
                player_position: 0,
                turn_no: 0,
                dart_no: 0,
                ring: "Double".into(),
                segment: 20,
                value: 40,
            }],
        };

        let state = reconstruct_state(&detail).expect("reconstructs");
        let v = serde_json::to_value(state).unwrap();
        assert_eq!(v["mode"], "x01");
        assert_eq!(v["winner_id"], 0);
        assert_eq!(v["x01"]["remaining"]["0"], 0);
        assert_eq!(v["turns"][0]["darts"][0]["value"], 40);
    }
}
