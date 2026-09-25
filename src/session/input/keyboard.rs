use crate::{command::input::keyboard::Key, session::protocol::Diagnostic};
#[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
use bevy::camera::RenderTarget;
use bevy::{
    ecs::schedule::ScheduleCleanupPolicy,
    input::{
        ButtonState, InputSystems,
        keyboard::{Key as LogicalKey, KeyboardInput, keyboard_input_system},
    },
    prelude::*,
    window::WindowEvent,
};
use std::collections::{HashMap, HashSet};

#[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
#[derive(Clone, Message)]
struct VirtualKeyboardInput {
    key_code: KeyCode,
    logical_key: LogicalKey,
    state: ButtonState,
}

enum Pending {
    Window(KeyboardInput),
    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
    Headless(VirtualKeyboardInput),
}

enum Target {
    Window(Entity),
    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
    Headless,
}

#[derive(Default)]
pub(crate) struct State {
    pressed: HashSet<KeyCode>,
    pending: Vec<Pending>,
}

impl State {
    pub(crate) fn key(&mut self, world: &World, key: &Key, down: bool) -> Result<(), Diagnostic> {
        let (key_code, logical_key) = key
            .resolve()
            .ok_or_else(|| Diagnostic::new("invalid_key", key.as_str()))?;
        let target = target(world)?;
        if self.pressed.contains(&key_code) == down {
            return Err(Diagnostic::new(
                if down {
                    "key_already_pressed"
                } else {
                    "key_not_pressed"
                },
                key.as_str(),
            ));
        }
        if down {
            self.pressed.insert(key_code);
        } else {
            self.pressed.remove(&key_code);
        }
        let state = if down {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };
        self.pending.push(match target {
            Target::Window(window) => Pending::Window(KeyboardInput {
                key_code,
                logical_key,
                window,
                state,
                repeat: false,
                text: None,
            }),
            #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
            Target::Headless => Pending::Headless(VirtualKeyboardInput {
                key_code,
                logical_key,
                state,
            }),
        });
        Ok(())
    }

    pub(crate) fn flush(&mut self, world: &mut World) {
        for event in self.pending.drain(..) {
            match event {
                Pending::Window(event) => {
                    world.write_message(event.clone());
                    world.write_message(WindowEvent::KeyboardInput(event));
                }
                #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
                Pending::Headless(event) => {
                    world.write_message(event);
                }
            }
        }
    }
}

fn target(world: &World) -> Result<Target, Diagnostic> {
    if let Some((window, _)) = super::primary_window(world) {
        return Ok(Target::Window(window));
    }
    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
    if !world
        .iter_entities()
        .any(|entity| entity.contains::<Window>())
    {
        let marked: Vec<_> = world
            .iter_entities()
            .filter(|entity| {
                #[cfg(feature = "headless-2d")]
                if entity.contains::<crate::session::HeadlessCaptureCamera2d>() {
                    return true;
                }
                #[cfg(feature = "headless-3d")]
                if entity.contains::<crate::session::HeadlessCaptureCamera3d>() {
                    return true;
                }
                false
            })
            .collect();
        if let [entity] = marked.as_slice() {
            let image_target = entity.get::<RenderTarget>().and_then(|target| {
                let RenderTarget::Image(target) = target else {
                    return None;
                };
                Some(target)
            });
            #[cfg(feature = "headless-2d")]
            let marked_2d = entity.contains::<crate::session::HeadlessCaptureCamera2d>();
            #[cfg(not(feature = "headless-2d"))]
            let marked_2d = false;
            #[cfg(feature = "headless-3d")]
            let marked_3d = entity.contains::<crate::session::HeadlessCaptureCamera3d>();
            #[cfg(not(feature = "headless-3d"))]
            let marked_3d = false;

            if marked_2d != marked_3d {
                #[cfg(feature = "headless-2d")]
                if marked_2d && entity.contains::<Camera2d>() && image_target.is_some() {
                    return Ok(Target::Headless);
                }
                #[cfg(feature = "headless-3d")]
                if marked_3d
                    && entity.contains::<Camera3d>()
                    && !entity.contains::<Camera2d>()
                    && image_target.is_some_and(|target| {
                        target.scale_factor.is_finite() && target.scale_factor > 0.0
                    })
                    && entity.get::<Camera>().is_some_and(|camera| {
                        camera.is_active
                            && camera.viewport.is_none()
                            && matches!(
                                camera.output_mode,
                                bevy::camera::CameraOutputMode::Write { .. }
                            )
                    })
                    && entity.get::<Projection>().is_some_and(|projection| {
                        let Projection::Perspective(projection) = projection else {
                            return false;
                        };
                        projection.fov.is_finite()
                            && projection.fov > 0.0
                            && projection.fov < std::f32::consts::PI
                            && projection.aspect_ratio.is_finite()
                            && projection.aspect_ratio > 0.0
                            && projection.near.is_finite()
                            && projection.near > 0.0
                            && projection.far.is_finite()
                            && projection.far > projection.near
                            && projection.near_clip_plane.is_finite()
                    })
                {
                    return Ok(Target::Headless);
                }
            }
        }
    }
    #[cfg(all(feature = "headless-2d", feature = "headless-3d"))]
    let expected = "expected exactly one primary window or fixed headless 2D/perspective 3D target";
    #[cfg(all(feature = "headless-3d", not(feature = "headless-2d")))]
    let expected = "expected exactly one primary window or fixed headless perspective 3D target";
    #[cfg(not(feature = "headless-3d"))]
    let expected = "expected exactly one primary window or fixed headless 2D target";
    Err(Diagnostic::new("keyboard_window_unavailable", expected))
}

pub(super) fn install(app: &mut App) {
    app.world_mut()
        .schedule_scope(PreUpdate, |world, schedule| {
            schedule
                .remove_systems_in_set(
                    keyboard_input_system,
                    world,
                    ScheduleCleanupPolicy::RemoveSystemsOnly,
                )
                .expect("replace native keyboard state updater");
        });
    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
    app.add_message::<VirtualKeyboardInput>();
    app.add_systems(PreUpdate, update.in_set(InputSystems));
}

fn apply(
    key_code: KeyCode,
    logical_key: &LogicalKey,
    state: ButtonState,
    physical: &mut ButtonInput<KeyCode>,
    logical: &mut ButtonInput<LogicalKey>,
    held: &mut HashMap<KeyCode, LogicalKey>,
) {
    match state {
        ButtonState::Pressed => {
            physical.press(key_code);
            logical.press(logical_key.clone());
            held.insert(key_code, logical_key.clone());
        }
        ButtonState::Released => {
            physical.release(key_code);
            held.remove(&key_code);
            // Both Shift keys, for example, map to logical Shift. Releasing one
            // physical key must not release a logical key still held by the other.
            if !held.values().any(|key| key == logical_key) {
                logical.release(logical_key.clone());
            }
        }
    }
}

fn update(
    mut events: MessageReader<KeyboardInput>,
    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))] mut virtual_events: MessageReader<
        VirtualKeyboardInput,
    >,
    mut physical: ResMut<ButtonInput<KeyCode>>,
    mut logical: ResMut<ButtonInput<LogicalKey>>,
    mut held: Local<HashMap<KeyCode, LogicalKey>>,
) {
    physical.bypass_change_detection().clear();
    logical.bypass_change_detection().clear();
    for event in events.read() {
        apply(
            event.key_code,
            &event.logical_key,
            event.state,
            &mut physical,
            &mut logical,
            &mut held,
        );
    }
    #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
    for event in virtual_events.read() {
        apply(
            event.key_code,
            &event.logical_key,
            event.state,
            &mut physical,
            &mut logical,
            &mut held,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::PrimaryWindow;

    #[test]
    fn validates_tokens_windows_and_transitions_without_mutating_on_rejection() {
        let mut world = World::new();
        let mut state = State::default();
        assert_eq!(
            state
                .key(&world, &Key::Unknown("A".into()), true)
                .unwrap_err()
                .code,
            "invalid_key"
        );
        assert_eq!(
            state.key(&world, &Key::A, true).unwrap_err().code,
            "keyboard_window_unavailable"
        );
        let window = world
            .spawn((
                Window {
                    focused: false,
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        assert_eq!(
            state.key(&world, &Key::A, false).unwrap_err().code,
            "key_not_pressed"
        );
        state.key(&world, &Key::A, true).unwrap();
        assert_eq!(
            state.key(&world, &Key::A, true).unwrap_err().code,
            "key_already_pressed"
        );
        // Typed and wire callers must agree even if an unknown wrapper contains a valid token.
        assert_eq!(
            state
                .key(&world, &Key::Unknown("a".into()), true)
                .unwrap_err()
                .code,
            "key_already_pressed"
        );
        let duplicate = world.spawn((Window::default(), PrimaryWindow)).id();
        assert_eq!(
            state.key(&world, &Key::A, false).unwrap_err().code,
            "keyboard_window_unavailable"
        );
        world.despawn(duplicate);
        world.despawn(window);
        assert_eq!(
            state.key(&world, &Key::A, false).unwrap_err().code,
            "keyboard_window_unavailable"
        );
        assert!(state.pressed.contains(&KeyCode::KeyA));
        assert_eq!(state.pending.len(), 1);
        let event = match &state.pending[0] {
            Pending::Window(event) => event,
            #[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
            Pending::Headless(_) => panic!("window input changed target"),
        };
        assert_eq!(event.window, window);
        assert_eq!(event.logical_key, LogicalKey::Character("a".into()));
        assert!(event.text.is_none());
        assert!(!event.repeat);
        world.spawn((Window::default(), PrimaryWindow));
        state.key(&world, &Key::A, false).unwrap();
        assert!(!state.pressed.contains(&KeyCode::KeyA));
        assert_eq!(state.pending.len(), 2);
    }
}
