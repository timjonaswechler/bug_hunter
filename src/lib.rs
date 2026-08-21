//! Transport and Bevy integration for isolated Controlled Sessions.
//!
//! A Controller communicates with one Controlled Session through protocol-v2 [`Command`] values,
//! reads session state with [`observation::observe_world`], and supplies session-local Virtual
//! Input. Mark entities with [`AutomationTarget`] to make them discoverable. Rendered compositions
//! may opt into PNG artifacts with [`screenshot::Plugin`]. [`AutomationControlPlugin`] connects
//! these services to [`JsonLinesInput`] and an [`Output`] implementation.
//!
//! The `host` feature additionally exposes Debug Host process management, Session Recording,
//! diagnostics, controller workflows, and report helpers in [`host`]. The `driver` feature and
//! [`driver`] module remain compatibility aliases. Those host-side utilities are not part of a
//! Player Run. A Player Run does not depend on this crate or expose automation behavior.

pub mod client;
#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "host")]
pub use host as driver;
pub mod entity;
pub mod keyboard;
pub mod observation;
pub mod pointer;
pub mod protocol;
pub mod screenshot;
pub mod target;
pub mod text;
pub mod time;
pub use client::transport;

pub use client::{
    AutomationControlPlugin, Input, InputFactory, JsonLinesInput, Output, StdoutOutput,
};
pub use entity::{Handle, HandleError};
pub use protocol::{
    Command, PROTOCOL_VERSION, ProtocolError, Ready, Request, Response, ResponseStatus, RunMode,
    decode_request,
};
pub use target::AutomationTarget;
pub use time::Clock as ControlledClock;

/// Environment variable through which a Debug Host supplies a Controlled Session's artifact root.
///
/// The root contains artifacts produced by the child session, such as screenshots. It is distinct
/// from the host-side Session Recording root used by the feature-gated driver.
pub const AUTOMATION_CONTROL_ARTIFACT_DIR: &str = "AUTOMATION_CONTROL_ARTIFACT_DIR";

/// Resolves [`AUTOMATION_CONTROL_ARTIFACT_DIR`], falling back to `default` when absent or empty.
///
/// An explicit root passed to [`screenshot::Plugin::with_artifact_root`] bypasses this helper.
pub fn artifact_root_path(default: impl Into<std::path::PathBuf>) -> std::path::PathBuf {
    std::env::var_os(AUTOMATION_CONTROL_ARTIFACT_DIR)
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| default.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_uses_the_grouped_protocol_v2_wire_form() {
        let command = Command::Shutdown;
        assert_eq!(
            serde_json::to_value(command).unwrap(),
            serde_json::json!({"type": "shutdown"})
        );
        assert_eq!(PROTOCOL_VERSION, 2);
    }
}
