//! Persistence layer: SeaORM entities, migrations, and repository helpers for
//! storing game history in PostgreSQL.

pub mod entities;
pub mod migration;
pub mod repository;

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use sea_orm_migration::MigratorTrait;

/// Connect to PostgreSQL using `database_url` (e.g. `DATABASE_URL`).
pub async fn connect(database_url: &str) -> Result<DatabaseConnection> {
    sea_orm::Database::connect(database_url)
        .await
        .context("connecting to PostgreSQL")
}

/// Apply all pending migrations.
pub async fn run_migrations(db: &DatabaseConnection) -> Result<()> {
    migration::Migrator::up(db, None)
        .await
        .context("running migrations")
}
