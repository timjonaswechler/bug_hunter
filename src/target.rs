//! Discoverable entity marker for Controlled Session observations.
//!
//! [`crate::observation::Selector::Targets`] selects entities containing [`AutomationTarget`].
//! Discovery returns session-local handles rather than persistent semantic IDs.

use bevy::{ecs::reflect::ReflectComponent, prelude::Component, reflect::Reflect};

/// Marker for entities that a Controller may discover in a Controlled Session.
///
/// The marker intentionally carries no semantic identifier, role, label, or action list. Entity
/// handles are selected from the current World observation and are valid only for this session.
#[derive(Component, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct AutomationTarget;
