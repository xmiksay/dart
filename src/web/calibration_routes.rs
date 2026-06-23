//! Browser-calibration REST endpoints: camera listing/snapshot (gated behind the
//! `capture` feature) and load/save of the shared `calibration.json`.
//!
//! The wire format uses compact `[x, y]` pairs, NOT the internal `Point { x, y }`
//! serde shape — handlers convert at the boundary so the JSON contract stays
//! stable regardless of the geometry type. See `docs/protocol.md`,
//! "Camera calibration (browser, one-time)".

use std::path::Path;

use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::Json;
#[cfg(feature = "capture")]
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use super::routes::AppError;
use crate::calibration::Calibration;
use crate::Point;

/// Shared on-disk calibration file — the single source of truth used by the CLI
/// and the capture loop too, so the path is fixed rather than configurable.
const CALIBRATION_PATH: &str = "calibration.json";

#[derive(Deserialize)]
pub struct SnapshotQuery {
    // Read only by the capture build; the 501 stub ignores it but still parses
    // the query so the contract is identical across feature sets.
    #[cfg_attr(not(feature = "capture"), allow(dead_code))]
    #[serde(default)]
    camera: u32,
}

#[derive(Deserialize)]
pub struct CalibrationBody {
    /// Expected to be exactly four `[x, y]` pairs in canonical board order.
    points: Vec<[f64; 2]>,
}

/// `GET /api/cameras` — list connected cameras (capture build only).
#[cfg(feature = "capture")]
pub async fn list_cameras() -> Result<Json<serde_json::Value>, AppError> {
    let cameras = crate::capture::camera_list()
        // A camera-enumeration failure is environmental, not the client's fault.
        .map_err(|_| AppError::client(StatusCode::SERVICE_UNAVAILABLE, "could not list cameras"))?;
    let list: Vec<_> = cameras
        .into_iter()
        .map(|(index, name)| json!({ "index": index, "name": name }))
        .collect();
    Ok(Json(serde_json::Value::Array(list)))
}

/// `GET /api/cameras` — 501 stub when built without the `capture` feature.
#[cfg(not(feature = "capture"))]
pub async fn list_cameras() -> AppError {
    AppError::client(
        StatusCode::NOT_IMPLEMENTED,
        "camera support disabled; rebuild with --features capture",
    )
}

/// `GET /api/camera/snapshot?camera=N` — one PNG frame (capture build only).
#[cfg(feature = "capture")]
pub async fn camera_snapshot(Query(q): Query<SnapshotQuery>) -> Result<Response, AppError> {
    // A capture failure means the camera is busy or missing — surface a clean
    // 503 with a generic message; never leak the raw device/driver error.
    let png = crate::capture::snapshot_png(q.camera).map_err(|_| {
        AppError::client(
            StatusCode::SERVICE_UNAVAILABLE,
            "could not capture from camera (busy or unavailable)",
        )
    })?;
    Ok(([(axum::http::header::CONTENT_TYPE, "image/png")], png).into_response())
}

/// `GET /api/camera/snapshot` — 501 stub when built without the `capture` feature.
#[cfg(not(feature = "capture"))]
pub async fn camera_snapshot(Query(_q): Query<SnapshotQuery>) -> AppError {
    AppError::client(
        StatusCode::NOT_IMPLEMENTED,
        "camera support disabled; rebuild with --features capture",
    )
}

/// `GET /api/calibration` — the saved quad as `[[x,y]×4]`, or `null` if none.
pub async fn get_calibration() -> Result<Json<serde_json::Value>, AppError> {
    let path = Path::new(CALIBRATION_PATH);
    if !path.exists() {
        return Ok(Json(serde_json::Value::Null));
    }
    let cal = Calibration::load(path)?;
    Ok(Json(json!({ "points": points_to_pairs(&cal.points) })))
}

/// `POST /api/calibration` — validate, persist, and echo the mapped centre.
pub async fn post_calibration(
    Json(body): Json<CalibrationBody>,
) -> Result<Json<serde_json::Value>, AppError> {
    let points = pairs_to_points(&body.points)?;
    let cal = Calibration::new(points).map_err(|_| {
        AppError::client(
            StatusCode::BAD_REQUEST,
            "the four points do not form a valid board quad",
        )
    })?;
    cal.save(Path::new(CALIBRATION_PATH))?;

    // Map the four points' centroid through the homography: it should land near
    // the bull (0,0), giving the UI a quick sanity signal that the quad is sane.
    let centre = cal.to_board(centroid(&points))?;
    Ok(Json(json!({ "ok": true, "centre": [centre.x, centre.y] })))
}

/// Convert the canonical `[Point; 4]` into the wire `[[x,y]×4]` shape.
fn points_to_pairs(points: &[Point; 4]) -> Vec<[f64; 2]> {
    points.iter().map(|p| [p.x, p.y]).collect()
}

/// Convert exactly four `[x,y]` pairs into `[Point; 4]`; 400 otherwise.
fn pairs_to_points(pairs: &[[f64; 2]]) -> Result<[Point; 4], AppError> {
    let arr: [[f64; 2]; 4] = pairs.try_into().map_err(|_| {
        AppError::client(
            StatusCode::BAD_REQUEST,
            "points must be exactly four [x, y] pairs",
        )
    })?;
    Ok(arr.map(|[x, y]| Point::new(x, y)))
}

fn centroid(points: &[Point; 4]) -> Point {
    let n = points.len() as f64;
    let (sx, sy) = points
        .iter()
        .fold((0.0, 0.0), |(sx, sy), p| (sx + p.x, sy + p.y));
    Point::new(sx / n, sy / n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::canonical_calibration;

    #[test]
    fn pairs_round_trip_through_points() {
        let pairs = vec![[0.0, 1.0], [2.0, 3.0], [4.0, 5.0], [6.0, 7.0]];
        let points = pairs_to_points(&pairs).expect("four pairs convert");
        assert_eq!(points[2], Point::new(4.0, 5.0));
        assert_eq!(points_to_pairs(&points), pairs);
    }

    #[test]
    fn wrong_pair_count_is_rejected() {
        let three = vec![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]];
        assert!(pairs_to_points(&three).is_err());
    }

    /// POST→GET round trip against a real (unique) temp file, with cleanup.
    #[test]
    fn save_then_load_round_trip_on_disk() {
        let pairs: Vec<[f64; 2]> = canonical_calibration().iter().map(|p| [p.x, p.y]).collect();
        let points = pairs_to_points(&pairs).expect("canonical pairs convert");
        let cal = Calibration::new(points).expect("canonical quad is valid");

        let path =
            std::env::temp_dir().join(format!("dartscore_web_calib_{}.json", std::process::id()));
        cal.save(&path).expect("save calibration");

        let back = Calibration::load(&path).expect("load calibration");
        assert_eq!(points_to_pairs(&back.points), pairs);

        let _ = std::fs::remove_file(&path);
    }
}
