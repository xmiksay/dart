//! candle YOLOv8 inference: image -> 4 calibration points + dart tips (Phase 1).
//!
//! Vendors the candle `yolo-v8` model graph (see `model.rs`) parameterized to
//! `nc = NUM_CLASSES`, loads weights from a local safetensors artifact, runs one
//! forward pass, decodes the `[1, 4+nc, anchors]` output, applies per-class NMS,
//! and extracts the four calibration corners + dart tips in original-image pixels.

mod model;

use crate::geometry::Point;
use crate::scoring::Detection;
use anyhow::{anyhow, Context, Result};
use candle_core::{DType, Device, IndexOp, Module, Tensor};
use candle_nn::VarBuilder;
use image::RgbImage;
use model::{Multiples, YoloV8};
use std::path::Path;

/// Frozen class layout — must match python/data.yaml and the calibration order
/// in src/geometry.rs. 0..=3 are calibration corners, 4 is the dart tip.
pub const NUM_CLASSES: usize = 5;
const CLASS_DART: usize = 4;

/// Inference input size (multiple of 32). For best parity, match the training
/// `imgsz`; the network is fully convolutional so any multiple of 32 works.
const IMG_SIZE: usize = 640;
const CONF_THRESHOLD: f32 = 0.25;
const NMS_THRESHOLD: f32 = 0.45;
/// Letterbox padding colour (Ultralytics default 114), normalized.
const PAD: f32 = 114.0 / 255.0;

struct Detected {
    cx: f32,
    cy: f32,
    w: f32,
    h: f32,
    conf: f32,
    class: usize,
}

/// How the image was letterboxed, so detections can be mapped back.
struct Letterbox {
    tensor: Tensor,
    scale: f32,
    pad_x: f32,
    pad_y: f32,
}

fn letterbox(img: &RgbImage, device: &Device) -> Result<Letterbox> {
    let (w0, h0) = img.dimensions();
    let scale = (IMG_SIZE as f32 / w0 as f32).min(IMG_SIZE as f32 / h0 as f32);
    let nw = (w0 as f32 * scale).round() as u32;
    let nh = (h0 as f32 * scale).round() as u32;
    let resized = image::imageops::resize(img, nw, nh, image::imageops::FilterType::Triangle);
    let pad_x = (IMG_SIZE as u32 - nw) / 2;
    let pad_y = (IMG_SIZE as u32 - nh) / 2;

    // CHW, RGB, normalized; padding region stays at PAD.
    let mut data = vec![PAD; 3 * IMG_SIZE * IMG_SIZE];
    let plane = IMG_SIZE * IMG_SIZE;
    for y in 0..nh {
        for x in 0..nw {
            let px = resized.get_pixel(x, y);
            let dx = (x + pad_x) as usize;
            let dy = (y + pad_y) as usize;
            let off = dy * IMG_SIZE + dx;
            for c in 0..3 {
                data[c * plane + off] = px[c] as f32 / 255.0;
            }
        }
    }
    let tensor = Tensor::from_vec(data, (1, 3, IMG_SIZE, IMG_SIZE), device)?;
    Ok(Letterbox {
        tensor,
        scale,
        pad_x: pad_x as f32,
        pad_y: pad_y as f32,
    })
}

fn iou(a: &Detected, b: &Detected) -> f32 {
    let (ax1, ay1, ax2, ay2) = (
        a.cx - a.w / 2.0,
        a.cy - a.h / 2.0,
        a.cx + a.w / 2.0,
        a.cy + a.h / 2.0,
    );
    let (bx1, by1, bx2, by2) = (
        b.cx - b.w / 2.0,
        b.cy - b.h / 2.0,
        b.cx + b.w / 2.0,
        b.cy + b.h / 2.0,
    );
    let ix = (ax2.min(bx2) - ax1.max(bx1)).max(0.0);
    let iy = (ay2.min(by2) - ay1.max(by1)).max(0.0);
    let inter = ix * iy;
    let union = a.w * a.h + b.w * b.h - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// Greedy per-class non-maximum suppression.
fn nms(mut dets: Vec<Detected>) -> Vec<Detected> {
    dets.sort_by(|a, b| b.conf.total_cmp(&a.conf));
    let mut keep: Vec<Detected> = Vec::new();
    'outer: for d in dets {
        for k in &keep {
            if k.class == d.class && iou(k, &d) > NMS_THRESHOLD {
                continue 'outer;
            }
        }
        keep.push(d);
    }
    keep
}

fn decode(pred: &Tensor) -> Result<Vec<Detected>> {
    // pred: [1, 4+nc, anchors] -> rows of [4+nc].
    let pred = pred.i(0)?.t()?.contiguous()?;
    let rows: Vec<Vec<f32>> = pred.to_vec2()?;
    let mut out = Vec::new();
    for row in rows {
        let (mut best, mut best_c) = (0.0f32, 0usize);
        for (c, &s) in row[4..4 + NUM_CLASSES].iter().enumerate() {
            if s > best {
                best = s;
                best_c = c;
            }
        }
        if best >= CONF_THRESHOLD {
            out.push(Detected {
                cx: row[0],
                cy: row[1],
                w: row[2],
                h: row[3],
                conf: best,
                class: best_c,
            });
        }
    }
    Ok(out)
}

/// A loaded YOLOv8n detector. Build once and reuse across frames (the live
/// capture loop must not rebuild the graph or re-mmap weights per frame).
pub struct Detector {
    model: YoloV8,
    device: Device,
}

impl Detector {
    /// Load the detector from a safetensors weights file.
    pub fn load(weights: &Path) -> Result<Self> {
        if !weights.exists() {
            return Err(anyhow!(
                "weights not found at {} — train + convert first (python/README.md)",
                weights.display()
            ));
        }
        let device = Device::Cpu;
        // SAFETY: mmap of a trusted local artifact produced by our conversion step.
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights], DType::F32, &device)? };
        let model = YoloV8::load(vb, Multiples::n(), NUM_CLASSES)
            .context("building YOLOv8n graph — check class count matches the weights")?;
        Ok(Self { model, device })
    }

    /// Detect on an in-memory RGB frame, returning calibration points + tips in
    /// original-image pixels (origin top-left).
    pub fn detect(&self, img: &RgbImage) -> Result<Detection> {
        let lb = letterbox(img, &self.device)?;
        let pred = self.model.forward(&lb.tensor)?;
        let dets = nms(decode(&pred)?);

        // Map a letterboxed centre back to the original image.
        let to_orig = |cx: f32, cy: f32| {
            Point::new(
                ((cx - lb.pad_x) / lb.scale) as f64,
                ((cy - lb.pad_y) / lb.scale) as f64,
            )
        };

        // One best detection per calibration class (0..=3), in canonical order.
        let mut calibration = [None; 4];
        let mut tips = Vec::new();
        for d in &dets {
            if d.class == CLASS_DART {
                tips.push((d.conf, to_orig(d.cx, d.cy)));
            } else if d.class < 4 {
                let slot = &mut calibration[d.class];
                if slot.is_none_or(|(c, _)| d.conf > c) {
                    *slot = Some((d.conf, to_orig(d.cx, d.cy)));
                }
            }
        }

        let calibration = collect_calibration(calibration)?;
        // Keep at most three highest-confidence tips (a turn is three darts).
        tips.sort_by(|a, b| b.0.total_cmp(&a.0));
        tips.truncate(3);
        let tips = tips.into_iter().map(|(_, p)| p).collect();

        Ok(Detection { calibration, tips })
    }
}

/// Run detection on an image file (loads the model fresh; for one-shot CLI use).
pub fn detect_image(weights: &Path, image: &Path) -> Result<Detection> {
    let img = image::open(image)
        .with_context(|| format!("opening image {}", image.display()))?
        .to_rgb8();
    Detector::load(weights)?.detect(&img)
}

fn collect_calibration(slots: [Option<(f32, Point)>; 4]) -> Result<[Point; 4]> {
    let names = ["cal_top", "cal_right", "cal_bottom", "cal_left"];
    let mut pts = [Point::new(0.0, 0.0); 4];
    for (i, slot) in slots.iter().enumerate() {
        let (_, p) =
            slot.ok_or_else(|| anyhow!("calibration point '{}' not detected", names[i]))?;
        pts[i] = p;
    }
    Ok(pts)
}

/// CLI entry: detect and print the points.
pub fn run(image: &Path) -> Result<()> {
    let weights = Path::new("models/dartscore.safetensors");
    let det = detect_image(weights, image)?;
    for (i, p) in det.calibration.iter().enumerate() {
        println!("calib[{i}]: ({:.1}, {:.1})", p.x, p.y);
    }
    for (i, p) in det.tips.iter().enumerate() {
        println!("tip[{i}]:   ({:.1}, {:.1})", p.x, p.y);
    }
    Ok(())
}
