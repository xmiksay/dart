//! `darts` entity — every scored dart of a finished game, in throw order.

use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "darts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub game_id: Uuid,
    pub player_position: i32,
    pub turn_no: i32,
    pub dart_no: i32,
    pub ring: String,
    pub segment: i16,
    pub value: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
