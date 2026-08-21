//! Debug Host support for launching and controlling a child Controlled Session.
//!
//! This module is available with the `host` Cargo feature and is not part of a Player Run.
//! [`LaunchSpec`] and [`Config`] describe child startup, while [`Session`] owns the JSONL protocol
//! lifecycle (`spawn` → `ready` → requests/waits → `shutdown`). [`recording`] persists host-side
//! Session Recordings; diagnostics and report helpers preserve failure artifacts; [`github`] can
//! prepare or publish issue drafts.
//!
//! Host and child artifacts have separate roots. [`SessionOptions::artifact_dir`] contains Session
//! Recordings and driver diagnostics. [`SessionOptions::session_artifact_dir`] is passed to the
//! child through [`crate::AUTOMATION_CONTROL_ARTIFACT_DIR`] for screenshots and other artifacts the
//! Controlled Session writes. Neither root is implicitly nested below the other.

mod command_line;
mod config;
mod controller;
mod diagnostics;
pub mod github;
mod issue_report;
mod launch;
pub mod recording;
mod repl;
mod replay;
mod report;
mod runner;
mod script;
mod session;
pub mod wait;

pub use command_line::{
    CommandLine, CommandLineError, ReportOptions, RunOptions, USAGE as COMMAND_LINE_USAGE,
};
pub use config::{
    ApplicationConfig, CONFIG_VERSION, Config, ConfigError, DefaultMode, ReportConfig,
    ScreenConfig, SessionConfig, ToolConfig,
};
pub use diagnostics::{
    DEFAULT_RECENT_LOG_CAPACITY, DiagnosticArtifacts, DiagnosticsError, FAILURE_REPORT_VERSION,
    FailureHeadline, FailureReport, RecentLogs,
};
pub use issue_report::{IssueDraft, ReportError};
pub use launch::{LaunchSpec, LaunchTargetKind};
pub use runner::run_embedded;
pub use session::{DriverError, Session, SessionOptions};
