//! Camera capture + motion-settle trigger feeding detection → scoring (Phase 3).
//!
//! The `settle` trigger is pure and always built (and tested). The live camera
//! loop needs hardware + the detector, so it is gated behind the `capture`
//! feature (which implies `detection`).

pub mod settle;

#[cfg(feature = "capture")]
mod camera;

#[cfg(feature = "capture")]
pub use camera::{camera_list, list_cameras, run, snapshot, snapshot_png};
