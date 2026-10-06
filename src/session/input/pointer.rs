use crate::session::protocol::Diagnostic;
use bevy::{
    camera::NormalizedRenderTarget,
    input::{
        ButtonState,
        mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel},
        touch::TouchPhase,
    },
    picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput},
    prelude::*,
    window::WindowRef,
};
use std::collections::HashSet;

// Identifies a virtual device in this World, not a client or a global OS device.
pub(super) const ID: PointerId = PointerId::Custom(bevy::asset::uuid::Uuid::from_u128(
    0x776f6f64_7065_636b_6572_000000000001,
));

/// Accepted state is private: neither input messages nor Window/ButtonInput change before a tick.
#[derive(Default)]
pub(crate) struct State {
    location: Option<(Entity, Vec2)>,
    pressed: HashSet<MouseButton>,
    pending: Vec<PointerInput>,
}

impl State {
    pub(crate) fn move_to(&mut self, world: &World, position: [f32; 2]) -> Result<(), Diagnostic> {
        let position = finite(position)?;
        let (entity, window) = window(world)?;
        self.move_on(entity, window, position)
    }

    pub(crate) fn move_by(&mut self, world: &World, delta: [f32; 2]) -> Result<(), Diagnostic> {
        let delta = finite(delta)?;
        let (entity, window) = window(world)?;
        let (_, position) = self.location(world)?;
        self.move_on(entity, window, position + delta)
    }

    fn move_on(
        &mut self,
        entity: Entity,
        window: &Window,
        position: Vec2,
    ) -> Result<(), Diagnostic> {
        if !in_bounds(window, position) {
            return Err(Diagnostic::new(
                "pointer_position_out_of_bounds",
                "position must lie inside the primary window",
            ));
        }
        let delta = self
            .location
            .filter(|(id, _)| *id == entity)
            .map(|(_, p)| position - p);
        self.pending.push(PointerInput::new(
            ID,
            location(entity, position),
            PointerAction::Move {
                delta: delta.unwrap_or(Vec2::ZERO),
            },
        ));
        self.location = Some((entity, position));
        Ok(())
    }

    pub(crate) fn button(
        &mut self,
        world: &World,
        token: &str,
        down: bool,
    ) -> Result<(), Diagnostic> {
        let (button, picking_button) = match token {
            "left" => (MouseButton::Left, PointerButton::Primary),
            "right" => (MouseButton::Right, PointerButton::Secondary),
            "middle" => (MouseButton::Middle, PointerButton::Middle),
            _ => return Err(Diagnostic::new("invalid_pointer_button", token)),
        };
        let (window, position) = self.location(world)?;
        if self.pressed.contains(&button) == down {
            return Err(Diagnostic::new(
                if down {
                    "pointer_button_already_pressed"
                } else {
                    "pointer_button_not_pressed"
                },
                token,
            ));
        }
        self.pending.push(PointerInput::new(
            ID,
            location(window, position),
            if down {
                PointerAction::Press(picking_button)
            } else {
                PointerAction::Release(picking_button)
            },
        ));
        if down {
            self.pressed.insert(button);
        } else {
            self.pressed.remove(&button);
        }
        Ok(())
    }

    pub(crate) fn scroll(&mut self, world: &World, delta: [f32; 2]) -> Result<(), Diagnostic> {
        let delta = finite(delta)?;
        window(world)?;
        let (window, position) = self.location(world)?;
        self.pending.push(PointerInput::new(
            ID,
            location(window, position),
            PointerAction::Scroll {
                unit: MouseScrollUnit::Line,
                x: delta.x,
                y: delta.y,
                phase: TouchPhase::Moved,
            },
        ));
        Ok(())
    }

    fn location(&self, world: &World) -> Result<(Entity, Vec2), Diagnostic> {
        self.location
            .filter(|(entity, position)| {
                window(world)
                    .is_ok_and(|(id, window)| id == *entity && in_bounds(window, *position))
            })
            .ok_or_else(|| {
                Diagnostic::new(
                    "pointer_location_unavailable",
                    "move the pointer into the primary window first",
                )
            })
    }

    pub(crate) fn flush(&mut self, world: &mut World) {
        // Picking consumes our custom device directly. Never write Window's cursor
        // position: Winit would forward that change to the operating system.
        for event in self.pending.drain(..) {
            let NormalizedRenderTarget::Window(window) = event.location.target else {
                unreachable!("virtual pointer is window-backed")
            };
            match event.action {
                PointerAction::Press(button) | PointerAction::Release(button) => {
                    world.write_message(MouseButtonInput {
                        window: window.entity(),
                        button: match button {
                            PointerButton::Primary => MouseButton::Left,
                            PointerButton::Secondary => MouseButton::Right,
                            PointerButton::Middle => MouseButton::Middle,
                        },
                        state: if matches!(event.action, PointerAction::Press(_)) {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        },
                    });
                }
                PointerAction::Scroll { unit, x, y, phase } => {
                    world.write_message(MouseWheel {
                        window: window.entity(),
                        unit,
                        x,
                        y,
                        phase,
                    });
                }
                _ => {}
            }
            world.write_message(event);
        }
    }
}

fn location(window: Entity, position: Vec2) -> Location {
    Location {
        target: NormalizedRenderTarget::Window(WindowRef::Entity(window).normalize(None).unwrap()),
        position,
    }
}

fn finite(pair: [f32; 2]) -> Result<Vec2, Diagnostic> {
    let value = Vec2::from_array(pair);
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Diagnostic::new(
            "invalid_arguments",
            "coordinates and deltas must be finite",
        ))
    }
}

fn in_bounds(window: &Window, position: Vec2) -> bool {
    position.is_finite()
        && position.x >= 0.0
        && position.y >= 0.0
        && position.x < window.width()
        && position.y < window.height()
}

fn window(world: &World) -> Result<(Entity, &Window), Diagnostic> {
    super::primary_window(world).ok_or_else(|| {
        Diagnostic::new(
            "pointer_window_unavailable",
            "expected exactly one primary window",
        )
    })
}

#[cfg(test)]
mod tests;
