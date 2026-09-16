//! HTTP management and fixed-session WebSocket access.

mod access;
pub use access::{Client, Error, Management};
