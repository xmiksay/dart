//! One-time manual calibration for a fixed camera.
//!
//! Instead of detecting the four board calibration points every frame, a fixed
//! camera can be calibrated once: mark the four outer-double-edge intersections
//! (5&20 top, 13&6 right, 17&3 bottom, 8&11 left) in a captured frame and save
//! their pixel coordinates. The homography to the canonical board is derived
//! from them and reused for every throw — the detector then only needs dart tips.

use crate::geometry::{canonical_calibration, classify, Point, Score};
use crate::homography::Homography;
use anyhow::{Context, Result};
use std::path::Path;

/// Saved calibration: the four board points in image pixels, canonical order.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Calibration {
    /// 5&20 (top), 13&6 (right), 17&3 (bottom), 8&11 (left).
    pub points: [Point; 4],
}

impl Calibration {
    /// Build from four marked points, validating they yield a usable homography.
    pub fn new(points: [Point; 4]) -> Result<Self> {
        let c = Self { points };
        c.homography()
            .context("the four marked points do not form a valid board quad")?;
        Ok(c)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading calibration {}", path.display()))?;
        serde_json::from_str(&raw).context("parsing calibration JSON")
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
            .with_context(|| format!("writing calibration {}", path.display()))?;
        Ok(())
    }

    /// Homography mapping image pixels onto the canonical board (mm).
    pub fn homography(&self) -> Result<Homography> {
        Homography::from_correspondences(&self.points, &canonical_calibration())
    }

    /// Map an image-pixel point onto the board, then to a board position (mm).
    pub fn to_board(&self, tip: Point) -> Result<Point> {
        Ok(self.homography()?.apply(tip))
    }

    /// Score a dart tip given in image pixels.
    pub fn score_tip(&self, tip: Point) -> Result<Score> {
        Ok(classify(self.to_board(tip)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Ring;

    /// With the marked points equal to the canonical board, the homography is the
    /// identity, so a board-mm tip classifies directly.
    #[test]
    fn identity_calibration_scores_tips() {
        let cal = Calibration::new(canonical_calibration()).unwrap();
        assert_eq!(cal.score_tip(Point::new(0.0, 0.0)).unwrap().value, 50);
        let t20 = cal.score_tip(Point::new(0.0, 103.0)).unwrap();
        assert_eq!(t20.ring, Ring::Treble);
        assert_eq!(t20.segment, 20);
    }

    #[test]
    fn degenerate_points_are_rejected() {
        let collinear = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(2.0, 2.0),
            Point::new(3.0, 3.0),
        ];
        assert!(Calibration::new(collinear).is_err());
    }

    #[test]
    fn save_load_round_trip() {
        let cal = Calibration::new(canonical_calibration()).unwrap();
        let path = std::env::temp_dir().join("dartscore_calib_test.json");
        cal.save(&path).unwrap();
        let back = Calibration::load(&path).unwrap();
        assert_eq!(back.points, cal.points);
        let _ = std::fs::remove_file(&path);
    }
}
