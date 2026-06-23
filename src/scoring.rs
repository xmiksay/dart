//! Ties detection output to scores: warp dart tips through the calibration
//! homography into canonical board space, then classify each by polar geometry.

use crate::geometry::{self, canonical_calibration, Point, Score};
use crate::homography::Homography;
use anyhow::Result;

/// A single detection frame: the four calibration points (in fixed top, right,
/// bottom, left order) plus the detected dart tips, all in image pixels.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Detection {
    /// Calibration points in canonical order: 5&20, 13&6, 17&3, 8&11.
    pub calibration: [Point; 4],
    /// Detected dart-tip points.
    pub tips: Vec<Point>,
}

/// Score every dart tip in a detection frame.
pub fn score_detection(det: &Detection) -> Result<Vec<Score>> {
    let h = Homography::from_correspondences(&det.calibration, &canonical_calibration())?;
    Ok(det
        .tips
        .iter()
        .map(|&tip| geometry::classify(h.apply(tip)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Ring;

    /// With calibration points equal to canonical (identity homography), tips
    /// in board coordinates classify directly.
    #[test]
    fn identity_calibration_scores_directly() {
        let det = Detection {
            calibration: canonical_calibration(),
            tips: vec![Point::new(0.0, 0.0), Point::new(0.0, 103.0)],
        };
        let scores = score_detection(&det).unwrap();
        assert_eq!(scores[0].value, 50); // bull
        assert_eq!(scores[1].ring, Ring::Treble);
        assert_eq!(scores[1].value, 60); // treble 20
    }

    /// A scaled + translated camera view must be corrected by the homography so
    /// the same physical dart yields the same score.
    #[test]
    fn perspective_view_is_corrected() {
        let scale = 3.0;
        let offset = Point::new(640.0, 360.0);
        let to_image = |p: Point| Point::new(p.x * scale + offset.x, -p.y * scale + offset.y);

        let calib_canon = canonical_calibration();
        let calibration = [
            to_image(calib_canon[0]),
            to_image(calib_canon[1]),
            to_image(calib_canon[2]),
            to_image(calib_canon[3]),
        ];
        // Treble-20 in board space -> its image position.
        let tip_board = Point::new(0.0, 103.0);
        let det = Detection {
            calibration,
            tips: vec![to_image(tip_board)],
        };

        let scores = score_detection(&det).unwrap();
        assert_eq!(scores[0].ring, Ring::Treble);
        assert_eq!(scores[0].segment, 20);
        assert_eq!(scores[0].value, 60);
    }
}
