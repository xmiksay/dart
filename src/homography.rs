//! 4-point planar homography (Direct Linear Transform) used to map detected
//! image points onto the canonical board. Exactly four correspondences give a
//! unique transform, so no RANSAC is needed.

use crate::geometry::Point;
use anyhow::{anyhow, Result};
use nalgebra::{DMatrix, Matrix3, Vector3};

// Minimum gap between the smallest and next-smallest eigenvalue of AᵀA below
// which the calibration quad is treated as degenerate (near-collinear points).
const DEGENERACY_RATIO: f64 = 1e-9;

/// A 3×3 perspective transform mapping source (image) points to destination
/// (canonical board) points.
#[derive(Debug, Clone, Copy)]
pub struct Homography {
    m: Matrix3<f64>,
}

impl Homography {
    /// Solve the homography from four source→destination correspondences.
    pub fn from_correspondences(src: &[Point; 4], dst: &[Point; 4]) -> Result<Self> {
        // Build the 8×9 DLT system A·h = 0; h is the null-space vector.
        let mut a = DMatrix::<f64>::zeros(8, 9);
        for i in 0..4 {
            let (x, y) = (src[i].x, src[i].y);
            let (u, v) = (dst[i].x, dst[i].y);
            let r0 = 2 * i;
            let r1 = 2 * i + 1;
            a[(r0, 0)] = -x;
            a[(r0, 1)] = -y;
            a[(r0, 2)] = -1.0;
            a[(r0, 6)] = x * u;
            a[(r0, 7)] = y * u;
            a[(r0, 8)] = u;

            a[(r1, 3)] = -x;
            a[(r1, 4)] = -y;
            a[(r1, 5)] = -1.0;
            a[(r1, 6)] = x * v;
            a[(r1, 7)] = y * v;
            a[(r1, 8)] = v;
        }

        // h is the null-space of A. nalgebra's `svd` is thin (V^T would be 8×9,
        // dropping the null-space row for a wide matrix), so solve via the
        // smallest eigenvector of the 9×9 normal matrix AᵀA instead.
        let ata = a.transpose() * &a;
        let eig = ata.symmetric_eigen();

        // Eigenvalues are not guaranteed sorted; find the two smallest.
        let mut min_idx = 0usize;
        let mut min_val = f64::INFINITY;
        let mut second_min = f64::INFINITY;
        for (i, &lambda) in eig.eigenvalues.iter().enumerate() {
            if lambda < min_val {
                second_min = min_val;
                min_val = lambda;
                min_idx = i;
            } else if lambda < second_min {
                second_min = lambda;
            }
        }

        // A well-posed quad has a single near-zero eigenvalue; if the gap to the
        // next one collapses, the points are (near) degenerate.
        if second_min <= DEGENERACY_RATIO {
            return Err(anyhow!(
                "degenerate calibration points: homography is singular"
            ));
        }

        let h = eig.eigenvectors.column(min_idx);
        let m = Matrix3::new(
            h[0], h[1], h[2], //
            h[3], h[4], h[5], //
            h[6], h[7], h[8],
        );

        if !m.iter().all(|c| c.is_finite()) || m[(2, 2)].abs() < 1e-12 {
            return Err(anyhow!(
                "degenerate calibration points: homography is singular"
            ));
        }

        Ok(Self { m })
    }

    /// Apply the transform to a point, dividing out the homogeneous coordinate.
    pub fn apply(&self, p: Point) -> Point {
        let v = self.m * Vector3::new(p.x, p.y, 1.0);
        Point::new(v.x / v.z, v.y / v.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(a: [(f64, f64); 4]) -> [Point; 4] {
        a.map(|(x, y)| Point::new(x, y))
    }

    #[test]
    fn identity_maps_points_to_themselves() {
        let p = pts([(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let h = Homography::from_correspondences(&p, &p).unwrap();
        for q in p {
            let r = h.apply(q);
            assert!((r.x - q.x).abs() < 1e-6, "x: {} vs {}", r.x, q.x);
            assert!((r.y - q.y).abs() < 1e-6, "y: {} vs {}", r.y, q.y);
        }
    }

    #[test]
    fn recovers_affine_scale_and_translation() {
        let src = pts([(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]);
        // dst = src * 2 + (5, -3)
        let dst = pts([(5.0, -3.0), (205.0, -3.0), (205.0, 197.0), (5.0, 197.0)]);
        let h = Homography::from_correspondences(&src, &dst).unwrap();
        let mid = h.apply(Point::new(50.0, 50.0));
        assert!((mid.x - 105.0).abs() < 1e-6, "x = {}", mid.x);
        assert!((mid.y - 97.0).abs() < 1e-6, "y = {}", mid.y);
    }

    #[test]
    fn maps_correspondences_exactly() {
        // An arbitrary perspective-looking quad onto a square.
        let src = pts([(12.0, 9.0), (210.0, 20.0), (198.0, 230.0), (5.0, 205.0)]);
        let dst = pts([(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]);
        let h = Homography::from_correspondences(&src, &dst).unwrap();
        for i in 0..4 {
            let r = h.apply(src[i]);
            assert!((r.x - dst[i].x).abs() < 1e-6, "x[{i}] = {}", r.x);
            assert!((r.y - dst[i].y).abs() < 1e-6, "y[{i}] = {}", r.y);
        }
    }

    #[test]
    fn collinear_points_are_rejected() {
        let src = pts([(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]);
        let dst = pts([(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        // Degenerate src should not yield a usable transform.
        let res = Homography::from_correspondences(&src, &dst);
        // Either an explicit error or a non-finite result — both are acceptable
        // rejections; assert we don't silently produce a valid mapping.
        if let Ok(h) = res {
            let r = h.apply(Point::new(1.0, 1.0));
            assert!(!r.x.is_finite() || !r.y.is_finite() || r.x.abs() > 1e6);
        }
    }
}
