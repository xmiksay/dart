//! Singleton `live_session` table: one row holding the in-progress game as a
//! jsonb snapshot so a server restart can resume the live game. The
//! `id = 0` check constraint enforces at most one row (upserted on every change).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(LiveSession::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(LiveSession::Id)
                            .small_integer()
                            .not_null()
                            .primary_key()
                            .check(Expr::col(LiveSession::Id).eq(0)),
                    )
                    .col(
                        ColumnDef::new(LiveSession::Snapshot)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(LiveSession::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(LiveSession::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum LiveSession {
    Table,
    Id,
    Snapshot,
    UpdatedAt,
}
