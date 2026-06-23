//! `live_session` entity — the single (`id = 0`) row holding the in-progress
//! game as a jsonb snapshot, upserted on every state change so a restart resumes.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "live_session")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i16,
    #[sea_orm(column_type = "JsonBinary")]
    pub snapshot: Json,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
