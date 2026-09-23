use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveTo {
    /// Logical pixels from the upper-left corner of the primary window.
    pub position: [f32; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveBy {
    pub delta: [f32; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Press {
    /// One of `left`, `right`, or `middle`.
    pub button: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    /// One of `left`, `right`, or `middle`.
    pub button: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scroll {
    /// Horizontal and vertical deltas in Bevy line units.
    pub delta: [f32; 2],
}

crate::command::request!(MoveTo, PointerMoveTo, ());
crate::command::request!(MoveBy, PointerMoveBy, ());
crate::command::request!(Press, PointerPress, ());
crate::command::request!(Release, PointerRelease, ());
crate::command::request!(Scroll, PointerScroll, ());
