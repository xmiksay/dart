//! Web layer: Axum HTTP + WebSocket server pushing real-time game state to the
//! projector SPA, plus REST history endpoints. See `docs/protocol.md` for the
//! authoritative client/server message contract.

mod protocol;
mod routes;
mod session;
mod state;
mod static_files;
mod ws;

use anyhow::{Context, Result};

use state::AppState;

/// Run the web server (HTTP + WebSocket). Connection and bind address come from
/// the `DATABASE_URL` and `BIND_ADDR` environment variables.
pub async fn run() -> Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must be set to run the web server")?;
    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let db = crate::db::connect(&database_url).await?;
    crate::db::run_migrations(&db).await?;

    let app = routes::router(AppState::new(db));

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    tracing::info!("dartscore web server listening on {bind}");
    axum::serve(listener, app).await.context("serving HTTP")?;
    Ok(())
}
