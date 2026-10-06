//! HTTP management and fixed-session WebSocket access.

mod access;
#[cfg(feature = "cli")]
pub(crate) use access::Interrupt;
pub use access::{Client, Error, Management};
