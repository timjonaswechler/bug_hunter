//! Commands implemented by the experimental v3 vertical slice.
pub mod input;
pub mod inspect;
pub mod recording;
pub mod replay;
pub mod screenshot;
pub mod tick;

use serde::{Deserialize, Serialize};

pub(crate) mod private {
    pub trait Sealed {}
}

pub trait Request: private::Sealed + Into<Command> {
    type Output: serde::de::DeserializeOwned;
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "command", content = "arguments", deny_unknown_fields)]
pub enum Command {
    #[serde(rename = "replay.start")]
    ReplayStart(replay::Start),
    #[serde(rename = "replay.stop")]
    ReplayStop(replay::Stop),
    #[serde(rename = "recording.start")]
    RecordingStart(recording::Start),
    #[serde(rename = "recording.stop")]
    RecordingStop(recording::Stop),
    #[serde(rename = "input.text.input")]
    TextInput(input::text::Input),
    #[serde(rename = "input.keyboard.press")]
    KeyboardPress(input::keyboard::Press),
    #[serde(rename = "input.keyboard.release")]
    KeyboardRelease(input::keyboard::Release),
    #[serde(rename = "input.pointer.move_to")]
    PointerMoveTo(input::pointer::MoveTo),
    #[serde(rename = "input.pointer.move_by")]
    PointerMoveBy(input::pointer::MoveBy),
    #[serde(rename = "input.pointer.press")]
    PointerPress(input::pointer::Press),
    #[serde(rename = "input.pointer.release")]
    PointerRelease(input::pointer::Release),
    #[serde(rename = "input.pointer.scroll")]
    PointerScroll(input::pointer::Scroll),
    #[serde(rename = "tick.warp.start")]
    Start(tick::warp::Start),
    #[serde(rename = "tick.warp.set_pace")]
    SetPace(tick::warp::SetPace),
    #[serde(rename = "tick.warp.stop")]
    Stop(tick::warp::Stop),
    #[serde(rename = "inspect.query")]
    Inspect(inspect::Command),
    #[serde(rename = "screenshot.capture")]
    Screenshot(screenshot::Capture),
    #[serde(rename = "shutdown")]
    Shutdown(Empty),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Empty {}

impl Command {
    pub(crate) fn is_recordable(&self) -> bool {
        !matches!(
            self,
            Self::RecordingStart(_)
                | Self::RecordingStop(_)
                | Self::Shutdown(_)
                | Self::ReplayStart(_)
                | Self::ReplayStop(_)
        )
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::ReplayStart(_) => "replay.start",
            Self::ReplayStop(_) => "replay.stop",
            Self::RecordingStart(_) => "recording.start",
            Self::RecordingStop(_) => "recording.stop",
            Self::TextInput(_) => "input.text.input",
            Self::KeyboardPress(_) => "input.keyboard.press",
            Self::KeyboardRelease(_) => "input.keyboard.release",
            Self::PointerMoveTo(_) => "input.pointer.move_to",
            Self::PointerMoveBy(_) => "input.pointer.move_by",
            Self::PointerPress(_) => "input.pointer.press",
            Self::PointerRelease(_) => "input.pointer.release",
            Self::PointerScroll(_) => "input.pointer.scroll",
            Self::Start(_) => "tick.warp.start",
            Self::SetPace(_) => "tick.warp.set_pace",
            Self::Stop(_) => "tick.warp.stop",
            Self::Inspect(_) => "inspect.query",
            Self::Screenshot(_) => "screenshot.capture",
            Self::Shutdown(_) => "shutdown",
        }
    }

    pub(crate) fn validate_output(&self, output: &serde_json::Value) -> bool {
        fn valid<T: serde::de::DeserializeOwned>(v: &serde_json::Value) -> bool {
            serde_json::from_value::<T>(v.clone()).is_ok()
        }
        match self {
            Self::ReplayStart(_) => valid::<replay::Completion>(output),
            Self::ReplayStop(_) => valid::<replay::Stopped>(output),
            Self::RecordingStart(start) => {
                serde_json::from_value::<recording::Started>(output.clone())
                    .is_ok_and(|result| result.path == start.path)
            }
            Self::RecordingStop(_) => valid::<recording::Stopped>(output),
            Self::Start(start) => serde_json::from_value::<tick::warp::Completion>(output.clone())
                .is_ok_and(|c| {
                    c.requested_ticks == start.ticks
                        && c.executed_ticks <= c.requested_ticks
                        && (c.outcome == tick::warp::Outcome::Stopped
                            || c.executed_ticks == c.requested_ticks)
                }),
            Self::SetPace(_) => valid::<tick::warp::PaceChanged>(output),
            Self::Stop(_) => valid::<tick::warp::Stopped>(output),
            Self::Inspect(_) => valid::<inspect::Output>(output),
            Self::Screenshot(capture) => {
                serde_json::from_value::<screenshot::Output>(output.clone()).is_ok_and(|result| {
                    result.path == capture.path && result.width > 0 && result.height > 0
                })
            }
            Self::TextInput(_)
            | Self::KeyboardPress(_)
            | Self::KeyboardRelease(_)
            | Self::PointerMoveTo(_)
            | Self::PointerMoveBy(_)
            | Self::PointerPress(_)
            | Self::PointerRelease(_)
            | Self::PointerScroll(_)
            | Self::Shutdown(_) => output.is_null(),
        }
    }
}

macro_rules! request {
    ($ty:ty, $variant:ident, $output:ty) => {
        impl crate::command::private::Sealed for $ty {}
        impl crate::command::Request for $ty {
            type Output = $output;
        }
        impl From<$ty> for crate::command::Command {
            fn from(value: $ty) -> Self {
                Self::$variant(value)
            }
        }
    };
}
pub(crate) use request;
