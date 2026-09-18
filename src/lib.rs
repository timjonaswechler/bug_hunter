//! Controlled Bevy sessions and a loopback-only multi-session server.
//!
//! Direct Rust integration uses [`session::Session`], [`session::Plugin`] and [`command`].
//! Enable `server`, `client` or `cli` for the network and executable entry points.
//! The experimental v3 slice includes controlled input, inspection, screenshots, recording,
//! replay, failure observation, report rendering, local storage and GitHub publication.
//! The server observes and submits reports independently of clients. The interactive REPL and
//! prevalidated scripts use the same client contract.

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
