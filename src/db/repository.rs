//! Read/write helpers over the game-history entities. Inputs come from the live
//! [`crate::web`] session; outputs feed the REST history endpoints.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QueryOrder, Set, SqlErr, TransactionTrait,
};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

use super::entities::{dart, game, game_player, player};

/// A finished game ready to persist (built by the web session on a win).
pub struct NewGame {
    pub mode: String,
    pub config: serde_json::Value,
    pub cut_throat: Option<bool>,
    pub winner_index: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub players: Vec<NewPlayer>,
    pub darts: Vec<NewDart>,
}

pub struct NewPlayer {
    pub name: String,
    pub position: i32,
    pub final_result: String,
}

pub struct NewDart {
    pub player_position: i32,
    pub turn_no: i32,
    pub dart_no: i32,
    pub ring: String,
    pub segment: i16,
    pub value: i32,
}

/// Summary row for `GET /api/games`.
#[derive(Serialize)]
pub struct GameSummary {
    pub id: Uuid,
    pub mode: String,
    pub players: Vec<String>,
    pub winner: Option<String>,
    pub finished_at: chrono::DateTime<chrono::FixedOffset>,
}

/// Full game for `GET /api/games/{id}`.
#[derive(Serialize)]
pub struct GameDetail {
    #[serde(flatten)]
    pub game: game::Model,
    pub players: Vec<game_player::Model>,
    pub darts: Vec<dart::Model>,
}

/// Insert a finished game and all its players/darts in one transaction.
pub async fn save_finished_game(db: &DatabaseConnection, g: NewGame) -> Result<Uuid> {
    let txn = db.begin().await.context("opening transaction")?;
    let game_id = Uuid::new_v4();

    game::ActiveModel {
        id: Set(game_id),
        mode: Set(g.mode),
        config: Set(g.config),
        cut_throat: Set(g.cut_throat),
        winner_index: Set(g.winner_index),
        started_at: Set(g.started_at.into()),
        finished_at: Set(g.finished_at.into()),
    }
    .insert(&txn)
    .await
    .context("inserting game")?;

    for p in g.players {
        // Best-effort attribution: link to a saved player of the same name if one
        // exists, but never fail the save when there is no match.
        let player_id = player::Entity::find()
            .filter(player::Column::Name.eq(p.name.clone()))
            .one(&txn)
            .await
            .context("looking up saved player")?
            .map(|m| m.id);

        game_player::ActiveModel {
            id: NotSet,
            game_id: Set(game_id),
            name: Set(p.name),
            position: Set(p.position),
            final_result: Set(p.final_result),
            player_id: Set(player_id),
        }
        .insert(&txn)
        .await
        .context("inserting game player")?;
    }

    let darts: Vec<dart::ActiveModel> = g
        .darts
        .into_iter()
        .map(|d| dart::ActiveModel {
            id: NotSet,
            game_id: Set(game_id),
            player_position: Set(d.player_position),
            turn_no: Set(d.turn_no),
            dart_no: Set(d.dart_no),
            ring: Set(d.ring),
            segment: Set(d.segment),
            value: Set(d.value),
        })
        .collect();
    if !darts.is_empty() {
        dart::Entity::insert_many(darts)
            .exec(&txn)
            .await
            .context("inserting darts")?;
    }

    txn.commit().await.context("committing game")?;
    Ok(game_id)
}

/// List finished games (most recent first) with player names and the winner.
pub async fn list_games(db: &DatabaseConnection) -> Result<Vec<GameSummary>> {
    let games = game::Entity::find()
        .order_by_desc(game::Column::FinishedAt)
        .all(db)
        .await
        .context("loading games")?;
    let players = game_player::Entity::find()
        .order_by_asc(game_player::Column::Position)
        .all(db)
        .await
        .context("loading game players")?;

    let mut by_game: HashMap<Uuid, Vec<game_player::Model>> = HashMap::new();
    for p in players {
        by_game.entry(p.game_id).or_default().push(p);
    }

    let summaries = games
        .into_iter()
        .map(|g| {
            let roster = by_game.remove(&g.id).unwrap_or_default();
            let winner = g
                .winner_index
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| roster.get(i))
                .map(|p| p.name.clone());
            let names = roster.into_iter().map(|p| p.name).collect();
            GameSummary {
                id: g.id,
                mode: g.mode,
                players: names,
                winner,
                finished_at: g.finished_at,
            }
        })
        .collect();
    Ok(summaries)
}

/// Load one finished game with its players and darts, or `None` if absent.
pub async fn get_game(db: &DatabaseConnection, id: Uuid) -> Result<Option<GameDetail>> {
    let Some(game) = game::Entity::find_by_id(id)
        .one(db)
        .await
        .context("loading game")?
    else {
        return Ok(None);
    };

    let players = game_player::Entity::find()
        .filter(game_player::Column::GameId.eq(id))
        .order_by_asc(game_player::Column::Position)
        .all(db)
        .await
        .context("loading game players")?;

    let darts = dart::Entity::find()
        .filter(dart::Column::GameId.eq(id))
        .order_by_asc(dart::Column::TurnNo)
        .order_by_asc(dart::Column::DartNo)
        .all(db)
        .await
        .context("loading darts")?;

    Ok(Some(GameDetail {
        game,
        players,
        darts,
    }))
}

/// Failure modes of a player write that the REST layer maps to distinct status
/// codes. `Duplicate` (unique name) becomes 409; `Db` becomes a generic 500.
#[derive(Debug)]
pub enum PlayerError {
    Duplicate,
    Db(anyhow::Error),
}

/// Classify a DB error from a player write: a unique-constraint violation is a
/// duplicate name; anything else is an opaque internal error.
fn classify_player_err(err: DbErr, ctx: &'static str) -> PlayerError {
    match err.sql_err() {
        Some(SqlErr::UniqueConstraintViolation(_)) => PlayerError::Duplicate,
        _ => PlayerError::Db(anyhow::Error::new(err).context(ctx)),
    }
}

/// List saved players, alphabetically by name.
pub async fn list_players(db: &DatabaseConnection) -> Result<Vec<player::Model>> {
    player::Entity::find()
        .order_by_asc(player::Column::Name)
        .all(db)
        .await
        .context("loading players")
}

/// Create a saved player; `Err(Duplicate)` if the name is already taken.
pub async fn create_player(
    db: &DatabaseConnection,
    name: String,
) -> Result<player::Model, PlayerError> {
    player::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(name),
        created_at: NotSet,
    }
    .insert(db)
    .await
    .map_err(|e| classify_player_err(e, "inserting player"))
}

/// Rename a saved player. `Ok(None)` if no player has that id; `Err(Duplicate)`
/// if the new name collides with another player.
pub async fn rename_player(
    db: &DatabaseConnection,
    id: Uuid,
    name: String,
) -> Result<Option<player::Model>, PlayerError> {
    let existing = player::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| classify_player_err(e, "loading player"))?;
    let Some(model) = existing else {
        return Ok(None);
    };

    let mut active: player::ActiveModel = model.into();
    active.name = Set(name);
    active
        .update(db)
        .await
        .map(Some)
        .map_err(|e| classify_player_err(e, "updating player"))
}

/// Delete a saved player. Returns `true` if a row was removed (idempotent).
pub async fn delete_player(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
    let res = player::Entity::delete_by_id(id)
        .exec(db)
        .await
        .context("deleting player")?;
    Ok(res.rows_affected > 0)
}
