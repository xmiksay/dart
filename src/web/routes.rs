//! HTTP router: REST history + players endpoints, the WS upgrade, and the
//! embedded SPA fallback (see [`super::static_files`]).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, patch};
use axum::Router;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use super::state::AppState;
use super::static_files::static_handler;
use super::ws::ws_handler;
use crate::db::repository::{self, PlayerError};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/health", get(health))
        .route("/api/games", get(list_games))
        .route("/api/games/{id}", get(get_game))
        .route("/api/players", get(list_players).post(create_player))
        .route(
            "/api/players/{id}",
            patch(rename_player).delete(delete_player),
        )
        .with_state(state)
        // Everything else (assets + client routes) is served from the embedded
        // SPA bundle; unmatched `/api`/`/ws` paths 404 inside the handler.
        .fallback(static_handler)
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn list_games(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let games = repository::list_games(&state.db).await?;
    Ok(Json(serde_json::to_value(games)?))
}

async fn get_game(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    match repository::get_game(&state.db, id).await? {
        Some(game) => Ok(Json(serde_json::to_value(game)?).into_response()),
        None => Err(AppError::client(StatusCode::NOT_FOUND, "game not found")),
    }
}

#[derive(Deserialize)]
struct PlayerBody {
    name: String,
}

async fn list_players(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let players = repository::list_players(&state.db).await?;
    Ok(Json(serde_json::to_value(players)?))
}

async fn create_player(
    State(state): State<AppState>,
    Json(body): Json<PlayerBody>,
) -> Result<Response, AppError> {
    let name = validated_name(body.name)?;
    match repository::create_player(&state.db, name).await {
        Ok(p) => Ok((StatusCode::CREATED, Json(serde_json::to_value(p)?)).into_response()),
        Err(PlayerError::Duplicate) => Err(AppError::client(
            StatusCode::CONFLICT,
            "a player with that name already exists",
        )),
        Err(PlayerError::Db(e)) => Err(e.into()),
    }
}

async fn rename_player(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<PlayerBody>,
) -> Result<Response, AppError> {
    let name = validated_name(body.name)?;
    match repository::rename_player(&state.db, id, name).await {
        Ok(Some(p)) => Ok(Json(serde_json::to_value(p)?).into_response()),
        Ok(None) => Err(AppError::client(StatusCode::NOT_FOUND, "player not found")),
        Err(PlayerError::Duplicate) => Err(AppError::client(
            StatusCode::CONFLICT,
            "a player with that name already exists",
        )),
        Err(PlayerError::Db(e)) => Err(e.into()),
    }
}

async fn delete_player(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    repository::delete_player(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Trim and reject empty player names before they reach the DB.
fn validated_name(name: String) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::client(
            StatusCode::BAD_REQUEST,
            "name must not be empty",
        ));
    }
    Ok(trimmed.to_string())
}

/// A request failure. Internal errors are logged in full and surfaced as a
/// generic 500; client errors carry an explicit status and safe public message.
struct AppError {
    status: StatusCode,
    public: String,
    internal: Option<anyhow::Error>,
}

impl AppError {
    fn client(status: StatusCode, message: &str) -> Self {
        Self {
            status,
            public: message.to_string(),
            internal: None,
        }
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            public: "internal server error".to_string(),
            internal: Some(err.into()),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if let Some(err) = &self.internal {
            tracing::error!(error = ?err, "request failed");
        }
        (self.status, Json(json!({ "error": self.public }))).into_response()
    }
}
