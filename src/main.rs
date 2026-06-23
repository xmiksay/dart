//! `dartscore` CLI entry point.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use dartscore::bgsub::DartLocator;
use dartscore::calibration::Calibration;
use dartscore::{score_detection, Detection, Point};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "dartscore",
    about = "Automatic steel-tip dart scoring",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Score a detection frame supplied as JSON (4 calibration points + tips).
    Score {
        /// Path to a JSON file matching the `Detection` schema.
        input: PathBuf,
    },
    /// Save a one-time manual calibration from four marked board points.
    Calibrate {
        /// The four points in image pixels, canonical order
        /// (5&20 top, 13&6 right, 17&3 bottom, 8&11 left), e.g.
        /// `--points "610,135 1144,392 666,865 136,329"`.
        #[arg(long)]
        points: String,
        /// Where to write the calibration.
        #[arg(long, default_value = "calibration.json")]
        out: PathBuf,
    },
    /// Score dart-tip pixels against a saved calibration (validate the board).
    ScoreTip {
        /// One or more tips in image pixels, e.g. `"640,360 949,360"`.
        #[arg(long)]
        points: String,
        /// Calibration file to use.
        #[arg(long, default_value = "calibration.json")]
        calibration: PathBuf,
    },
    /// Background-subtraction dart detection on two saved frames (tune offline):
    /// an empty-board reference and a frame with darts.
    DetectBg {
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        frame: PathBuf,
        #[arg(long, default_value = "calibration.json")]
        calibration: PathBuf,
        #[arg(long, default_value_t = 35)]
        threshold: u8,
        #[arg(long, default_value_t = 120)]
        min_area: usize,
        /// Mask dilation radius (px) to merge a dart's fragments into one blob.
        #[arg(long, default_value_t = 4)]
        dilate: u32,
    },
    /// Run YOLOv8 inference on an image (Phase 1; needs `--features detection`).
    Detect { image: PathBuf },
    /// Capture from the camera and score live (Phase 3).
    Capture {
        /// Camera index to use (run with `--list` to see them).
        #[arg(long, default_value_t = 0)]
        camera: u32,
        /// List available cameras and exit.
        #[arg(long)]
        list: bool,
        /// Grab a single frame, save it, and exit (quick camera check).
        #[arg(long)]
        snapshot: bool,
        /// Also save a frame every N seconds regardless of motion (framing check).
        #[arg(long, value_name = "SECS")]
        interval: Option<u64>,
        /// Localized motion (0–255) at/above which a throw is "moving"
        /// (omit to auto-calibrate from the empty-board noise floor).
        #[arg(long)]
        motion_on: Option<f32>,
        /// Localized motion at/below which the scene is "still" (omit to auto).
        #[arg(long)]
        motion_off: Option<f32>,
        /// Consecutive still frames required before the trigger fires.
        #[arg(long, default_value_t = 5)]
        settle_frames: u32,
        /// Background-subtraction per-pixel diff threshold (omit to auto-calibrate).
        #[arg(long)]
        bg_threshold: Option<u8>,
        /// Background-subtraction minimum dart blob area in pixels.
        #[arg(long, default_value_t = 120)]
        bg_min_area: usize,
        /// Lock the camera to manual exposure with this value (disables auto
        /// exposure/white-balance/gain). Set this so the background stays stable.
        #[arg(long)]
        exposure: Option<u32>,
        /// Extra v4l2 control(s) to set, e.g. `--ctrl gamma=120` (repeatable).
        #[arg(long = "ctrl")]
        ctrl: Vec<String>,
    },
    /// Serve the web UI + WebSocket game server (Phase 4).
    Serve,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Score { input } => cmd_score(&input),
        Command::Calibrate { points, out } => cmd_calibrate(&points, &out),
        Command::ScoreTip {
            points,
            calibration,
        } => cmd_score_tip(&points, &calibration),
        Command::DetectBg {
            reference,
            frame,
            calibration,
            threshold,
            min_area,
            dilate,
        } => cmd_detect_bg(
            &reference,
            &frame,
            &calibration,
            threshold,
            min_area,
            dilate,
        ),
        Command::Detect { image } => cmd_detect(&image),
        Command::Capture {
            camera,
            list,
            snapshot,
            interval,
            motion_on,
            motion_off,
            settle_frames,
            bg_threshold,
            bg_min_area,
            exposure,
            ctrl,
        } => cmd_capture(CaptureOpts {
            camera,
            list,
            snapshot,
            interval,
            motion_on,
            motion_off,
            settle_frames,
            bg_threshold,
            bg_min_area,
            exposure,
            ctrl,
        }),
        Command::Serve => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(dartscore::web::run())
        }
    }
}

#[cfg(feature = "detection")]
fn cmd_detect(image: &std::path::Path) -> Result<()> {
    dartscore::detect::run(image)
}

#[cfg(not(feature = "detection"))]
fn cmd_detect(_image: &std::path::Path) -> Result<()> {
    anyhow::bail!("detection is disabled; rebuild with `--features detection`")
}

/// Options for the `capture` subcommand (bundled to keep the signature sane).
#[cfg_attr(not(feature = "capture"), allow(dead_code))]
struct CaptureOpts {
    camera: u32,
    list: bool,
    snapshot: bool,
    interval: Option<u64>,
    motion_on: Option<f32>,
    motion_off: Option<f32>,
    settle_frames: u32,
    bg_threshold: Option<u8>,
    bg_min_area: usize,
    exposure: Option<u32>,
    ctrl: Vec<String>,
}

#[cfg(feature = "capture")]
fn cmd_capture(o: CaptureOpts) -> Result<()> {
    if o.list {
        dartscore::capture::list_cameras()
    } else if o.snapshot {
        dartscore::capture::snapshot(o.camera)
    } else {
        // CLI prints scores inside the loop; no extra sink needed.
        dartscore::capture::run(
            o.camera,
            o.interval,
            o.motion_on,
            o.motion_off,
            o.settle_frames,
            o.bg_threshold,
            o.bg_min_area,
            o.exposure,
            o.ctrl,
            |_dart| {},
        )
    }
}

#[cfg(not(feature = "capture"))]
fn cmd_capture(_o: CaptureOpts) -> Result<()> {
    anyhow::bail!("camera capture is disabled; rebuild with `--features capture`")
}

/// Parse `"x,y x,y ..."` into points (image pixels).
fn parse_points(s: &str) -> Result<Vec<Point>> {
    s.split_whitespace()
        .map(|pair| {
            let (x, y) = pair
                .split_once(',')
                .ok_or_else(|| anyhow!("bad point '{pair}', expected x,y"))?;
            Ok(Point::new(
                x.trim().parse().with_context(|| format!("x in '{pair}'"))?,
                y.trim().parse().with_context(|| format!("y in '{pair}'"))?,
            ))
        })
        .collect()
}

fn cmd_calibrate(points: &str, out: &Path) -> Result<()> {
    let pts = parse_points(points)?;
    let pts: [Point; 4] = pts
        .try_into()
        .map_err(|v: Vec<Point>| anyhow!("calibrate needs exactly 4 points, got {}", v.len()))?;
    let cal = Calibration::new(pts)?;
    cal.save(out)?;
    println!("saved calibration -> {} (4 points)", out.display());

    // Sanity check: the centroid of the four edge points should map near the bull.
    let c = pts.iter().fold(Point::new(0.0, 0.0), |a, p| {
        Point::new(a.x + p.x / 4.0, a.y + p.y / 4.0)
    });
    let board = cal.to_board(c)?;
    println!(
        "sanity: points' centre maps to board ({:.1}, {:.1}) mm — should be near the bull (0,0)",
        board.x, board.y
    );
    Ok(())
}

fn cmd_score_tip(points: &str, calibration: &Path) -> Result<()> {
    let cal = Calibration::load(calibration)?;
    for (i, p) in parse_points(points)?.iter().enumerate() {
        let s = cal.score_tip(*p)?;
        println!(
            "tip {} ({:.0},{:.0}): {:?} {} -> {}",
            i + 1,
            p.x,
            p.y,
            s.ring,
            s.segment,
            s.value
        );
    }
    Ok(())
}

fn cmd_detect_bg(
    reference: &Path,
    frame: &Path,
    calibration: &Path,
    threshold: u8,
    min_area: usize,
    dilate: u32,
) -> Result<()> {
    let ref_img = image::open(reference)
        .with_context(|| format!("opening reference {}", reference.display()))?
        .to_luma8();
    let frame_img = image::open(frame)
        .with_context(|| format!("opening frame {}", frame.display()))?
        .to_luma8();
    if ref_img.dimensions() != frame_img.dimensions() {
        anyhow::bail!("reference and frame have different sizes");
    }

    let mut loc = DartLocator::new(ref_img);
    loc.threshold = threshold;
    loc.min_area = min_area;
    loc.dilate = dilate;
    loc.empty_area = 1; // diagnostic: never suppress as "empty"

    let a = loc.analyze(&frame_img);
    println!(
        "changed pixels vs reference (threshold {threshold}): {}",
        a.changed
    );
    let top: Vec<usize> = a.blob_sizes.iter().take(8).copied().collect();
    println!("blob sizes, largest first: {top:?}");
    println!("dart candidates (>= min_area {min_area}): {}", a.kept.len());
    if a.kept.is_empty() {
        println!(
            "→ none. Lower --threshold (more sensitive) or --min-area, and make sure \
             the reference is an EMPTY board of the same scene."
        );
    }

    match Calibration::load(calibration) {
        Ok(cal) => {
            for (tip, s) in loc.locate_and_score(&frame_img, &cal)? {
                println!(
                    "dart at ({:.0},{:.0}): {:?} {} -> {}",
                    tip.x, tip.y, s.ring, s.segment, s.value
                );
            }
        }
        Err(_) => println!(
            "(no {} — run `calibrate` to score the tips)",
            calibration.display()
        ),
    }
    Ok(())
}

fn cmd_score(input: &Path) -> Result<()> {
    let raw = std::fs::read_to_string(input)
        .with_context(|| format!("reading detection file {}", input.display()))?;
    let det: Detection = serde_json::from_str(&raw).context("parsing detection JSON")?;
    let scores = score_detection(&det)?;

    let total: u32 = scores.iter().map(|s| s.value).sum();
    for (i, s) in scores.iter().enumerate() {
        println!("dart {}: {:?} {} -> {}", i + 1, s.ring, s.segment, s.value);
    }
    println!("total: {total}");
    Ok(())
}
