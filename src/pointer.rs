//! Virtual pointer and picking events for window-backed render surfaces.
//!
//! A move establishes the session-local location used by later press, release, and line-unit scroll
//! actions. Successful bridge calls update state and construct Bevy events; event consumption
//! still occurs later in the application schedule.

use crate::entity::Handle;
use bevy::{
    camera::{NormalizedRenderTarget, RenderTarget},
    input::mouse::MouseScrollUnit,
    picking::pointer::{
        Location, PointerAction, PointerButton as BevyPointerButton, PointerId, PointerInput,
    },
    prelude::*,
    window::{PrimaryWindow, Window, WindowRef},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

const HELPER_DIAMETER: f32 = 16.0;

#[derive(Component)]
pub(crate) struct Helper;

/// A pointer-device button, not a semantic UI action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    /// Primary selection button, typically the left mouse button.
    Primary,
    /// Secondary action button, typically the right mouse button.
    Secondary,
    /// Middle mouse button.
    Middle,
}

/// Compatibility alias for [`Button`].
pub type PointerButton = Button;
/// Compatibility alias for [`Command`].
pub type PointerCommand = Command;
/// Compatibility alias for [`State`].
pub type PointerState = State;

impl From<Button> for BevyPointerButton {
    fn from(value: Button) -> Self {
        match value {
            Button::Primary => Self::Primary,
            Button::Secondary => Self::Secondary,
            Button::Middle => Self::Middle,
        }
    }
}

/// Virtual pointer transitions accepted by a Controlled Session.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Moves the virtual mouse pointer on a window surface.
    Move {
        /// Window handle, or `None` to resolve the sole primary window.
        surface: Option<Handle>,
        /// Surface coordinates in logical pixels.
        position: [f32; 2],
    },
    /// Presses a pointer button at the established location.
    Press {
        /// Button to press.
        button: Button,
    },
    /// Releases a pointer button at the established location.
    Release {
        /// Button to release.
        button: Button,
    },
    /// Scrolls at the established location using Bevy line units.
    Scroll {
        /// Horizontal and vertical line delta.
        delta: [f32; 2],
    },
}

impl Command {
    /// Checks that move coordinates or scroll deltas are finite.
    ///
    /// Surface, location, and button-transition checks occur in [`pointer_event`].
    pub fn validate(&self) -> Result<(), PointerError> {
        let finite = |values: &[f32; 2]| {
            values
                .iter()
                .all(|value| value.is_finite())
                .then_some(())
                .ok_or(PointerError::NonFinitePosition)
        };
        match self {
            Self::Move { position, .. } => finite(position),
            Self::Scroll { delta } => finite(delta),
            Self::Press { .. } | Self::Release { .. } => Ok(()),
        }
    }
}

/// Session-local virtual pointer state. It is never shared between Controlled Sessions.
#[derive(Clone, Debug, Default, Resource)]
pub struct State {
    /// Latest established pointer coordinates, or `None` before the first move.
    pub position: Option<[f32; 2]>,
    /// Latest resolved window surface, or `None` before the first move.
    pub surface: Option<Handle>,
    /// Buttons currently held by Virtual Input.
    pub pressed: BTreeSet<Button>,
    /// Most recent scroll delta; this is not an accumulated total.
    pub scroll_delta: [f32; 2],
}

impl State {
    /// Returns whether `button` is held in this session.
    pub fn is_pressed(&self, button: Button) -> bool {
        self.pressed.contains(&button)
    }

    /// Serializes the current pointer state for a Virtual Input summary observation.
    pub fn observation(&self) -> serde_json::Value {
        serde_json::json!({
            "position": self.position,
            "surface": self.surface,
            "pressed": self.pressed.iter().copied().collect::<Vec<_>>(),
            "scroll_delta": self.scroll_delta,
        })
    }
}

/// Pointer validation, surface-resolution, or transition failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PointerError {
    /// A move position or scroll delta contains a non-finite value.
    NonFinitePosition,
    /// No primary window is available for `surface: None`.
    NoPrimarySurface,
    /// More than one primary window is available.
    AmbiguousPrimarySurface,
    /// The requested surface handle is not live.
    SurfaceNotLive(Handle),
    /// The live requested entity is not a window.
    SurfaceNotWindow(Handle),
    /// Press, release, or scroll was requested before a successful move.
    NoLocation,
    /// A press was requested for an already-held button.
    ButtonAlreadyPressed(Button),
    /// A release was requested for a button that is not held.
    ButtonNotPressed(Button),
}

impl fmt::Display for PointerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinitePosition => formatter.write_str("pointer coordinates must be finite"),
            Self::NoPrimarySurface => formatter.write_str("no primary render surface is available"),
            Self::AmbiguousPrimarySurface => {
                formatter.write_str("more than one primary render surface is available")
            }
            Self::SurfaceNotLive(handle) => {
                write!(formatter, "surface handle {handle} is not live")
            }
            Self::SurfaceNotWindow(handle) => {
                write!(formatter, "surface handle {handle} is not a window")
            }
            Self::NoLocation => {
                formatter.write_str("pointer must move to a surface before this action")
            }
            Self::ButtonAlreadyPressed(button) => {
                write!(formatter, "pointer button {button:?} is already pressed")
            }
            Self::ButtonNotPressed(button) => {
                write!(formatter, "pointer button {button:?} is not pressed")
            }
        }
    }
}

impl std::error::Error for PointerError {}

/// A resolved render surface used by the pointer bridge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Surface {
    /// Session-local handle of the backing window entity.
    pub handle: Handle,
    /// Bevy normalized render target for pointer picking.
    pub target: NormalizedRenderTarget,
}

/// Resolves a requested window surface, or the sole primary window when `requested` is `None`.
///
/// Virtual pointer surfaces currently represent only window-backed normalized render targets.
pub fn resolve_surface(world: &World, requested: Option<Handle>) -> Result<Surface, PointerError> {
    let entity = match requested {
        Some(handle) => {
            let entity = handle
                .resolve(world)
                .map_err(|_| PointerError::SurfaceNotLive(handle))?;
            if world
                .get_entity(entity)
                .ok()
                .and_then(|value| value.get::<Window>())
                .is_none()
            {
                return Err(PointerError::SurfaceNotWindow(handle));
            }
            entity
        }
        None => {
            let mut primary = world
                .iter_entities()
                .filter(|entity| entity.contains::<PrimaryWindow>() && entity.contains::<Window>())
                .map(|entity| entity.id());
            let Some(entity) = primary.next() else {
                return Err(PointerError::NoPrimarySurface);
            };
            if primary.next().is_some() {
                return Err(PointerError::AmbiguousPrimarySurface);
            }
            entity
        }
    };
    let target = RenderTarget::Window(WindowRef::Entity(entity))
        .normalize(Some(entity))
        .ok_or(PointerError::SurfaceNotLive(Handle::from(entity)))?;
    Ok(Surface {
        handle: Handle::from(entity),
        target,
    })
}

/// Applies one virtual pointer transition and constructs a Bevy [`PointerInput`] event.
///
/// A successful call updates [`State`] but does not itself dispatch or guarantee consumption of the
/// event. Call move before actions that require a location.
///
/// ```
/// use automation_control::pointer::{Button, Command};
/// let sequence = [
///     Command::Move { surface: None, position: [10.0, 20.0] },
///     Command::Press { button: Button::Primary },
///     Command::Release { button: Button::Primary },
/// ];
/// assert!(sequence.iter().all(|command| command.validate().is_ok()));
/// ```
pub fn pointer_event(
    state: &mut State,
    world: &World,
    command: &Command,
) -> Result<PointerInput, PointerError> {
    command.validate()?;
    match *command {
        Command::Move { surface, position } => {
            let surface = resolve_surface(world, surface)?;
            let delta = match (state.surface, state.position) {
                (Some(previous), Some(previous_position)) if previous == surface.handle => {
                    Vec2::from(position) - Vec2::from(previous_position)
                }
                _ => Vec2::ZERO,
            };
            state.position = Some(position);
            state.surface = Some(surface.handle);
            Ok(PointerInput::new(
                PointerId::Mouse,
                Location {
                    target: surface.target,
                    position: Vec2::from(position),
                },
                PointerAction::Move { delta },
            ))
        }
        Command::Press { button } => {
            let (surface, position) = current_location(state, world)?;
            if !state.pressed.insert(button) {
                return Err(PointerError::ButtonAlreadyPressed(button));
            }
            Ok(PointerInput::new(
                PointerId::Mouse,
                Location {
                    target: surface.target,
                    position: Vec2::from(position),
                },
                PointerAction::Press(button.into()),
            ))
        }
        Command::Release { button } => {
            let (surface, position) = current_location(state, world)?;
            if !state.pressed.remove(&button) {
                return Err(PointerError::ButtonNotPressed(button));
            }
            Ok(PointerInput::new(
                PointerId::Mouse,
                Location {
                    target: surface.target,
                    position: Vec2::from(position),
                },
                PointerAction::Release(button.into()),
            ))
        }
        Command::Scroll { delta } => {
            let (surface, position) = current_location(state, world)?;
            state.scroll_delta = delta;
            Ok(PointerInput::new(
                PointerId::Mouse,
                Location {
                    target: surface.target,
                    position: Vec2::from(position),
                },
                PointerAction::Scroll {
                    x: delta[0],
                    y: delta[1],
                    unit: MouseScrollUnit::Line,
                    phase: bevy::input::touch::TouchPhase::Moved,
                },
            ))
        }
    }
}

fn current_location(state: &State, world: &World) -> Result<(Surface, [f32; 2]), PointerError> {
    let position = state.position.ok_or(PointerError::NoLocation)?;
    let handle = state.surface.ok_or(PointerError::NoLocation)?;
    Ok((resolve_surface(world, Some(handle))?, position))
}

/// Ensures the picking core has a virtual mouse pointer entity.
///
/// This supports small compositions that omit Bevy's native pointer-input plugin.
pub fn ensure_mouse_pointer(mut commands: Commands, pointers: Query<&PointerId>) {
    if !pointers.iter().any(PointerId::is_mouse) {
        commands.spawn(PointerId::Mouse);
    }
}

pub(crate) fn spawn_helper(mut commands: Commands) {
    commands.spawn((
        Helper,
        Pickable::IGNORE,
        GlobalZIndex(i32::MAX),
        Node {
            position_type: PositionType::Absolute,
            width: px(HELPER_DIAMETER),
            height: px(HELPER_DIAMETER),
            border_radius: BorderRadius::MAX,
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgb(1.0, 0.0, 0.0)),
    ));
}

pub(crate) fn update_helper(state: Res<State>, mut helpers: Query<&mut Node, With<Helper>>) {
    for mut node in &mut helpers {
        let Some([x, y]) = state.position else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        node.left = px(x - HELPER_DIAMETER / 2.0);
        node.top = px(y - HELPER_DIAMETER / 2.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_all_pointer_variants_without_a_wire_click() {
        let commands = [
            Command::Move {
                surface: None,
                position: [1.0, 2.0],
            },
            Command::Press {
                button: Button::Primary,
            },
            Command::Release {
                button: Button::Middle,
            },
            Command::Scroll { delta: [0.0, -1.0] },
        ];
        for command in commands {
            assert!(command.validate().is_ok());
        }
        assert!(
            Command::Move {
                surface: None,
                position: [f32::NAN, 0.0]
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn surface_and_button_state_errors_are_typed() {
        let mut world = World::new();
        let mut state = State::default();
        let move_command = Command::Move {
            surface: None,
            position: [10.0, 20.0],
        };
        assert_eq!(
            pointer_event(&mut state, &world, &move_command).unwrap_err(),
            PointerError::NoPrimarySurface
        );

        let first = world.spawn((Window::default(), PrimaryWindow)).id();
        let second = world.spawn((Window::default(), PrimaryWindow)).id();
        assert_eq!(
            pointer_event(&mut state, &world, &move_command).unwrap_err(),
            PointerError::AmbiguousPrimarySurface
        );
        world.despawn(second);

        let non_window = world.spawn_empty().id();
        assert_eq!(
            pointer_event(
                &mut state,
                &world,
                &Command::Move {
                    surface: Some(Handle::from(non_window)),
                    position: [10.0, 20.0],
                },
            )
            .unwrap_err(),
            PointerError::SurfaceNotWindow(Handle::from(non_window))
        );

        pointer_event(
            &mut state,
            &world,
            &Command::Move {
                surface: Some(Handle::from(first)),
                position: [10.0, 20.0],
            },
        )
        .unwrap();
        let press = Command::Press {
            button: Button::Secondary,
        };
        pointer_event(&mut state, &world, &press).unwrap();
        assert_eq!(
            pointer_event(&mut state, &world, &press).unwrap_err(),
            PointerError::ButtonAlreadyPressed(Button::Secondary)
        );
        let release = Command::Release {
            button: Button::Secondary,
        };
        pointer_event(&mut state, &world, &release).unwrap();
        assert_eq!(
            pointer_event(&mut state, &world, &release).unwrap_err(),
            PointerError::ButtonNotPressed(Button::Secondary)
        );
        pointer_event(&mut state, &world, &Command::Scroll { delta: [1.0, -2.0] }).unwrap();
        assert_eq!(state.scroll_delta, [1.0, -2.0]);
        assert_eq!(
            state.observation()["scroll_delta"],
            serde_json::json!([1.0, -2.0])
        );
    }
}
