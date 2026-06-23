//! Motion-settle trigger. Fires once after a dart lands and the image stabilizes,
//! so we only run inference on a still frame (not a motion-blurred one).
//!
//! Driven by a per-frame motion score — the mean absolute grayscale difference
//! against the previous frame. The state machine is pure and deterministic, so
//! it is unit-tested in isolation from any camera.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Scene quiet, waiting for movement.
    Idle,
    /// Movement in progress (dart in flight / being placed).
    Moving,
    /// Movement stopped; counting consecutive still frames.
    Settling,
}

/// Detects the "moved then settled" transition that marks a completed throw.
#[derive(Debug, Clone)]
pub struct SettleDetector {
    phase: Phase,
    stable: u32,
    /// Motion at/above this is "moving".
    motion_on: f32,
    /// Motion at/below this is "still".
    motion_off: f32,
    /// Consecutive still frames required to declare the scene settled.
    settle_frames: u32,
}

impl SettleDetector {
    pub fn new(motion_on: f32, motion_off: f32, settle_frames: u32) -> Self {
        Self {
            phase: Phase::Idle,
            stable: 0,
            motion_on,
            motion_off,
            settle_frames: settle_frames.max(1),
        }
    }

    /// Sensible defaults for a webcam pointed at a board (motion score is mean
    /// absolute 0–255 grayscale diff). Tune on real footage in Phase 3.
    pub fn with_defaults() -> Self {
        Self::new(6.0, 1.5, 5)
    }

    /// Feed one frame's motion score. Returns `true` exactly once, on the frame
    /// where the scene has settled after a period of motion.
    pub fn update(&mut self, motion: f32) -> bool {
        match self.phase {
            Phase::Idle => {
                if motion >= self.motion_on {
                    self.phase = Phase::Moving;
                }
                false
            }
            Phase::Moving => {
                if motion <= self.motion_off {
                    self.phase = Phase::Settling;
                    self.stable = 1;
                }
                false
            }
            Phase::Settling => {
                if motion >= self.motion_on {
                    // Movement resumed before settling completed.
                    self.phase = Phase::Moving;
                    self.stable = 0;
                    false
                } else if motion <= self.motion_off {
                    self.stable += 1;
                    if self.stable >= self.settle_frames {
                        self.phase = Phase::Idle;
                        self.stable = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    // Intermediate motion: stay settling, don't count this frame.
                    false
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.stable = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> SettleDetector {
        SettleDetector::new(6.0, 1.5, 3)
    }

    fn feed(d: &mut SettleDetector, scores: &[f32]) -> usize {
        scores.iter().filter(|&&m| d.update(m)).count()
    }

    #[test]
    fn quiet_never_triggers() {
        let mut d = detector();
        assert_eq!(feed(&mut d, &[0.2, 0.5, 1.0, 0.1, 0.0]), 0);
    }

    #[test]
    fn throw_then_settle_triggers_once() {
        let mut d = detector();
        // quiet, motion (dart lands), then 3 still frames -> one trigger.
        let triggers = feed(&mut d, &[0.3, 9.0, 8.0, 0.5, 0.5, 0.5]);
        assert_eq!(triggers, 1);
    }

    #[test]
    fn trigger_fires_on_the_settle_frame_not_before() {
        let mut d = detector();
        assert!(!d.update(0.3)); // idle
        assert!(!d.update(9.0)); // moving
        assert!(!d.update(0.5)); // settling stable=1
        assert!(!d.update(0.5)); // stable=2
        assert!(d.update(0.5)); // stable=3 -> trigger
        assert!(!d.update(0.5)); // back to idle, no re-trigger
    }

    #[test]
    fn motion_during_settle_delays_trigger() {
        let mut d = detector();
        let triggers = feed(
            &mut d,
            // move, settle x2, move again, then settle x3 -> exactly one trigger
            &[9.0, 0.5, 0.5, 9.0, 0.5, 0.5, 0.5],
        );
        assert_eq!(triggers, 1);
    }

    #[test]
    fn two_throws_trigger_twice() {
        let mut d = detector();
        let triggers = feed(&mut d, &[9.0, 0.5, 0.5, 0.5, 0.2, 9.0, 0.5, 0.5, 0.5]);
        assert_eq!(triggers, 2);
    }
}
