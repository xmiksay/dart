//! `game_players` entity — players of a finished game, in turn order.

use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "game_players")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub game_id: Uuid,
    pub name: String,
    /// 0-based turn-order position.
    pub position: i32,
    /// Final standing (X01 remaining or cricket points, as text).
    pub final_result: String,
    /// Best-effort link to the persistent `players` row with a matching name at
    /// save time; `NULL` for ad-hoc names. `ON DELETE SET NULL` keeps history.
    pub player_id: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
