//! Canonical dartboard geometry and polar score classification.
//!
//! All board coordinates are in millimetres with the origin at the bull, the
//! +x axis pointing right and the +y axis pointing up (segment 20 at top).
//! Radii follow the WDF/BDO regulation steel-tip spec, measured to the wires.

/// A 2D point in board (or image) space, in millimetres / pixels.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

// --- Radii (mm), measured to the spider wire ---
/// Inner bull ("double bull", scores 50).
pub const R_INNER_BULL: f64 = 6.35;
/// Outer bull (scores 25).
pub const R_OUTER_BULL: f64 = 15.9;
/// Treble ring inner edge.
pub const R_TREBLE_INNER: f64 = 99.0;
/// Treble ring outer edge.
pub const R_TREBLE_OUTER: f64 = 107.0;
/// Double ring inner edge.
pub const R_DOUBLE_INNER: f64 = 162.0;
/// Double ring outer edge — the scoring boundary; this is the homography unit.
pub const R_DOUBLE_OUTER: f64 = 170.0;

/// Number of segments and the angular width of each.
pub const SEGMENTS: usize = 20;
pub const SEGMENT_DEG: f64 = 360.0 / SEGMENTS as f64; // 18°

/// Segment numbers in clockwise order starting from 20 at the top.
/// Index = round(angle_cw_from_top / 18°) mod 20.
pub const NUMBERS: [u8; SEGMENTS] = [
    20, 1, 18, 4, 13, 6, 10, 15, 2, 17, 3, 19, 7, 16, 8, 11, 14, 9, 12, 5,
];

/// Which scoring ring a point falls in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Ring {
    InnerBull,
    OuterBull,
    Single,
    Treble,
    Double,
    Miss,
}

/// The classified result of a single dart in board space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Score {
    pub ring: Ring,
    /// Base segment number (1–20) for ring hits, 25 for any bull, 0 for a miss.
    pub segment: u8,
    /// Final point value.
    pub value: u32,
}

/// Resolve the segment number (1–20) for a point's angle.
///
/// Angle is measured clockwise from the +y axis (top). `round` snaps to the
/// nearest segment centre, which implicitly places the segment boundaries at
/// the 9° half-offsets — the detail that is easy to get wrong.
fn segment_for_angle(x: f64, y: f64) -> u8 {
    let mut deg = x.atan2(y).to_degrees(); // clockwise from +y
    if deg < 0.0 {
        deg += 360.0;
    }
    let idx = (deg / SEGMENT_DEG).round() as usize % SEGMENTS;
    NUMBERS[idx]
}

/// Classify a point in canonical board coordinates (mm, origin at bull).
pub fn classify(p: Point) -> Score {
    let r = (p.x * p.x + p.y * p.y).sqrt();

    if r <= R_INNER_BULL {
        return Score {
            ring: Ring::InnerBull,
            segment: 25,
            value: 50,
        };
    }
    if r <= R_OUTER_BULL {
        return Score {
            ring: Ring::OuterBull,
            segment: 25,
            value: 25,
        };
    }
    if r > R_DOUBLE_OUTER {
        return Score {
            ring: Ring::Miss,
            segment: 0,
            value: 0,
        };
    }

    let segment = segment_for_angle(p.x, p.y);
    let (ring, multiplier) = if (R_TREBLE_INNER..=R_TREBLE_OUTER).contains(&r) {
        (Ring::Treble, 3)
    } else if (R_DOUBLE_INNER..=R_DOUBLE_OUTER).contains(&r) {
        (Ring::Double, 2)
    } else {
        (Ring::Single, 1)
    };

    Score {
        ring,
        segment,
        value: segment as u32 * multiplier,
    }
}

/// The four canonical calibration points on the outer double edge (r = 170),
/// at the boundaries between segments 5&20 (top), 13&6 (right), 17&3 (bottom),
/// 8&11 (left). Order is fixed and must match the detector's calibration
/// classes. Each boundary sits at a 9° half-segment offset.
pub fn canonical_calibration() -> [Point; 4] {
    // Angles clockwise from top, in degrees.
    let angles_deg = [-9.0, 81.0, 171.0, 261.0];
    angles_deg.map(|a| {
        let t = a * std::f64::consts::PI / 180.0;
        Point::new(R_DOUBLE_OUTER * t.sin(), R_DOUBLE_OUTER * t.cos())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn bullseyes() {
        assert_eq!(classify(pt(0.0, 0.0)).value, 50);
        assert_eq!(classify(pt(0.0, 0.0)).ring, Ring::InnerBull);
        // Just inside outer bull.
        let s = classify(pt(0.0, 10.0));
        assert_eq!(s.ring, Ring::OuterBull);
        assert_eq!(s.value, 25);
    }

    #[test]
    fn cardinal_segments() {
        // Top = 20, right = 6, bottom = 3, left = 11 on a real board.
        assert_eq!(segment_for_angle(0.0, 50.0), 20);
        assert_eq!(segment_for_angle(50.0, 0.0), 6);
        assert_eq!(segment_for_angle(0.0, -50.0), 3);
        assert_eq!(segment_for_angle(-50.0, 0.0), 11);
    }

    #[test]
    fn single_double_treble() {
        // Straight up = segment 20.
        assert_eq!(
            classify(pt(0.0, 50.0)),
            Score {
                ring: Ring::Single,
                segment: 20,
                value: 20
            }
        );
        assert_eq!(
            classify(pt(0.0, 103.0)),
            Score {
                ring: Ring::Treble,
                segment: 20,
                value: 60
            }
        );
        assert_eq!(
            classify(pt(0.0, 166.0)),
            Score {
                ring: Ring::Double,
                segment: 20,
                value: 40
            }
        );
    }

    #[test]
    fn outside_is_miss() {
        let s = classify(pt(0.0, 200.0));
        assert_eq!(s.ring, Ring::Miss);
        assert_eq!(s.value, 0);
    }

    #[test]
    fn ring_boundaries_are_inclusive_edges() {
        // Exactly on the treble inner/outer edges counts as treble.
        assert_eq!(classify(pt(0.0, R_TREBLE_INNER)).ring, Ring::Treble);
        assert_eq!(classify(pt(0.0, R_TREBLE_OUTER)).ring, Ring::Treble);
        // Just inside the double outer edge is a double; just outside is a miss.
        assert_eq!(classify(pt(0.0, R_DOUBLE_OUTER)).ring, Ring::Double);
        assert_eq!(classify(pt(0.0, R_DOUBLE_OUTER + 0.01)).ring, Ring::Miss);
    }

    #[test]
    fn segment_seam_at_nine_degrees() {
        // The 20|1 boundary sits at +9°. Just before it is still 20, just after is 1.
        let r = 50.0;
        let before = 8.0_f64.to_radians();
        let after = 10.0_f64.to_radians();
        assert_eq!(segment_for_angle(r * before.sin(), r * before.cos()), 20);
        assert_eq!(segment_for_angle(r * after.sin(), r * after.cos()), 1);
    }

    #[test]
    fn canonical_points_lie_on_double_edge() {
        for p in canonical_calibration() {
            let r = (p.x * p.x + p.y * p.y).sqrt();
            assert!((r - R_DOUBLE_OUTER).abs() < 1e-9);
        }
    }
}
