//! Shared application state: the single in-memory game session, a broadcast
//! channel fanning serialized `state` events to all WS clients, and the DB.

use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tokio::sync::{broadcast, Mutex};

use super::session::GameSession;

/// Capacity of the per-client broadcast lag buffer.
const BROADCAST_CAPACITY: usize = 64;

#[derive(Clone)]
pub struct AppState {
    pub session: Arc<Mutex<GameSession>>,
    pub tx: broadcast::Sender<String>,
    pub db: DatabaseConnection,
    /// Latest camera frame (PNG) published by the capture feed, so the browser
    /// calibration snapshot can reuse it instead of opening the busy camera.
    /// `None` until the feed runs.
    #[cfg(feature = "capture")]
    pub latest_frame: Arc<std::sync::Mutex<Option<Vec<u8>>>>,
}

impl AppState {
    /// Build state around a specific session (used to resume a restored game).
    pub fn with_session(db: DatabaseConnection, session: GameSession) -> Self {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            session: Arc::new(Mutex::new(session)),
            tx,
            db,
            #[cfg(feature = "capture")]
            latest_frame: Arc::new(std::sync::Mutex::new(None)),
        }
    }
}
