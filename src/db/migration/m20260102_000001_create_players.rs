//! Persistent player roster: a `players` table plus a nullable FK from
//! `game_players` back to it, so finished-game records can be attributed to a
//! saved player for future per-player stats.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Players::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Players::Id)
                            .uuid()
                            .not_null()
                            .default(Expr::cust("gen_random_uuid()"))
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Players::Name).text().not_null().unique_key())
                    .col(
                        ColumnDef::new(Players::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(GamePlayers::Table)
                    .add_column(ColumnDef::new(GamePlayers::PlayerId).uuid().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_game_players_player")
                    .from(GamePlayers::Table, GamePlayers::PlayerId)
                    .to(Players::Table, Players::Id)
                    .on_delete(ForeignKeyAction::SetNull)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_game_players_player")
                    .table(GamePlayers::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(GamePlayers::Table)
                    .drop_column(GamePlayers::PlayerId)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(Players::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Players {
    Table,
    Id,
    Name,
    CreatedAt,
}

#[derive(DeriveIden)]
enum GamePlayers {
    Table,
    PlayerId,
}
