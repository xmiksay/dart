//! Aggregate X01 statistics over rolling time windows (7/14/30/90 days), built
//! from finished games. Pure read side for `GET /api/stats`.
//!
//! Averages use the canonical 3-dart figure `scored / darts * 3`, where
//! `scored = start - remaining` (so busts are penalised correctly — their darts
//! count but credit nothing). Cricket games are excluded; these are X01 metrics.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, FixedOffset, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use uuid::Uuid;

use super::entities::{dart, game, game_player};

/// Rolling windows we report, in days (widest last — used to bound the query).
const WINDOWS: [i64; 4] = [7, 14, 30, 90];

/// A single dart worth exactly this many points is a treble 20 (the only way to
/// score 60 with one dart); counting these answers "number of 60s".
const SIXTY: i32 = 60;

#[derive(Serialize)]
pub struct StatsResponse {
    pub windows: Vec<WindowStats>,
}

/// All X01 stats for one rolling window.
#[derive(Serialize)]
pub struct WindowStats {
    pub days: i64,
    pub games_played: u32,
    /// Single darts worth 60 (treble 20) thrown across the window's games.
    pub sixties: u32,
    /// Saved player with the highest career 3-dart average in the window.
    pub best_average_player: Option<PlayerAvg>,
    /// Saved player with the highest win rate in the window.
    pub best_winrate_player: Option<PlayerWinRate>,
    /// Highest single-game 3-dart average by any player (saved or guest).
    pub best_game_average: Option<GameAvg>,
    /// X01 leg won in the fewest darts.
    pub fewest_darts_game: Option<GameLeg>,
}

#[derive(Serialize)]
pub struct PlayerAvg {
    pub name: String,
    pub average: f64,
    pub games: u32,
    pub darts: u32,
}

#[derive(Serialize)]
pub struct PlayerWinRate {
    pub name: String,
    pub win_rate: f64,
    pub wins: u32,
    pub games: u32,
}

#[derive(Serialize)]
pub struct GameAvg {
    pub game_id: Uuid,
    pub player: String,
    pub average: f64,
    pub start: i64,
    pub finished_at: DateTime<FixedOffset>,
}

#[derive(Serialize)]
pub struct GameLeg {
    pub game_id: Uuid,
    pub player: String,
    pub darts: u32,
    pub start: i64,
    pub finished_at: DateTime<FixedOffset>,
}

/// One player's performance in one game (the unit all metrics aggregate over).
struct PlayerPerf {
    name: String,
    player_id: Option<Uuid>,
    scored: i64,
    darts: u32,
    win: bool,
}

/// One finished X01 game with each player's performance and its 60-count.
struct GamePerf {
    game_id: Uuid,
    finished_at: DateTime<FixedOffset>,
    start: i64,
    sixties: u32,
    players: Vec<PlayerPerf>,
}

/// Running saved-player aggregate while folding over a window's games.
struct PlayerAgg {
    name: String,
    scored: i64,
    darts: u32,
    games: u32,
    wins: u32,
}

/// Compute all rolling-window X01 stats in a single pass over recent games.
pub async fn compute_stats(db: &DatabaseConnection) -> Result<StatsResponse> {
    let now = Utc::now();
    let widest = WINDOWS.iter().copied().max().unwrap_or(90);
    let cutoff = (now - Duration::days(widest)).fixed_offset();

    let games = game::Entity::find()
        .filter(game::Column::Mode.eq("x01"))
        .filter(game::Column::FinishedAt.gte(cutoff))
        .all(db)
        .await
        .context("loading games for stats")?;

    if games.is_empty() {
        let windows = WINDOWS.iter().map(|&d| window_stats(d, &[])).collect();
        return Ok(StatsResponse { windows });
    }

    let ids: Vec<Uuid> = games.iter().map(|g| g.id).collect();
    let players = game_player::Entity::find()
        .filter(game_player::Column::GameId.is_in(ids.clone()))
        .all(db)
        .await
        .context("loading game players for stats")?;
    let darts = dart::Entity::find()
        .filter(dart::Column::GameId.is_in(ids))
        .all(db)
        .await
        .context("loading darts for stats")?;

    let perfs = build_perfs(&games, &players, &darts);
    let windows = WINDOWS
        .iter()
        .map(|&days| {
            let cutoff = (now - Duration::days(days)).fixed_offset();
            let subset: Vec<&GamePerf> = perfs.iter().filter(|g| g.finished_at >= cutoff).collect();
            window_stats(days, &subset)
        })
        .collect();
    Ok(StatsResponse { windows })
}

/// Join games, players, and darts into per-game performance records.
fn build_perfs(
    games: &[game::Model],
    players: &[game_player::Model],
    darts: &[dart::Model],
) -> Vec<GamePerf> {
    let mut darts_by_gp: HashMap<(Uuid, i32), u32> = HashMap::new();
    let mut sixties_by_game: HashMap<Uuid, u32> = HashMap::new();
    for d in darts {
        *darts_by_gp
            .entry((d.game_id, d.player_position))
            .or_default() += 1;
        if d.value == SIXTY {
            *sixties_by_game.entry(d.game_id).or_default() += 1;
        }
    }
    let mut players_by_game: HashMap<Uuid, Vec<&game_player::Model>> = HashMap::new();
    for p in players {
        players_by_game.entry(p.game_id).or_default().push(p);
    }

    games
        .iter()
        .map(|g| {
            let start = g.config.get("start").and_then(|v| v.as_i64()).unwrap_or(0);
            let roster = players_by_game.remove(&g.id).unwrap_or_default();
            let players = roster
                .into_iter()
                .map(|p| {
                    let remaining: i64 = p.final_result.parse().unwrap_or(0);
                    PlayerPerf {
                        name: p.name.clone(),
                        player_id: p.player_id,
                        scored: (start - remaining).max(0),
                        darts: darts_by_gp.get(&(g.id, p.position)).copied().unwrap_or(0),
                        win: g.winner_index == Some(p.position),
                    }
                })
                .collect();
            GamePerf {
                game_id: g.id,
                finished_at: g.finished_at,
                start,
                sixties: sixties_by_game.get(&g.id).copied().unwrap_or(0),
                players,
            }
        })
        .collect()
}

/// Reduce a window's games into its headline stats.
fn window_stats(days: i64, games: &[&GamePerf]) -> WindowStats {
    let games_played = games.len() as u32;
    let sixties = games.iter().map(|g| g.sixties).sum();

    // Per saved-player career aggregate (guests have no player_id → excluded).
    let mut agg: HashMap<Uuid, PlayerAgg> = HashMap::new();
    for g in games {
        for p in &g.players {
            let Some(pid) = p.player_id else { continue };
            let e = agg.entry(pid).or_insert_with(|| PlayerAgg {
                name: p.name.clone(),
                scored: 0,
                darts: 0,
                games: 0,
                wins: 0,
            });
            e.name = p.name.clone();
            e.scored += p.scored;
            e.darts += p.darts;
            e.games += 1;
            if p.win {
                e.wins += 1;
            }
        }
    }

    let best_average_player = agg
        .values()
        .filter(|a| a.darts > 0)
        .max_by(|a, b| {
            three_dart(a.scored, a.darts)
                .partial_cmp(&three_dart(b.scored, b.darts))
                .unwrap_or(Ordering::Equal)
        })
        .map(|a| PlayerAvg {
            name: a.name.clone(),
            average: three_dart(a.scored, a.darts),
            games: a.games,
            darts: a.darts,
        });

    let best_winrate_player = agg
        .values()
        .filter(|a| a.games > 0)
        .max_by(|a, b| {
            let (wa, wb) = (
                a.wins as f64 / a.games as f64,
                b.wins as f64 / b.games as f64,
            );
            wa.partial_cmp(&wb)
                .unwrap_or(Ordering::Equal)
                .then(a.games.cmp(&b.games))
        })
        .map(|a| PlayerWinRate {
            name: a.name.clone(),
            win_rate: a.wins as f64 / a.games as f64,
            wins: a.wins,
            games: a.games,
        });

    let mut best_game_average: Option<GameAvg> = None;
    let mut fewest_darts_game: Option<GameLeg> = None;
    for g in games {
        for p in &g.players {
            if p.darts == 0 {
                continue;
            }
            let avg = three_dart(p.scored, p.darts);
            if best_game_average.as_ref().is_none_or(|b| avg > b.average) {
                best_game_average = Some(GameAvg {
                    game_id: g.game_id,
                    player: p.name.clone(),
                    average: avg,
                    start: g.start,
                    finished_at: g.finished_at,
                });
            }
            if p.win && fewest_darts_game.as_ref().is_none_or(|b| p.darts < b.darts) {
                fewest_darts_game = Some(GameLeg {
                    game_id: g.game_id,
                    player: p.name.clone(),
                    darts: p.darts,
                    start: g.start,
                    finished_at: g.finished_at,
                });
            }
        }
    }

    WindowStats {
        days,
        games_played,
        sixties,
        best_average_player,
        best_winrate_player,
        best_game_average,
        fewest_darts_game,
    }
}

/// The standard 3-dart average for `scored` points over `darts` darts.
fn three_dart(scored: i64, darts: u32) -> f64 {
    scored as f64 / darts as f64 * 3.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<FixedOffset> {
        Utc::now().fixed_offset()
    }

    fn perf(game_id: Uuid, start: i64, sixties: u32, players: Vec<PlayerPerf>) -> GamePerf {
        GamePerf {
            game_id,
            finished_at: now(),
            start,
            sixties,
            players,
        }
    }

    #[test]
    fn empty_window_is_all_none() {
        let w = window_stats(7, &[]);
        assert_eq!(w.games_played, 0);
        assert_eq!(w.sixties, 0);
        assert!(w.best_average_player.is_none());
        assert!(w.fewest_darts_game.is_none());
    }

    #[test]
    fn aggregates_saved_players_and_picks_best() {
        let saved = Uuid::new_v4();
        let other = Uuid::new_v4();
        // Ann (saved): wins a 501 leg in 15 darts -> avg 100.2.
        // Bob (saved): loses with 300 scored over 15 darts -> avg 60.
        let g = perf(
            Uuid::new_v4(),
            501,
            3,
            vec![
                PlayerPerf {
                    name: "Ann".into(),
                    player_id: Some(saved),
                    scored: 501,
                    darts: 15,
                    win: true,
                },
                PlayerPerf {
                    name: "Bob".into(),
                    player_id: Some(other),
                    scored: 300,
                    darts: 15,
                    win: false,
                },
            ],
        );
        let w = window_stats(30, &[&g]);
        assert_eq!(w.games_played, 1);
        assert_eq!(w.sixties, 3);

        let avg = w.best_average_player.unwrap();
        assert_eq!(avg.name, "Ann");
        assert!((avg.average - 100.2).abs() < 1e-6);

        let wr = w.best_winrate_player.unwrap();
        assert_eq!(wr.name, "Ann");
        assert!((wr.win_rate - 1.0).abs() < 1e-6);

        let best = w.best_game_average.unwrap();
        assert_eq!(best.player, "Ann");

        let fewest = w.fewest_darts_game.unwrap();
        assert_eq!(fewest.player, "Ann");
        assert_eq!(fewest.darts, 15);
    }

    #[test]
    fn guests_excluded_from_player_aggregates_but_not_best_game() {
        // A guest (no player_id) throws a blistering single-game average.
        let g = perf(
            Uuid::new_v4(),
            501,
            0,
            vec![PlayerPerf {
                name: "Guest".into(),
                player_id: None,
                scored: 501,
                darts: 9,
                win: true,
            }],
        );
        let w = window_stats(30, &[&g]);
        assert!(w.best_average_player.is_none(), "guest not a saved player");
        assert_eq!(w.best_game_average.unwrap().player, "Guest");
        assert_eq!(w.fewest_darts_game.unwrap().darts, 9);
    }
}
