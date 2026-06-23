//! Automatic dart scoring core.
//!
//! The neural detector (candle/YOLOv8) has a deliberately narrow job: from one
//! frame return four board calibration points and up to three dart-tip points.
//! Everything downstream of that — homography, board mapping, score
//! classification, game logic, persistence, web UI — is plain deterministic Rust.

pub mod db;
pub mod game;
pub mod geometry;
pub mod homography;
pub mod scoring;
pub mod web;

#[cfg(feature = "detection")]
pub mod detect;

pub use geometry::{classify, Point, Ring, Score};
pub use scoring::{score_detection, Detection};
