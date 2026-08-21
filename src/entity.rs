//! Session-local Bevy entity handles and live-World resolution.
//!
//! Handles preserve an entity index and generation without relying on a lossy JSON integer. They
//! are meaningful only in the Controlled Session and World that produced them.

use bevy::ecs::entity::{Entity, EntityGeneration, EntityIndex};
use bevy::prelude::World;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A session-local, lossless Bevy entity handle.
///
/// Handles are valid only for the session that produced them. The two 32-bit fields avoid the
/// precision loss that a JSON number can suffer when consumed by JavaScript.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(deny_unknown_fields)]
pub struct Handle {
    /// Raw Bevy entity index bits.
    pub index: u32,
    /// Raw Bevy entity generation bits.
    pub generation: u32,
}

impl Handle {
    /// Creates a handle from raw Bevy index and generation bits.
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Losslessly encodes a Bevy entity for the current session.
    pub const fn from_entity(entity: Entity) -> Self {
        Self {
            index: entity.index_u32(),
            generation: entity.generation().to_bits(),
        }
    }

    /// Reconstructs an [`Entity`] when the raw index bits are representable.
    ///
    /// This does not check whether the entity is live. Use [`Self::resolve`] for validation against
    /// a particular [`World`].
    pub fn entity(self) -> Option<Entity> {
        let index = EntityIndex::from_raw_u32(self.index)?;
        Some(Entity::from_index_and_generation(
            index,
            EntityGeneration::from_bits(self.generation),
        ))
    }

    /// Resolves and validates this handle against the current [`World`].
    ///
    /// Despawned entities and reused indices with another generation return
    /// [`HandleError::NotLive`].
    pub fn resolve(self, world: &World) -> Result<Entity, HandleError> {
        let entity = self.entity().ok_or(HandleError::InvalidBits(self))?;
        world
            .get_entity(entity)
            .map(|_| entity)
            .map_err(|_| HandleError::NotLive(self))
    }
}

impl From<Entity> for Handle {
    fn from(value: Entity) -> Self {
        Self::from_entity(value)
    }
}

impl TryFrom<Handle> for Entity {
    type Error = HandleError;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        value.entity().ok_or(HandleError::InvalidBits(value))
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.index, self.generation)
    }
}

/// Failure while reconstructing or resolving a session-local [`Handle`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandleError {
    /// The raw entity index bits cannot represent a Bevy entity.
    InvalidBits(Handle),
    /// The encoded entity and generation are not live in the inspected World.
    NotLive(Handle),
}

impl fmt::Display for HandleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBits(handle) => write!(formatter, "invalid entity handle {handle}"),
            Self::NotLive(handle) => write!(formatter, "entity handle {handle} is not live"),
        }
    }
}

impl std::error::Error for HandleError {}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::World;

    #[test]
    fn handle_roundtrips_large_generation_without_a_json_number() {
        let entity = Entity::from_index_and_generation(
            EntityIndex::from_raw_u32(42).unwrap(),
            EntityGeneration::from_bits(u32::MAX),
        );
        let handle = Handle::from_entity(entity);
        let json = serde_json::to_value(handle).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"index": 42, "generation": u32::MAX})
        );
        assert_eq!(Handle::entity(handle), Some(entity));
        assert_eq!(serde_json::from_value::<Handle>(json).unwrap(), handle);
    }

    #[test]
    fn handle_rejects_despawned_and_wrong_generation_entities() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let handle = Handle::from(entity);
        assert_eq!(handle.resolve(&world), Ok(entity));
        let wrong_generation = Handle::new(handle.index, handle.generation.wrapping_add(1));
        assert_eq!(
            wrong_generation.resolve(&world),
            Err(HandleError::NotLive(wrong_generation))
        );
        world.despawn(entity);
        assert_eq!(handle.resolve(&world), Err(HandleError::NotLive(handle)));

        let replacement = world.spawn_empty().id();
        assert_ne!(Handle::from(replacement), handle);
    }
}
