//! `games` entity — one row per finished game.

use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "games")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub mode: String,
    /// Mode-specific config (X01 rules or cricket flags), stored as jsonb.
    #[sea_orm(column_type = "JsonBinary")]
    pub config: Json,
    pub cut_throat: Option<bool>,
    /// Roster position (0-based) of the winner.
    pub winner_index: Option<i32>,
    pub started_at: DateTimeWithTimeZone,
    pub finished_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
