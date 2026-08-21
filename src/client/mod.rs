//! Controlled Session integration that runs inside the child application.

mod plugin;
pub mod transport;

pub use plugin::{AutomationControlPlugin, InputFactory};
pub use transport::{Input, JsonLinesInput, Output, StdoutOutput};
