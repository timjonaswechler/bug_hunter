use serde::{Deserialize, Serialize};

/// Commits text to the application's focused editable entity on the next tick.
/// Success acknowledges validation and queuing, not application processing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// At most 16,384 UTF-8 bytes. Empty text is allowed.
    pub text: String,
}

impl Input {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

crate::command::request!(Input, TextInput, ());
