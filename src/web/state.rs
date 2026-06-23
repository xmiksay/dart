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
}

impl AppState {
    pub fn new(db: DatabaseConnection) -> Self {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            session: Arc::new(Mutex::new(GameSession::new())),
            tx,
            db,
        }
    }
}
