//! Host-side bounded observation polling.
//!
//! There is no wire-level wait command. [`crate::driver::Session::wait_for_observation`] alternates
//! observations with explicit one-frame controlled-time advances.

use crate::time;

/// A hard upper bound for one host-side observation wait.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameLimit {
    pub(super) frames: u64,
    pub(super) step_nanoseconds: u64,
}

impl FrameLimit {
    /// Validates a maximum frame count and the controlled step used after each missed observation.
    pub fn new(frames: u64, step_nanoseconds: u64) -> Result<Self, time::Error> {
        time::Command::advance(frames, step_nanoseconds).validate()?;
        Ok(Self {
            frames,
            step_nanoseconds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_limit_uses_the_controlled_clock_bounds() {
        assert!(FrameLimit::new(1, 1).is_ok());
        assert_eq!(
            FrameLimit::new(0, 1).unwrap_err(),
            time::Error::InvalidFrames
        );
        assert_eq!(FrameLimit::new(1, 0).unwrap_err(), time::Error::InvalidStep);
        assert!(FrameLimit::new(time::MAX_FRAMES + 1, 1).is_err());
        assert!(FrameLimit::new(1, time::MAX_STEP_NANOSECONDS + 1).is_err());
    }
}
