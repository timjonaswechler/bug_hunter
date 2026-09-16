//! Controlled Bevy sessions and a loopback-only multi-session server.
//!
//! Direct Rust integration uses [`session::Session`], [`session::Plugin`] and [`command`].
//! Enable `server`, `client` or `cli` for the network and executable entry points.
//! The experimental v3 slice implements Warp, Resource-Inspect and Shutdown.
//! Other target commands are not implemented yet; the old v2 API has been removed.

#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "client")]
pub mod client;
pub mod command;
pub mod handle;
pub mod report;
#[cfg(feature = "server")]
pub mod server;
pub mod session;
