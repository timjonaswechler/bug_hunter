//! Request-correlated state shared between main and render worlds.
use bevy::prelude::Entity;
use std::sync::Mutex;

#[derive(Debug)]
enum Stage {
    Warmup {
        ready: bool,
    },
    Capture {
        entity: Entity,
        result: Option<bool>,
    },
}

#[derive(Debug)]
pub(super) struct Verification(Mutex<Stage>);

impl Default for Verification {
    fn default() -> Self {
        Self(Mutex::new(Stage::Warmup { ready: false }))
    }
}

impl Verification {
    pub(super) fn warmup_ready(&self) -> bool {
        matches!(*self.0.lock().unwrap(), Stage::Warmup { ready: true })
    }

    pub(super) fn begin_capture(&self, entity: Entity) {
        *self.0.lock().unwrap() = Stage::Capture {
            entity,
            result: None,
        };
    }

    pub(super) fn capture_result(&self, entity: Entity) -> Option<bool> {
        match *self.0.lock().unwrap() {
            Stage::Capture {
                entity: expected,
                result,
            } if expected == entity => result,
            _ => None,
        }
    }

    pub(super) fn record_frame(&self, entity: Option<Entity>, ready: bool) {
        let mut stage = self.0.lock().unwrap();
        match (&mut *stage, entity) {
            (Stage::Warmup { ready: current }, None) => *current = ready,
            (
                Stage::Capture {
                    entity: expected,
                    result,
                },
                Some(actual),
            ) if *expected == actual && result.is_none() => *result = Some(ready),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warmup_cannot_verify_a_later_capture_entity() {
        let verification = Verification::default();
        verification.record_frame(None, true);
        assert!(verification.warmup_ready());
        let entity = Entity::from_bits(7);
        verification.begin_capture(entity);
        assert_eq!(verification.capture_result(entity), None);
        verification.record_frame(Some(entity), false);
        assert_eq!(verification.capture_result(entity), Some(false));
        verification.record_frame(Some(entity), true);
        assert_eq!(
            verification.capture_result(entity),
            Some(false),
            "a later frame must not repair the rejected capture frame"
        );
    }

    #[test]
    fn only_the_correlated_capture_entity_can_be_verified() {
        let verification = Verification::default();
        let entity = Entity::from_bits(9);
        verification.begin_capture(entity);
        verification.record_frame(Some(Entity::from_bits(10)), true);
        assert_eq!(verification.capture_result(entity), None);
        verification.record_frame(Some(entity), true);
        assert_eq!(verification.capture_result(entity), Some(true));
    }
}
