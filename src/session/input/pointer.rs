use crate::session::protocol::Diagnostic;
#[cfg(feature = "headless-2d")]
use bevy::camera::{ImageRenderTarget, RenderTarget};
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

#[derive(Clone, Debug)]
enum Target {
    Window {
        entity: Entity,
        logical_size: Vec2,
    },
    #[cfg(feature = "headless-2d")]
    Image {
        camera: Entity,
        target: ImageRenderTarget,
        logical_size: Vec2,
    },
}

impl PartialEq for Target {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // Window geometry is mutable. Entity identity and current bounds are
            // deliberately checked separately when an accepted location is reused.
            (Self::Window { entity: left, .. }, Self::Window { entity: right, .. }) => {
                left == right
            }
            #[cfg(feature = "headless-2d")]
            (
                Self::Image {
                    camera: left_camera,
                    target: left_target,
                    logical_size: left_size,
                },
                Self::Image {
                    camera: right_camera,
                    target: right_target,
                    logical_size: right_size,
                },
            ) => {
                left_camera == right_camera
                    && left_target == right_target
                    && left_size == right_size
            }
            #[cfg(feature = "headless-2d")]
            _ => false,
        }
    }
}

impl Target {
    fn logical_size(&self) -> Vec2 {
        match self {
            Self::Window { logical_size, .. } => *logical_size,
            #[cfg(feature = "headless-2d")]
            Self::Image { logical_size, .. } => *logical_size,
        }
    }

    fn location(&self, position: Vec2) -> Location {
        let target = match self {
            Self::Window { entity, .. } => {
                NormalizedRenderTarget::Window(WindowRef::Entity(*entity).normalize(None).unwrap())
            }
            #[cfg(feature = "headless-2d")]
            Self::Image { target, .. } => NormalizedRenderTarget::Image(target.clone()),
        };
        Location { target, position }
    }
}

/// Accepted state is private: neither input messages nor Window/ButtonInput change before a tick.
#[derive(Default)]
pub(crate) struct State {
    location: Option<(Target, Vec2)>,
    pressed: HashSet<MouseButton>,
    pending: Vec<PointerInput>,
}

impl State {
    pub(crate) fn move_to(&mut self, world: &World, position: [f32; 2]) -> Result<(), Diagnostic> {
        let position = finite(position)?;
        let target = target(world)?;
        self.move_on(target, position)
    }

    pub(crate) fn move_by(&mut self, world: &World, delta: [f32; 2]) -> Result<(), Diagnostic> {
        let delta = finite(delta)?;
        let target = target(world)?;
        let (_, position) = self.location(world)?;
        self.move_on(target, position + delta)
    }

    fn move_on(&mut self, target: Target, position: Vec2) -> Result<(), Diagnostic> {
        if !in_bounds(target.logical_size(), position) {
            return Err(Diagnostic::new(
                "pointer_position_out_of_bounds",
                "position must lie inside the logical image target or primary window",
            ));
        }
        let delta = self
            .location
            .as_ref()
            .filter(|(accepted, _)| *accepted == target)
            .map(|(_, previous)| position - *previous);
        self.pending.push(PointerInput::new(
            ID,
            target.location(position),
            PointerAction::Move {
                delta: delta.unwrap_or(Vec2::ZERO),
            },
        ));
        self.location = Some((target, position));
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
        let (target, position) = self.location(world)?;
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
            target.location(position),
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
        target(world)?;
        let (target, position) = self.location(world)?;
        self.pending.push(PointerInput::new(
            ID,
            target.location(position),
            PointerAction::Scroll {
                unit: MouseScrollUnit::Line,
                x: delta.x,
                y: delta.y,
                phase: TouchPhase::Moved,
            },
        ));
        Ok(())
    }

    fn location(&self, world: &World) -> Result<(&Target, Vec2), Diagnostic> {
        self.location
            .as_ref()
            .filter(|(accepted, position)| {
                target(world).is_ok_and(|current| {
                    current == *accepted && in_bounds(current.logical_size(), *position)
                })
            })
            .map(|(target, position)| (target, *position))
            .ok_or_else(|| {
                Diagnostic::new(
                    "pointer_location_unavailable",
                    "move the pointer into the current image target or primary window first",
                )
            })
    }

    pub(crate) fn flush(&mut self, world: &mut World) {
        // Picking consumes our custom device directly. Native compatibility messages are
        // emitted only for a real Window; image targets never invent a Window identity.
        for event in self.pending.drain(..) {
            if let NormalizedRenderTarget::Window(window) = &event.location.target {
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
            }
            world.write_message(event);
        }
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

fn in_bounds(logical_size: Vec2, position: Vec2) -> bool {
    position.is_finite()
        && position.x >= 0.0
        && position.y >= 0.0
        && position.x < logical_size.x
        && position.y < logical_size.y
}

fn target(world: &World) -> Result<Target, Diagnostic> {
    if let Some((entity, window)) = super::primary_window(world) {
        return Ok(Target::Window {
            entity,
            logical_size: Vec2::new(window.width(), window.height()),
        });
    }

    #[cfg(feature = "headless-2d")]
    if !world
        .iter_entities()
        .any(|entity| entity.contains::<Window>())
    {
        let image_cameras: Vec<_> = world
            .iter_entities()
            .filter_map(|entity| {
                let camera = entity.get::<Camera>()?;
                let RenderTarget::Image(image_target) = entity.get::<RenderTarget>()? else {
                    return None;
                };
                Some((entity, camera, image_target))
            })
            .collect();
        if let [(entity, camera, image_target)] = image_cameras.as_slice()
            && entity.contains::<crate::session::HeadlessCaptureCamera2d>()
            && entity.contains::<Camera2d>()
            && !entity.contains::<Camera3d>()
            && camera.is_active
            && camera.viewport.is_none()
            && matches!(
                camera.output_mode,
                bevy::camera::CameraOutputMode::Write { .. }
            )
            && image_target.scale_factor.is_finite()
            && image_target.scale_factor > 0.0
            && let Some(info) = camera.computed.target_info.as_ref()
            && info.physical_size.x > 0
            && info.physical_size.y > 0
            && info.scale_factor.to_bits() == image_target.scale_factor.to_bits()
            && world
                .get_resource::<Assets<Image>>()
                .and_then(|images| images.get(&image_target.handle))
                .is_some_and(|image| {
                    info.physical_size
                        == UVec2::new(
                            image.texture_descriptor.size.width,
                            image.texture_descriptor.size.height,
                        )
                })
        {
            return Ok(Target::Image {
                camera: entity.id(),
                target: (*image_target).clone(),
                logical_size: info.physical_size.as_vec2() / info.scale_factor,
            });
        }
    }

    Err(Diagnostic::new(
        "pointer_window_unavailable",
        "expected exactly one primary window or initialized marked full-image 2D target",
    ))
}

#[cfg(test)]
mod tests;
