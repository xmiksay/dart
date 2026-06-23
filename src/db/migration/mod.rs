//! SeaORM migrations (append-only). Add a new module + push it onto
//! [`Migrator::migrations`] for every schema change; never edit a shipped one.

use sea_orm_migration::prelude::*;

mod m20260101_000001_create_tables;
mod m20260102_000001_create_players;
mod m20260103_000001_create_live_session;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260101_000001_create_tables::Migration),
            Box::new(m20260102_000001_create_players::Migration),
            Box::new(m20260103_000001_create_live_session::Migration),
        ]
    }
}
