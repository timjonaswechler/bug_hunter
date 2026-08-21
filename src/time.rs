//! Explicit controlled-frame advancement and the per-session controlled clock.
//!
//! Controlled Sessions have no background simulation clock: controlled schedules run only for
//! requested advances. Each frame uses the requested fixed step and is counted only after it
//! completes.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fmt, time::Duration};

/// Maximum number of simulation frames accepted by one advance command.
pub const MAX_FRAMES: u64 = 10_000;
/// Maximum controlled delta accepted for one simulation frame.
pub const MAX_STEP_NANOSECONDS: u64 = 1_000_000_000;

/// Controlled-time actions accepted by a Controlled Session.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Runs `frames` controlled frames, each with the same controlled-time step.
    Advance {
        /// Number of frames to complete.
        frames: u64,
        /// Controlled delta applied to every frame.
        step_nanoseconds: u64,
    },
}

impl Command {
    /// Constructs an explicit frame-advance request.
    pub const fn advance(frames: u64, step_nanoseconds: u64) -> Self {
        Self::Advance {
            frames,
            step_nanoseconds,
        }
    }

    /// Enforces positive values and the per-command frame and step maxima.
    pub fn validate(&self) -> Result<(), Error> {
        let Self::Advance {
            frames,
            step_nanoseconds,
        } = *self;
        if frames == 0 {
            return Err(Error::InvalidFrames);
        }
        if frames > MAX_FRAMES {
            return Err(Error::TooManyFrames(frames));
        }
        if step_nanoseconds == 0 {
            return Err(Error::InvalidStep);
        }
        if step_nanoseconds > MAX_STEP_NANOSECONDS {
            return Err(Error::StepTooLarge(step_nanoseconds));
        }
        Ok(())
    }

    pub(crate) const fn into_advance(self) -> Advance {
        match self {
            Self::Advance {
                frames,
                step_nanoseconds,
            } => Advance {
                frames,
                step: Duration::from_nanos(step_nanoseconds),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Advance {
    pub frames: u64,
    pub step: Duration,
}

/// Controlled-time command validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Frame count is zero.
    InvalidFrames,
    /// Frame count exceeds [`MAX_FRAMES`].
    TooManyFrames(u64),
    /// Per-frame step is zero.
    InvalidStep,
    /// Per-frame step exceeds [`MAX_STEP_NANOSECONDS`].
    StepTooLarge(u64),
}

impl Error {
    /// Returns the stable protocol error code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidFrames => "invalid_time_frames",
            Self::TooManyFrames(_) => "time_frames_too_large",
            Self::InvalidStep => "invalid_time_step",
            Self::StepTooLarge(_) => "time_step_too_large",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFrames => formatter.write_str("frames must be greater than zero"),
            Self::TooManyFrames(frames) => {
                write!(
                    formatter,
                    "frames must be at most {MAX_FRAMES}, got {frames}"
                )
            }
            Self::InvalidStep => formatter.write_str("step_nanoseconds must be greater than zero"),
            Self::StepTooLarge(step) => write!(
                formatter,
                "step_nanoseconds must be at most {MAX_STEP_NANOSECONDS}, got {step}"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Session-local record of completed controlled frames and elapsed controlled time.
#[derive(Clone, Debug, Default, Resource)]
pub struct Clock {
    frame_index: u64,
    elapsed: Duration,
    last_step_nanoseconds: Option<u64>,
}

impl Clock {
    /// Returns the number of controlled frames completed in this session.
    pub const fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Returns total elapsed controlled time across completed frames.
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Returns the most recently completed frame's step, or `None` before the first frame.
    pub const fn last_step_nanoseconds(&self) -> Option<u64> {
        self.last_step_nanoseconds
    }

    pub(crate) fn complete_frame(&mut self, step: Duration) {
        self.frame_index = self.frame_index.saturating_add(1);
        self.elapsed = self.elapsed.saturating_add(step);
        self.last_step_nanoseconds = Some(step.as_nanos() as u64);
    }

    /// Serializes `frame_index`, `elapsed_nanoseconds`, and `last_step_nanoseconds`.
    pub fn observation(&self) -> Value {
        json!({
            "frame_index": self.frame_index,
            "elapsed_nanoseconds": u64::try_from(self.elapsed.as_nanos()).unwrap_or(u64::MAX),
            "last_step_nanoseconds": self.last_step_nanoseconds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_records_only_completed_frames() {
        let mut clock = Clock::default();
        assert_eq!(
            clock.observation(),
            json!({
                "frame_index": 0,
                "elapsed_nanoseconds": 0,
                "last_step_nanoseconds": null,
            })
        );

        clock.complete_frame(Duration::from_nanos(16_666_667));
        assert_eq!(clock.frame_index(), 1);
        assert_eq!(clock.elapsed(), Duration::from_nanos(16_666_667));
        assert_eq!(clock.last_step_nanoseconds(), Some(16_666_667));
    }
}
