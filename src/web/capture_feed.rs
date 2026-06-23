//! Live camera feed: bridges the blocking `dartscore::capture::run` detection
//! loop into the async web game. Compiled only with the `capture` feature.
//!
//! Each detected dart becomes a `Throw` command applied through the exact same
//! `apply_command` path a WS client uses, so game logic, live-session
//! persistence, and the `state` broadcast all run unchanged. A missing or busy
//! camera logs an error and exits its thread; the web server keeps running.

use anyhow::Context;
use tokio::sync::mpsc;

use crate::geometry::Score;

use super::protocol::Command;
use super::state::AppState;

const DEFAULT_BG_MIN_AREA: usize = 120;

/// Capture loop configuration, read from the environment. Thresholds left as
/// `None` are auto-calibrated from the empty-board noise floor at startup.
struct CaptureConfig {
    camera_index: u32,
    exposure: Option<u32>,
    bg_threshold: Option<u8>,
    bg_min_area: usize,
}

/// Read capture config from the environment. Returns `None` (and starts no
/// camera) unless `DART_CAMERA` is set; an unparseable optional var falls back
/// to its default with a warning.
fn config_from_env() -> Option<CaptureConfig> {
    let camera_index = match std::env::var("DART_CAMERA") {
        Ok(raw) => match raw.trim().parse::<u32>() {
            Ok(idx) => idx,
            Err(e) => {
                tracing::error!(value = %raw, error = ?e, "DART_CAMERA is not a valid u32; camera disabled");
                return None;
            }
        },
        Err(_) => return None,
    };

    Some(CaptureConfig {
        camera_index,
        exposure: parse_opt_env("DART_EXPOSURE"),
        bg_threshold: parse_opt_env("DART_BG_THRESHOLD"),
        bg_min_area: parse_env_or("DART_BG_MIN_AREA", DEFAULT_BG_MIN_AREA),
    })
}

/// Parse an optional env var into `Some(T)`, or `None` if unset/unparseable.
fn parse_opt_env<T: std::str::FromStr>(key: &str) -> Option<T> {
    let raw = std::env::var(key).ok()?;
    match raw.trim().parse::<T>() {
        Ok(v) => Some(v),
        Err(_) => {
            tracing::warn!(var = key, value = %raw, "ignoring unparseable value");
            None
        }
    }
}

/// Parse an env var into `T`, falling back to `default` when unset/unparseable.
fn parse_env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    match std::env::var(key) {
        Ok(raw) => match raw.trim().parse::<T>() {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!(var = key, value = %raw, "unparseable value; using default");
                default
            }
        },
        Err(_) => default,
    }
}

/// Start the camera capture feed if `DART_CAMERA` is set. Spawns a blocking OS
/// thread for the detection loop and a tokio task that applies each detected
/// dart to the shared session. A no-op (with an info log) when unconfigured.
pub fn start(state: AppState) {
    let Some(cfg) = config_from_env() else {
        tracing::info!("DART_CAMERA not set; camera capture disabled");
        return;
    };

    tracing::info!(
        camera = cfg.camera_index,
        exposure = ?cfg.exposure,
        bg_threshold = ?cfg.bg_threshold,
        bg_min_area = cfg.bg_min_area,
        "starting camera capture feed"
    );

    // Darts flow thread -> channel -> async applier. Unbounded so the blocking
    // capture thread never awaits/blocks on a full queue.
    let (dart_tx, dart_rx) = mpsc::unbounded_channel::<Score>();

    spawn_capture_thread(cfg, dart_tx);
    tokio::spawn(consume_darts(state, dart_rx));
}

/// Spawn the blocking detection loop on a dedicated OS thread. The capture API
/// is synchronous, so it must never run on the async runtime.
fn spawn_capture_thread(cfg: CaptureConfig, dart_tx: mpsc::UnboundedSender<Score>) {
    std::thread::spawn(move || {
        // `send` on a closed channel only happens if the applier task is gone
        // (server shutdown); nothing to do but drop the dart.
        let on_dart = move |dart: Score| {
            if dart_tx.send(dart).is_err() {
                tracing::debug!("dart channel closed; dropping detected dart");
            }
        };

        let result = crate::capture::run(
            cfg.camera_index,
            None, // interval
            None, // motion_on (auto-calibrate)
            None, // motion_off (auto-calibrate)
            5,    // settle_frames
            cfg.bg_threshold,
            cfg.bg_min_area,
            cfg.exposure,
            Vec::new(), // extra controls
            on_dart,
        )
        .with_context(|| format!("camera capture loop (camera {})", cfg.camera_index));

        // A camera failure must never crash the server: log and let the thread
        // exit, leaving WS-driven play fully functional.
        if let Err(e) = result {
            tracing::error!(error = ?e, "camera capture loop stopped");
        }
    });
}

/// Receive detected darts and apply each as a `Throw` through the shared
/// command path. Drops darts that arrive while the session is idle.
async fn consume_darts(state: AppState, mut dart_rx: mpsc::UnboundedReceiver<Score>) {
    while let Some(dart) = dart_rx.recv().await {
        match super::ws::apply_command(&state, Command::Throw { dart }).await {
            Ok(()) => {}
            Err(e) => {
                // No active game is the expected idle case, not an error.
                tracing::debug!(error = %e, ?dart, "dropping detected dart");
            }
        }
    }
    tracing::info!("capture feed channel closed; dart applier stopped");
}
