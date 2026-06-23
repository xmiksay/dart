//! WebSocket endpoint: per-client send loop, command intake, and the shared
//! command handler that mutates the session, persists wins, and broadcasts.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use super::protocol::{Command, ServerEvent};
use super::state::AppState;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // A single task owns the sink; everything else feeds it through `out_tx`.
    let (out_tx, mut out_rx) = mpsc::channel::<Message>(64);
    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Fan broadcast `state` events into this client's sink.
    let mut rx = state.tx.subscribe();
    let bcast_tx = out_tx.clone();
    let bcast = tokio::spawn(async move {
        while let Ok(json) = rx.recv().await {
            if bcast_tx.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Send the current state immediately on connect.
    if let Some(json) = current_state_json(&state).await {
        let _ = out_tx.send(Message::Text(json.into())).await;
    }

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                let reply = match serde_json::from_str::<Command>(text.as_str()) {
                    Ok(cmd) => match handle_command(&state, cmd).await {
                        Ok(()) => None,
                        Err(e) => Some(error_event(&e.to_string())),
                    },
                    Err(e) => Some(error_event(&format!("invalid command: {e}"))),
                };
                if let Some(json) = reply {
                    let _ = out_tx.send(Message::Text(json.into())).await;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    bcast.abort();
    drop(out_tx);
    let _ = writer.await;
}

/// Apply a command, persist a freshly won game, and broadcast the new state.
async fn handle_command(state: &AppState, cmd: Command) -> anyhow::Result<()> {
    let (state_json, finished) = {
        let mut session = state.session.lock().await;
        session.apply(cmd)?;
        let finished = session.take_finished_if_new();
        let json = serde_json::to_string(&ServerEvent::State {
            state: session.state(),
        })?;
        (json, finished)
    };

    if let Some(snapshot) = finished {
        // A persistence failure must not break live play; log and continue.
        if let Err(e) = crate::db::repository::save_finished_game(&state.db, snapshot).await {
            tracing::error!(error = ?e, "failed to persist finished game");
        }
    }

    let _ = state.tx.send(state_json);
    Ok(())
}

async fn current_state_json(state: &AppState) -> Option<String> {
    let session = state.session.lock().await;
    serde_json::to_string(&ServerEvent::State {
        state: session.state(),
    })
    .ok()
}

fn error_event(message: &str) -> String {
    serde_json::to_string(&ServerEvent::Error {
        message: message.to_string(),
    })
    .unwrap_or_else(|_| r#"{"type":"error","message":"internal error"}"#.to_string())
}
