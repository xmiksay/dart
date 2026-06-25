//! Web layer: Axum HTTP + WebSocket server pushing real-time game state to the
//! projector SPA, plus REST history endpoints. See `docs/protocol.md` for the
//! authoritative client/server message contract.

mod calibration_routes;
#[cfg(feature = "capture")]
mod capture_feed;
mod history;
mod protocol;
mod routes;
mod session;
mod snapshot;
mod state;
mod static_files;
mod ws;

use anyhow::{Context, Result};

use session::GameSession;
use snapshot::SessionSnapshot;
use state::AppState;

/// Run the web server (HTTP + WebSocket). Connection and bind address come from
/// the `DATABASE_URL` and `BIND_ADDR` environment variables.
pub async fn run() -> Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must be set to run the web server")?;
    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let db = crate::db::connect(&database_url).await?;
    crate::db::run_migrations(&db).await?;

    let session = restore_session(&db).await?;
    let state = AppState::with_session(db, session);

    // Bridge the live camera into the running game (no-op unless DART_CAMERA is
    // set; a camera failure never stops the server).
    #[cfg(feature = "capture")]
    capture_feed::start(state.clone());

    let app = routes::router(state);

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    tracing::info!("dartscore web server listening on {bind}");
    axum::serve(listener, app).await.context("serving HTTP")?;
    Ok(())
}

/// Restore the live session from the DB if one was persisted, replaying its
/// dart log to reconstruct game state; otherwise start an idle session. A
/// corrupt or unreadable snapshot must not block startup — log and start idle.
async fn restore_session(db: &sea_orm::DatabaseConnection) -> Result<GameSession> {
    let Some(value) = crate::db::repository::load_live_session(db).await? else {
        tracing::info!("no live session to restore; starting idle");
        return Ok(GameSession::new());
    };

    let snapshot: SessionSnapshot = match serde_json::from_value(value) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = ?e, "stored live session is unreadable; starting idle");
            return Ok(GameSession::new());
        }
    };

    match GameSession::from_snapshot(snapshot) {
        Ok(session) => {
            tracing::info!(
                has_game = session.has_active_game(),
                "restored live session"
            );
            Ok(session)
        }
        Err(e) => {
            tracing::error!(error = ?e, "failed to rebuild live session; starting idle");
            Ok(GameSession::new())
        }
    }
}
