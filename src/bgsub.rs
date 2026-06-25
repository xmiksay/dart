//! Classical (no-ML) dart detection via background subtraction, for a fixed,
//! calibrated camera. Diff a settled frame against the previous settled frame to
//! isolate the newly-landed dart, find its blob, and estimate the tip — the
//! bridge that lets scoring work before a YOLO model is trained.
//!
//! Tip heuristic: the embedded tip points *into* the board while the shaft/flight
//! extend outward, so the blob pixel mapping to the smallest board radius is the
//! best tip estimate. It's imperfect (shadows, overlapping darts) — the web
//! manual-correction UI is the backstop — but works well for a fixed setup.

use crate::calibration::Calibration;
use crate::geometry::{Point, Ring, Score};
use anyhow::{anyhow, Result};
use image::GrayImage;

/// Diagnostic view of one frame's background subtraction (see [`DartLocator::analyze`]).
pub struct Analysis {
    /// Changed pixels vs the empty-board reference (drives the empty-board reset).
    pub changed: usize,
    /// Sizes of every connected blob, largest first (before `min_area` filtering).
    pub blob_sizes: Vec<usize>,
    /// Blobs that pass `min_area` — the dart candidates.
    pub kept: Vec<Vec<(u32, u32)>>,
}

/// Locates newly-landed darts by differencing successive settled frames against
/// an empty-board reference.
pub struct DartLocator {
    reference: GrayImage,
    last: GrayImage,
    /// Per-pixel grayscale diff above which a pixel counts as "changed".
    pub threshold: u8,
    /// Minimum connected-component area (px) to count as a dart, not noise.
    pub min_area: usize,
    /// At/below this changed-pixel count vs the reference, the board is "empty"
    /// (all darts pulled) — the round resets. Keep below a single dart's area.
    pub empty_area: usize,
    /// Mask dilation radius (px) applied before blob-finding, to merge a dart's
    /// fragments (split by the spider wires / its thin shape) into one blob.
    pub dilate: u32,
    /// Two tips within this many image pixels are the same dart (add/remove match).
    pub match_px: f64,
    /// Gaussian blur sigma applied before diffing — suppresses sensor/focus
    /// jitter (a noisy/auto-focus webcam) so only large changes (a dart) survive.
    pub blur: f32,
    /// Tips of darts currently on the board (vs the empty reference).
    current: Vec<Point>,
}

/// Gaussian-blur a frame (no-op when `sigma <= 0`) to low-pass away noise.
fn blurred(img: &GrayImage, sigma: f32) -> std::borrow::Cow<'_, GrayImage> {
    if sigma <= 0.0 {
        std::borrow::Cow::Borrowed(img)
    } else {
        std::borrow::Cow::Owned(image::imageops::blur(img, sigma))
    }
}

/// What changed on the board between two settled frames.
#[derive(Debug, Default)]
pub struct DartChanges {
    /// Darts that newly appeared — the ones to score.
    pub added: Vec<(Point, Score)>,
    /// Darts that disappeared (bounce-out / fell / pulled) — not scored.
    pub removed: usize,
}

impl DartLocator {
    /// Create from an empty-board reference frame.
    pub fn new(reference: GrayImage) -> Self {
        let last = reference.clone();
        Self {
            reference,
            last,
            threshold: 35,
            min_area: 120,
            empty_area: 60,
            dilate: 4,
            match_px: 40.0,
            blur: 0.0,
            current: Vec::new(),
        }
    }

    /// Update the board state against the empty reference and report the change:
    /// darts that *newly* appeared (to score) and how many disappeared (bounce-out
    /// / fell / pulled — not scored). This is what the live loop should call, so a
    /// dart falling out isn't mistaken for a new throw.
    pub fn update(&mut self, frame: &GrayImage, cal: &Calibration) -> Result<DartChanges> {
        if frame.dimensions() != self.reference.dimensions() {
            return Ok(DartChanges::default());
        }
        // Board empty again → all darts gone; reset the round.
        if changed_count(frame, &self.reference, self.threshold) <= self.empty_area {
            let removed = self.current.len();
            self.current.clear();
            self.reset();
            return Ok(DartChanges {
                added: Vec::new(),
                removed,
            });
        }

        // Current darts = blobs vs the empty reference (the true board state).
        let mut tips = Vec::new();
        for blob in self.detect_blobs(frame, &self.reference, self.min_area) {
            let tip = blob_tip(&blob, cal)?;
            if cal.score_tip(tip)?.ring != Ring::Miss {
                tips.push(tip);
            }
        }

        let added_pts: Vec<Point> = tips
            .iter()
            .copied()
            .filter(|t| !self.current.iter().any(|c| dist(c, t) < self.match_px))
            .collect();
        let removed = self
            .current
            .iter()
            .filter(|c| !tips.iter().any(|t| dist(c, t) < self.match_px))
            .count();

        self.current = tips;
        self.last = frame.clone();

        let added = added_pts
            .into_iter()
            .map(|t| cal.score_tip(t).map(|s| (t, s)))
            .collect::<Result<Vec<_>>>()?;
        Ok(DartChanges { added, removed })
    }

    /// Diff the frame vs `base`, dilate to merge fragments, and return blobs of
    /// at least `min_area`.
    fn detect_blobs(
        &self,
        frame: &GrayImage,
        base: &GrayImage,
        min_area: usize,
    ) -> Vec<Vec<(u32, u32)>> {
        let (w, h) = (frame.width(), frame.height());
        let f = blurred(frame, self.blur);
        let b = blurred(base, self.blur);
        let mask = diff_mask(&f, &b, self.threshold);
        let mask = dilate(&mask, w as usize, h as usize, self.dilate);
        connected_components(&mask, w, h, min_area)
    }

    /// Re-baseline to the empty board (call when darts are pulled).
    pub fn reset(&mut self) {
        self.last = self.reference.clone();
    }

    /// Detect darts added since the previous settled frame; one blob (changed
    /// pixel coords) per new dart. Resets the round when the board reads empty.
    pub fn locate_new(&mut self, frame: &GrayImage) -> Vec<Vec<(u32, u32)>> {
        if frame.dimensions() != self.reference.dimensions() {
            return Vec::new();
        }
        if changed_count(frame, &self.reference, self.threshold) <= self.empty_area {
            self.reset();
            return Vec::new();
        }
        let blobs = self.detect_blobs(frame, &self.last, self.min_area);
        self.last = frame.clone();
        blobs
    }

    /// Inspect a frame without mutating state — for tuning. Reports the changed
    /// pixels vs the reference, every blob's size (before the `min_area` filter),
    /// and the blobs that pass it.
    pub fn analyze(&self, frame: &GrayImage) -> Analysis {
        if frame.dimensions() != self.reference.dimensions() {
            return Analysis {
                changed: 0,
                blob_sizes: Vec::new(),
                kept: Vec::new(),
            };
        }
        let changed = changed_count(frame, &self.reference, self.threshold);
        let mut all = self.detect_blobs(frame, &self.last, 1);
        all.sort_by_key(|b| std::cmp::Reverse(b.len()));
        let blob_sizes = all.iter().map(|b| b.len()).collect();
        let kept = all
            .into_iter()
            .filter(|b| b.len() >= self.min_area)
            .collect();
        Analysis {
            changed,
            blob_sizes,
            kept,
        }
    }

    /// Render the (dilated) changed-pixel mask as a black/white image — for
    /// visually diagnosing what background subtraction is flagging.
    pub fn change_mask(&self, frame: &GrayImage) -> GrayImage {
        let (w, h) = (frame.width(), frame.height());
        if frame.dimensions() != self.reference.dimensions() {
            return GrayImage::new(w, h);
        }
        let f = blurred(frame, self.blur);
        let b = blurred(&self.last, self.blur);
        let mask = diff_mask(&f, &b, self.threshold);
        let mask = dilate(&mask, w as usize, h as usize, self.dilate);
        let buf: Vec<u8> = mask.iter().map(|&b| if b { 255 } else { 0 }).collect();
        GrayImage::from_raw(w, h, buf).unwrap_or_else(|| GrayImage::new(w, h))
    }

    /// Locate new darts and score each via the saved calibration: returns the
    /// estimated tip (image pixels) and its [`Score`].
    pub fn locate_and_score(
        &mut self,
        frame: &GrayImage,
        cal: &Calibration,
    ) -> Result<Vec<(Point, Score)>> {
        let mut out = Vec::new();
        for blob in self.locate_new(frame) {
            let tip = blob_tip(&blob, cal)?;
            let score = cal.score_tip(tip)?;
            // Off-board blobs are shadows/hands, not darts.
            if score.ring != Ring::Miss {
                out.push((tip, score));
            }
        }
        Ok(out)
    }
}

/// 99.5th-percentile illumination-compensated per-pixel difference between two
/// frames — the noise floor a real change must clear. Used to auto-set the bgsub
/// threshold from a pair of empty-board frames.
pub fn noise_ceiling(a: &GrayImage, b: &GrayImage) -> u8 {
    if a.dimensions() != b.dimensions() {
        return 0;
    }
    let delta = mean_u8(a) - mean_u8(b);
    let mut hist = [0u32; 256];
    for (x, y) in a.as_raw().iter().zip(b.as_raw()) {
        let d = (((*x as f32 - *y as f32) - delta).abs().round() as i32).clamp(0, 255) as usize;
        hist[d] += 1;
    }
    let total: u32 = hist.iter().sum();
    let cutoff = (total as f32 * 0.995) as u32;
    let mut acc = 0u32;
    for (d, &c) in hist.iter().enumerate() {
        acc += c;
        if acc >= cutoff {
            return d as u8;
        }
    }
    255
}

fn dist(a: &Point, b: &Point) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

fn mean_u8(img: &GrayImage) -> f32 {
    let raw = img.as_raw();
    if raw.is_empty() {
        0.0
    } else {
        raw.iter().map(|&v| v as u32).sum::<u32>() as f32 / raw.len() as f32
    }
}

/// Per-pixel change test that ignores a *uniform* brightness shift between the
/// frames (auto-exposure / lighting drift) by subtracting the global mean delta,
/// so background subtraction doesn't light up the whole board.
fn changed_count(a: &GrayImage, b: &GrayImage, thr: u8) -> usize {
    let delta = mean_u8(a) - mean_u8(b);
    let thr = thr as f32;
    a.as_raw()
        .iter()
        .zip(b.as_raw())
        .filter(|(x, y)| ((**x as f32 - **y as f32) - delta).abs() > thr)
        .count()
}

fn diff_mask(a: &GrayImage, b: &GrayImage, thr: u8) -> Vec<bool> {
    let delta = mean_u8(a) - mean_u8(b);
    let thr = thr as f32;
    a.as_raw()
        .iter()
        .zip(b.as_raw())
        .map(|(x, y)| ((*x as f32 - *y as f32) - delta).abs() > thr)
        .collect()
}

/// Binary dilation by Chebyshev radius `r` (separable: horizontal then vertical).
/// Merges nearby fragments — e.g. the pieces a wire splits a dart into.
fn dilate(mask: &[bool], w: usize, h: usize, r: u32) -> Vec<bool> {
    if r == 0 {
        return mask.to_vec();
    }
    let r = r as usize;
    let mut tmp = vec![false; mask.len()];
    for y in 0..h {
        let row = y * w;
        for x in 0..w {
            let lo = x.saturating_sub(r);
            let hi = (x + r).min(w - 1);
            tmp[row + x] = (lo..=hi).any(|k| mask[row + k]);
        }
    }
    let mut out = vec![false; mask.len()];
    for x in 0..w {
        for y in 0..h {
            let lo = y.saturating_sub(r);
            let hi = (y + r).min(h - 1);
            out[y * w + x] = (lo..=hi).any(|k| tmp[k * w + x]);
        }
    }
    out
}

/// 4-connectivity connected components over a boolean mask, keeping blobs whose
/// area is at least `min_area`.
fn connected_components(mask: &[bool], w: u32, h: u32, min_area: usize) -> Vec<Vec<(u32, u32)>> {
    let (w, h) = (w as usize, h as usize);
    let mut seen = vec![false; mask.len()];
    let mut blobs = Vec::new();
    let mut stack = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        stack.clear();
        stack.push(start);
        seen[start] = true;
        let mut pixels = Vec::new();
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            pixels.push((x as u32, y as u32));
            if x > 0 && mask[i - 1] && !seen[i - 1] {
                seen[i - 1] = true;
                stack.push(i - 1);
            }
            if x + 1 < w && mask[i + 1] && !seen[i + 1] {
                seen[i + 1] = true;
                stack.push(i + 1);
            }
            if y > 0 && mask[i - w] && !seen[i - w] {
                seen[i - w] = true;
                stack.push(i - w);
            }
            if y + 1 < h && mask[i + w] && !seen[i + w] {
                seen[i + w] = true;
                stack.push(i + w);
            }
        }
        if pixels.len() >= min_area {
            blobs.push(pixels);
        }
    }
    blobs
}

/// The blob pixel mapping to the smallest board radius — the tip estimate.
fn blob_tip(blob: &[(u32, u32)], cal: &Calibration) -> Result<Point> {
    let h = cal.homography()?;
    let mut best: Option<(f64, Point)> = None;
    for &(x, y) in blob {
        let p = Point::new(x as f64, y as f64);
        let b = h.apply(p);
        let r = (b.x * b.x + b.y * b.y).sqrt();
        if best.as_ref().is_none_or(|(br, _)| r < *br) {
            best = Some((r, p));
        }
    }
    best.map(|(_, p)| p).ok_or_else(|| anyhow!("empty blob"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{canonical_calibration, Ring};
    use image::Luma;

    fn locator(reference: GrayImage) -> DartLocator {
        let mut l = DartLocator::new(reference);
        l.threshold = 10;
        l.min_area = 10;
        l.empty_area = 5;
        l.dilate = 0; // test the raw algorithm without fragment-merging
        l
    }

    #[test]
    fn connected_components_separates_and_filters() {
        // 10x10 mask: one 3x3 block (area 9) and one isolated pixel (area 1).
        let (w, h) = (10u32, 10u32);
        let mut mask = vec![false; (w * h) as usize];
        for y in 0..3 {
            for x in 0..3 {
                mask[(y * w + x) as usize] = true;
            }
        }
        mask[(8 * w + 8) as usize] = true;
        let blobs = connected_components(&mask, w, h, 5);
        assert_eq!(blobs.len(), 1); // the singleton is below min_area
        assert_eq!(blobs[0].len(), 9);
    }

    #[test]
    fn locates_and_scores_a_dart_via_calibration() {
        // Identity calibration: image pixels == board mm.
        let cal = Calibration::new(canonical_calibration()).unwrap();
        let reference = GrayImage::from_pixel(220, 220, Luma([0]));
        let mut frame = reference.clone();
        // A "dart": a 3px-wide vertical streak from r=103 (treble 20) outward.
        for y in 103..160u32 {
            for x in 0..3u32 {
                frame.put_pixel(x, y, Luma([255]));
            }
        }
        let mut loc = locator(reference);
        let hits = loc.locate_and_score(&frame, &cal).unwrap();
        assert_eq!(hits.len(), 1);
        let (tip, score) = hits[0];
        assert!((tip.y - 103.0).abs() < 2.0, "tip y = {}", tip.y); // innermost point
        assert_eq!(score.ring, Ring::Treble);
        assert_eq!(score.segment, 20);
    }

    #[test]
    fn noise_ceiling_zero_for_identical_frames() {
        let img = GrayImage::from_pixel(40, 40, Luma([120]));
        assert_eq!(noise_ceiling(&img, &img), 0);
    }

    #[test]
    fn dilation_merges_nearby_fragments() {
        // Two pixels 3 apart: two components raw, one after dilating by 2.
        let (w, h) = (10u32, 1u32);
        let mut mask = vec![false; 10];
        mask[2] = true;
        mask[5] = true;
        assert_eq!(connected_components(&mask, w, h, 1).len(), 2);
        let merged = dilate(&mask, w as usize, h as usize, 2);
        assert_eq!(connected_components(&merged, w, h, 1).len(), 1);
    }

    #[test]
    fn update_scores_new_darts_and_ignores_fall_out() {
        let cal = Calibration::new(canonical_calibration()).unwrap();
        let reference = GrayImage::from_pixel(220, 220, Luma([0]));
        let mut loc = locator(reference.clone());

        // Frame A: one dart streak (treble-20 region).
        let mut frame_a = reference.clone();
        for y in 103..150u32 {
            for x in 0..3u32 {
                frame_a.put_pixel(x, y, Luma([255]));
            }
        }
        let a = loc.update(&frame_a, &cal).unwrap();
        assert_eq!(a.added.len(), 1);
        assert_eq!(a.removed, 0);

        // Frame B: same dart still there (no change) → nothing added/removed.
        let b = loc.update(&frame_a, &cal).unwrap();
        assert_eq!(b.added.len(), 0);
        assert_eq!(b.removed, 0);

        // Frame C: dart fell out (back to empty) → removed, not scored.
        let c = loc.update(&reference, &cal).unwrap();
        assert_eq!(c.added.len(), 0);
        assert_eq!(c.removed, 1);
    }

    #[test]
    fn empty_board_resets_and_finds_nothing() {
        let reference = GrayImage::from_pixel(50, 50, Luma([0]));
        let mut loc = locator(reference.clone());
        // A near-identical frame (no dart) reads as empty -> no hits.
        assert!(loc.locate_new(&reference).is_empty());
    }
}
