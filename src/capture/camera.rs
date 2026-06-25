//! Live camera loop: grab frames, trigger on motion-settle, detect + score.

use super::settle::SettleDetector;
use crate::bgsub::DartLocator;
use crate::calibration::Calibration;
use crate::detect::Detector;
use crate::geometry::Score;
use crate::scoring::score_detection;
use anyhow::{Context, Result};
use image::{GrayImage, RgbImage};
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;
use std::path::Path;
use std::time::{Duration, Instant};

/// Side of the square blocks the frame is tiled into for localized motion.
const MOTION_BLOCK: u32 = 16;

/// Localized motion score (0–255): the mean absolute grayscale diff of the
/// single most-changed block. A landing dart lights up one block strongly even
/// though the whole-frame average stays near zero, while sensor noise — spread
/// thin across the frame — keeps every block low. Differing dimensions (a format
/// change) report no motion rather than a spike.
fn frame_motion(a: &GrayImage, b: &GrayImage) -> f32 {
    if a.dimensions() != b.dimensions() {
        return 0.0;
    }
    let (w, h) = a.dimensions();
    let (ra, rb) = (a.as_raw(), b.as_raw());
    let mut max_block = 0.0f32;
    let mut by = 0;
    while by < h {
        let y1 = (by + MOTION_BLOCK).min(h);
        let mut bx = 0;
        while bx < w {
            let x1 = (bx + MOTION_BLOCK).min(w);
            let mut sum: u64 = 0;
            let mut count: u64 = 0;
            for y in by..y1 {
                let row = (y * w) as usize;
                for x in bx..x1 {
                    let i = row + x as usize;
                    sum += (ra[i] as i32 - rb[i] as i32).unsigned_abs() as u64;
                    count += 1;
                }
            }
            if count > 0 {
                let m = sum as f32 / count as f32;
                if m > max_block {
                    max_block = m;
                }
            }
            bx += MOTION_BLOCK;
        }
        by += MOTION_BLOCK;
    }
    max_block
}

/// Enumerate cameras as `(index, name)` pairs (skips nodes that can't report an
/// index). Used by the CLI listing and the web `/api/cameras` endpoint.
pub fn camera_list() -> Result<Vec<(u32, String)>> {
    let cameras = nokhwa::query(nokhwa::utils::ApiBackend::Auto).context("querying cameras")?;
    Ok(cameras
        .iter()
        .filter_map(|c| c.index().as_index().ok().map(|i| (i, c.human_name())))
        .collect())
}

/// List the cameras the system exposes, with their index and name, so the
/// operator can pick one with `--camera <index>`.
pub fn list_cameras() -> Result<()> {
    let cameras = camera_list()?;
    if cameras.is_empty() {
        println!("no cameras found");
        return Ok(());
    }
    println!("available cameras:");
    for (index, name) in &cameras {
        println!("  [{index}] {name}");
    }
    Ok(())
}

/// Grab a single frame and save it, then exit — the quickest way to confirm a
/// camera index actually delivers video (vs. a metadata-only `/dev/video` node).
pub fn snapshot(camera_index: u32) -> Result<()> {
    std::fs::create_dir_all(CAPTURE_DIR).with_context(|| format!("creating {CAPTURE_DIR}/"))?;
    let mut camera = open_camera(camera_index)?;
    // Discard a few warm-up frames; the first frames can be black.
    let mut rgb = grab(&mut camera)?;
    for _ in 0..4 {
        rgb = grab(&mut camera)?;
    }
    let path = format!("{CAPTURE_DIR}/snapshot.png");
    rgb.save(&path).with_context(|| format!("saving {path}"))?;
    println!("saved {path} ({}x{})", rgb.width(), rgb.height());
    Ok(())
}

/// Grab a frame from a camera and return it PNG-encoded — used by the web
/// browser-calibration screen to show a still for the operator to click on.
pub fn snapshot_png(camera_index: u32) -> Result<Vec<u8>> {
    let mut camera = open_camera(camera_index)?;
    let mut rgb = grab(&mut camera)?;
    for _ in 0..4 {
        rgb = grab(&mut camera)?; // discard warm-up frames
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    rgb.write_to(&mut buf, image::ImageFormat::Png)
        .context("encoding snapshot PNG")?;
    Ok(buf.into_inner())
}

/// Apply v4l2 controls to `/dev/video<index>` via `v4l2-ctl`. Must run while the
/// device is open (this process holds the stream), because opening the camera
/// resets controls to auto — which is why setting them beforehand "has no
/// effect". Failures are logged and skipped (controls vary by camera).
fn apply_controls(index: u32, controls: &[String]) {
    if controls.is_empty() {
        return;
    }
    let device = format!("/dev/video{index}");
    for ctrl in controls {
        let result = std::process::Command::new("v4l2-ctl")
            .args(["-d", &device, "--set-ctrl", ctrl])
            .output();
        match result {
            Ok(o) if o.status.success() => tracing::info!("camera control set: {ctrl}"),
            Ok(o) => tracing::warn!(
                "v4l2-ctl --set-ctrl {ctrl} failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            ),
            Err(e) => {
                tracing::warn!(
                    "could not run v4l2-ctl ({e}); install v4l-utils to lock the camera"
                );
                return; // no point trying the rest
            }
        }
    }
}

/// Build the control list for a manual-exposure "lock" plus any extra `name=value`
/// controls. An explicit exposure value disables the camera's auto everything.
fn lock_controls(exposure: Option<u32>, extra: &[String]) -> Vec<String> {
    let mut controls = Vec::new();
    if let Some(exp) = exposure {
        controls.push("auto_exposure=1".into()); // 1 = Manual Mode (UVC)
        controls.push(format!("exposure_time_absolute={exp}"));
        controls.push("exposure_dynamic_framerate=0".into());
        controls.push("white_balance_automatic=0".into());
        controls.push("gain=0".into());
        controls.push("backlight_compensation=0".into());
    }
    controls.extend(extra.iter().cloned());
    controls
}

fn open_camera(index: u32) -> Result<Camera> {
    let requested =
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = Camera::new(CameraIndex::Index(index), requested)
        .context("opening camera (is it connected / not in use?)")?;
    camera.open_stream().context("starting camera stream")?;
    Ok(camera)
}

fn grab(camera: &mut Camera) -> Result<RgbImage> {
    let frame = camera.frame().context("capturing frame")?;
    frame
        .decode_image::<RgbFormat>()
        .context("decoding frame to RGB")
}

/// Where settled frames are written (capture-only mode + training collection).
const CAPTURE_DIR: &str = "captures";

/// Sample the static empty board to measure its noise floor: the peak localized
/// motion between consecutive frames, and the peak per-pixel diff vs the
/// reference. Thresholds are then set above these so noise doesn't trigger/score.
fn calibrate_noise(
    camera: &mut Camera,
    reference_gray: &GrayImage,
    frames: usize,
    blur: f32,
) -> Result<(f32, u8)> {
    // Motion (trigger) is measured on raw frames to match the live trigger;
    // pixel noise (bgsub cutoff) is measured post-blur to match detection.
    let ref_blur = if blur > 0.0 {
        image::imageops::blur(reference_gray, blur)
    } else {
        reference_gray.clone()
    };
    let mut prev = reference_gray.clone();
    let mut motion_floor = 0.0f32;
    let mut pixel_noise = 0u8;
    for _ in 0..frames {
        let gray = image::imageops::grayscale(&grab(camera)?);
        motion_floor = motion_floor.max(frame_motion(&prev, &gray));
        let fb = if blur > 0.0 {
            image::imageops::blur(&gray, blur)
        } else {
            gray.clone()
        };
        pixel_noise = pixel_noise.max(crate::bgsub::noise_ceiling(&ref_blur, &fb));
        prev = gray;
    }
    Ok((motion_floor, pixel_noise))
}

/// Capture loop: trigger on motion-settle, save the frame, and — when a trained
/// model is present — detect + score. Runs until interrupted (Ctrl-C).
///
/// With no `models/dartscore.safetensors` it runs in **capture-only** mode: the
/// camera + trigger still work and every settled frame is saved, so you can
/// verify framing/calibration markers, tune the trigger, and collect training
/// images before a model exists. Drop in weights and the same loop scores live.
#[allow(clippy::too_many_arguments)]
pub fn run(
    camera_index: u32,
    interval: Option<u64>,
    motion_on: Option<f32>,
    motion_off: Option<f32>,
    settle_frames: u32,
    bg_threshold: Option<u8>,
    bg_min_area: usize,
    bg_blur: f32,
    exposure: Option<u32>,
    extra_controls: Vec<String>,
    mut on_dart: impl FnMut(Score),
) -> Result<()> {
    let weights = Path::new("models/dartscore.safetensors");
    let detector = Detector::load(weights).ok();
    // Fall back to classical background-subtraction scoring when there's no model
    // but the camera is calibrated.
    let calibration = if detector.is_none() {
        Calibration::load(Path::new("calibration.json")).ok()
    } else {
        None
    };
    std::fs::create_dir_all(CAPTURE_DIR).with_context(|| format!("creating {CAPTURE_DIR}/"))?;

    let mut camera = open_camera(camera_index)?;

    // Lock exposure/white-balance now that we hold the stream (setting these
    // before opening has no effect — the open resets them to auto).
    apply_controls(camera_index, &lock_controls(exposure, &extra_controls));
    // Let the manual settings take hold before grabbing the reference.
    for _ in 0..5 {
        let _ = grab(&mut camera);
    }

    // Empty-board reference (used for bgsub scoring and noise calibration). Saved
    // so it can be diffed against saved throws for offline tuning (`detect-bg`).
    let ref_rgb = grab(&mut camera)?;
    let ref_path = format!("{CAPTURE_DIR}/reference.png");
    match ref_rgb.save(&ref_path) {
        Ok(()) => tracing::info!("saved empty-board reference -> {ref_path}"),
        Err(e) => tracing::warn!("failed to save {ref_path}: {e}"),
    }
    let reference_gray = image::imageops::grayscale(&ref_rgb);

    // Auto-measure the empty-board noise floor for any threshold not given on the
    // CLI, then set the trigger gates and bgsub cutoff above it.
    let need_auto = motion_on.is_none() || motion_off.is_none() || bg_threshold.is_none();
    let (motion_floor, pixel_noise) = if need_auto {
        tracing::info!("calibrating noise floor — keep the board EMPTY and still…");
        calibrate_noise(&mut camera, &reference_gray, 25, bg_blur)?
    } else {
        (0.0, 0)
    };
    let motion_on = motion_on.unwrap_or((motion_floor * 3.0).max(5.0));
    let mut motion_off = motion_off.unwrap_or((motion_floor * 1.5).max(2.0));
    if motion_off >= motion_on {
        motion_off = (motion_on - 1.0).max(1.0);
    }
    let resolved_bg = bg_threshold.unwrap_or((u16::from(pixel_noise) + 8).min(255) as u8);
    if need_auto {
        tracing::info!(
            "thresholds: motion_on={motion_on:.0} motion_off={motion_off:.0} \
             bg_threshold={resolved_bg} (noise floor: motion={motion_floor:.1} pixel={pixel_noise})"
        );
    }

    let mut locator = calibration.as_ref().map(|_| {
        let mut loc = DartLocator::new(reference_gray.clone());
        loc.threshold = resolved_bg;
        loc.min_area = bg_min_area;
        loc.blur = bg_blur;
        loc
    });

    match (&detector, &calibration) {
        (Some(_), _) => tracing::info!("detector loaded — live detect + score"),
        (None, Some(_)) => {
            tracing::info!("no model; background-subtraction scoring via calibration.json")
        }
        (None, None) => tracing::warn!(
            "no model and no calibration.json — capture-only mode (saving frames to {CAPTURE_DIR}/)"
        ),
    }

    let mut settle = SettleDetector::new(motion_on, motion_off, settle_frames);
    let mut prev: Option<GrayImage> = None;
    let mut last_tip_count = 0usize;
    let mut shot = 0usize;

    let interval = interval.map(Duration::from_secs);
    let mut last_interval_save = Instant::now();
    let mut last_motion_log = Instant::now();

    tracing::info!(
        "capture started on camera {camera_index} (trigger on>={motion_on:.0} off<={motion_off:.0} \
         settle={settle_frames}) — throw a dart (Ctrl-C to stop)"
    );
    loop {
        let rgb = grab(&mut camera)?;
        let gray = image::imageops::grayscale(&rgb);
        let motion = prev.as_ref().map(|p| frame_motion(p, &gray)).unwrap_or(0.0);

        // Surface the live localized motion score so the trigger can be tuned —
        // watch it during a real throw and set --motion-on just below the peak.
        if last_motion_log.elapsed() >= Duration::from_secs(2) {
            tracing::info!("motion score {motion:.2}");
            last_motion_log = Instant::now();
        }

        // Time-based saving for framing checks, independent of the trigger.
        if let Some(iv) = interval {
            if last_interval_save.elapsed() >= iv {
                shot += 1;
                save_frame(&rgb, shot);
                last_interval_save = Instant::now();
            }
        }

        if settle.update(motion) {
            shot += 1;
            save_frame(&rgb, shot);
            let darts = match (&detector, &calibration, &mut locator) {
                (Some(det), _, _) => score_frame(det, &rgb, &mut last_tip_count),
                (None, Some(cal), Some(loc)) => score_bgsub(loc, &gray, cal),
                _ => {
                    tracing::info!(
                        "capture-only: frame saved; calibrate or train a model to score"
                    );
                    Vec::new()
                }
            };
            // Emit each newly-scored dart to the sink (e.g. the web game).
            for dart in darts {
                on_dart(dart);
            }
        }
        prev = Some(gray);
    }
}

/// Score a settled frame by background subtraction against the calibration,
/// returning only newly-added darts (a dart falling out is a removal, not a throw).
fn score_bgsub(locator: &mut DartLocator, gray: &GrayImage, cal: &Calibration) -> Vec<Score> {
    match locator.update(gray, cal) {
        Ok(change) => {
            if change.removed > 0 {
                tracing::info!(
                    "{} dart(s) left the board (bounce-out / pulled)",
                    change.removed
                );
            }
            if change.added.is_empty() && change.removed == 0 {
                tracing::info!("settled, but no new dart found (background subtraction)");
            }
            change
                .added
                .into_iter()
                .map(|(tip, s)| {
                    println!(
                        "dart at ({:.0},{:.0}): {:?} {} -> {}",
                        tip.x, tip.y, s.ring, s.segment, s.value
                    );
                    s
                })
                .collect()
        }
        Err(e) => {
            tracing::warn!("background subtraction failed: {e:#}");
            Vec::new()
        }
    }
}

/// Save a frame to the capture dir, logging success/failure.
fn save_frame(rgb: &RgbImage, shot: usize) {
    let path = format!("{CAPTURE_DIR}/throw_{shot:04}.png");
    match rgb.save(&path) {
        Ok(()) => tracing::info!("saved frame -> {path}"),
        Err(e) => tracing::warn!("failed to save {path}: {e}"),
    }
}

/// Detect + score one settled frame, printing the darts and total.
fn score_frame(detector: &Detector, rgb: &RgbImage, last_tip_count: &mut usize) -> Vec<Score> {
    match detector.detect(rgb) {
        Ok(det) => {
            // Fewer tips than last trigger ⇒ darts were pulled: new round.
            if det.tips.len() < *last_tip_count {
                tracing::info!("darts removed — new round");
            }
            *last_tip_count = det.tips.len();
            match score_detection(&det) {
                Ok(scores) => {
                    let total: u32 = scores.iter().map(|s| s.value).sum();
                    for (i, s) in scores.iter().enumerate() {
                        println!("dart {}: {:?} {} -> {}", i + 1, s.ring, s.segment, s.value);
                    }
                    println!("frame total: {total}");
                    scores
                }
                Err(e) => {
                    tracing::warn!("scoring failed: {e:#}");
                    Vec::new()
                }
            }
        }
        // A blurred/occluded settle frame can miss calibration — keep going.
        Err(e) => {
            tracing::warn!("detection failed on settled frame: {e:#}");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;

    #[test]
    fn localized_change_beats_global_average() {
        let (w, h) = (128, 128);
        let base = GrayImage::from_pixel(w, h, Luma([0]));
        let mut dart = base.clone();
        // Brighten one 16x16 block (≈ a dart landing in a small patch).
        for y in 0..MOTION_BLOCK {
            for x in 0..MOTION_BLOCK {
                dart.put_pixel(x, y, Luma([200]));
            }
        }
        // Localized score sees the full block change; a whole-frame average of
        // the same change would be ~3 (200 * 256 / 16384) — below any useful gate.
        assert!(frame_motion(&base, &dart) > 100.0);
        assert_eq!(frame_motion(&base, &base), 0.0);
    }
}
