//! Automatic dart scoring core.
//!
//! The neural detector (candle/YOLOv8, added in a later phase) has a deliberately
//! narrow job: from one frame return four board calibration points and up to three
//! dart-tip points. Everything in this crate downstream of that — homography,
//! board mapping, score classification, game logic — is plain deterministic Rust.

pub mod geometry;
pub mod homography;
pub mod scoring;

pub use geometry::{classify, Point, Ring, Score};
pub use scoring::{score_detection, Detection};
