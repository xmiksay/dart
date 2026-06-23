//! Initial schema: normalized, queryable game history.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Games::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Games::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Games::Mode).text().not_null())
                    .col(ColumnDef::new(Games::Config).json_binary().not_null())
                    .col(ColumnDef::new(Games::CutThroat).boolean())
                    .col(ColumnDef::new(Games::WinnerIndex).integer())
                    .col(
                        ColumnDef::new(Games::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Games::FinishedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(GamePlayers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GamePlayers::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(GamePlayers::GameId).uuid().not_null())
                    .col(ColumnDef::new(GamePlayers::Name).text().not_null())
                    .col(ColumnDef::new(GamePlayers::Position).integer().not_null())
                    .col(ColumnDef::new(GamePlayers::FinalResult).text().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_game_players_game")
                            .from(GamePlayers::Table, GamePlayers::GameId)
                            .to(Games::Table, Games::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Darts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Darts::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Darts::GameId).uuid().not_null())
                    .col(ColumnDef::new(Darts::PlayerPosition).integer().not_null())
                    .col(ColumnDef::new(Darts::TurnNo).integer().not_null())
                    .col(ColumnDef::new(Darts::DartNo).integer().not_null())
                    .col(ColumnDef::new(Darts::Ring).text().not_null())
                    .col(ColumnDef::new(Darts::Segment).small_integer().not_null())
                    .col(ColumnDef::new(Darts::Value).integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_darts_game")
                            .from(Darts::Table, Darts::GameId)
                            .to(Games::Table, Games::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Darts::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(GamePlayers::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Games::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Games {
    Table,
    Id,
    Mode,
    Config,
    CutThroat,
    WinnerIndex,
    StartedAt,
    FinishedAt,
}

#[derive(DeriveIden)]
enum GamePlayers {
    Table,
    Id,
    GameId,
    Name,
    Position,
    FinalResult,
}

#[derive(DeriveIden)]
enum Darts {
    Table,
    Id,
    GameId,
    PlayerPosition,
    TurnNo,
    DartNo,
    Ring,
    Segment,
    Value,
}
