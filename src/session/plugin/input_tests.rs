use super::*;
use crate::command::input::{keyboard, pointer};
use bevy::{
    camera::NormalizedRenderTarget,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardFocusLost, KeyboardInput},
        mouse::MouseButtonInput,
    },
    picking::{
        input::PointerInputPlugin,
        pointer::{Location, PointerAction, PointerId, PointerInput, PointerLocation},
    },
    window::{CursorMoved, Ime, PrimaryWindow, WindowEvent, WindowRef, WindowResized},
};

#[derive(Resource, Default)]
struct Seen {
    keys: usize,
    text: usize,
    resizes: usize,
    keyboard_events: Vec<KeyboardInput>,
}

fn app() -> (App, Entity, mpsc::Sender<String>) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(PointerInputPlugin)
        .add_message::<Ime>()
        .init_resource::<Seen>()
        .add_systems(
            Update,
            |mut keys: MessageReader<KeyboardInput>,
             mut text: MessageReader<Ime>,
             mut windows: MessageReader<WindowEvent>,
             mut seen: ResMut<Seen>| {
                for event in keys.read() {
                    seen.keys += 1;
                    seen.keyboard_events.push(event.clone());
                }
                seen.text += text.read().count();
                seen.resizes += windows
                    .read()
                    .filter(|event| matches!(event, WindowEvent::WindowResized(_)))
                    .count();
            },
        );
    let mut window = Window {
        focused: false,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::new(90., 80.)));
    let window = app.world_mut().spawn((window, PrimaryWindow)).id();
    let (tx, input) = mpsc::channel();
    let (output, _rx) = mpsc::channel();
    install(&mut app, input, output, warp::Pace::AsFastAsPossible);
    (app, window, tx)
}

fn native_input(app: &mut App, window: Entity) {
    let world = app.world_mut();
    let moved = CursorMoved {
        window,
        position: Vec2::new(900., 800.),
        delta: None,
    };
    world.write_message(moved.clone());
    world.write_message(WindowEvent::CursorMoved(moved));
    let button = MouseButtonInput {
        window,
        button: MouseButton::Left,
        state: ButtonState::Released,
    };
    world.write_message(button);
    world.write_message(WindowEvent::MouseButtonInput(button));
    let key = KeyboardInput {
        window,
        key_code: KeyCode::KeyA,
        logical_key: Key::Character("a".into()),
        state: ButtonState::Pressed,
        text: Some("a".into()),
        repeat: false,
    };
    world.write_message(key.clone());
    world.write_message(WindowEvent::KeyboardInput(key));
    world.write_message(KeyboardFocusLost);
    world.write_message(WindowEvent::KeyboardFocusLost(KeyboardFocusLost));
    let text = Ime::Commit {
        window,
        value: "native".into(),
    };
    world.write_message(text.clone());
    world.write_message(WindowEvent::Ime(text));
    world.write_message(PointerInput::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(window).normalize(None).unwrap(),
            ),
            position: Vec2::new(900., 800.),
        },
        PointerAction::Move { delta: Vec2::ONE },
    ));
}

fn warp(app: &mut App, tx: &mpsc::Sender<String>, id: u64) {
    tx.send(protocol::encode(
        id,
        &warp::Start {
            ticks: 1,
            pace: None,
        }
        .into(),
    ))
    .unwrap();
    app.update();
}

#[cfg(feature = "headless-2d")]
#[derive(Resource, Default)]
struct HeadlessButtonSeen(u32);

#[cfg(feature = "headless-2d")]
#[test]
fn headless_image_pointer_drives_a_real_ui_button_only_in_warps() {
    use bevy::{
        asset::AssetPlugin,
        camera::{ComputedCameraValues, ImageRenderTarget, RenderTarget, RenderTargetInfo},
        image::{ImagePlugin, TextureAtlasPlugin},
        mesh::MeshPlugin,
        picking::{InteractionPlugin, PickingPlugin},
        render::render_resource::TextureFormat,
        text::TextPlugin,
        transform::TransformPlugin,
        ui::UiPlugin,
    };

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins((
            TransformPlugin,
            ImagePlugin::default(),
            TextureAtlasPlugin,
            MeshPlugin,
            bevy::camera::CameraPlugin,
            TextPlugin,
            PickingPlugin,
            InteractionPlugin,
            UiPlugin,
        ))
        .init_resource::<HeadlessButtonSeen>();
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            800,
            600,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let camera = app
        .world_mut()
        .spawn((
            Camera2d,
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(800, 600),
                        scale_factor: 2.0,
                    }),
                    ..default()
                },
                ..default()
            },
            RenderTarget::Image(ImageRenderTarget {
                handle: image,
                scale_factor: 2.0,
            }),
            crate::session::HeadlessCaptureCamera2d,
            IsDefaultUiCamera,
        ))
        .id();
    let button = app
        .world_mut()
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                left: px(100),
                top: px(80),
                width: px(100),
                height: px(50),
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .observe(
            |press: On<Pointer<Press>>, mut buttons: Query<&mut Interaction, With<Button>>| {
                *buttons.get_mut(press.event_target()).unwrap() = Interaction::Pressed;
            },
        )
        .observe(
            |release: On<Pointer<Release>>, mut buttons: Query<&mut Interaction, With<Button>>| {
                *buttons.get_mut(release.event_target()).unwrap() = Interaction::None;
            },
        )
        .id();
    app.add_systems(
        Update,
        |buttons: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
         mut seen: ResMut<HeadlessButtonSeen>| {
            seen.0 += buttons
                .iter()
                .filter(|interaction| **interaction == Interaction::Pressed)
                .count() as u32;
        },
    );

    let (tx, input) = mpsc::channel();
    let (output, rx) = mpsc::channel();
    install(&mut app, input, output, warp::Pace::AsFastAsPossible);
    let send = |id, command: Command| tx.send(protocol::encode(id, &command)).unwrap();
    let response = |id| {
        let message = rx.try_recv().unwrap().0;
        assert_eq!(message.id(), Some(id));
        message
    };

    warp(&mut app, &tx, 1); // Initialize camera and real Bevy UI layout.
    assert!(matches!(response(1), Message::Completed { .. }));
    send(
        2,
        pointer::MoveTo {
            position: [150.0, 105.0],
        }
        .into(),
    );
    send(
        3,
        pointer::Press {
            button: "left".into(),
        }
        .into(),
    );
    app.update();
    assert!(matches!(response(2), Message::Completed { .. }));
    assert!(matches!(response(3), Message::Completed { .. }));
    assert_eq!(
        app.world().get::<Interaction>(button),
        Some(&Interaction::None)
    );
    assert_eq!(app.world().resource::<HeadlessButtonSeen>().0, 0);

    warp(&mut app, &tx, 4);
    assert!(matches!(response(4), Message::Completed { .. }));
    assert_eq!(
        app.world().get::<Interaction>(button),
        Some(&Interaction::Pressed)
    );
    assert_eq!(app.world().resource::<HeadlessButtonSeen>().0, 1);

    send(
        5,
        pointer::Release {
            button: "left".into(),
        }
        .into(),
    );
    app.update();
    assert!(matches!(response(5), Message::Completed { .. }));
    assert_eq!(
        app.world().get::<Interaction>(button),
        Some(&Interaction::Pressed)
    );
    warp(&mut app, &tx, 6);
    assert!(matches!(response(6), Message::Completed { .. }));
    assert_eq!(
        app.world().get::<Interaction>(button),
        Some(&Interaction::None)
    );
}

#[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
#[derive(Resource, Default)]
struct HeadlessKeyboardSeen {
    ticks: u64,
    position: i32,
    physical: Vec<(bool, bool, bool)>,
    logical: Vec<(bool, bool, bool)>,
    native_events: usize,
}

#[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
fn headless_keyboard_app(
    spawn_target: fn(&mut App) -> Entity,
) -> (
    App,
    mpsc::Sender<String>,
    mpsc::Receiver<(Message, Option<mpsc::Sender<()>>)>,
) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<HeadlessKeyboardSeen>()
        .add_systems(
            Update,
            |physical: Res<ButtonInput<KeyCode>>,
             logical: Res<ButtonInput<Key>>,
             mut events: MessageReader<KeyboardInput>,
             mut seen: ResMut<HeadlessKeyboardSeen>| {
                seen.ticks += 1;
                if physical.pressed(KeyCode::KeyD) {
                    seen.position += 1;
                }
                seen.physical.push((
                    physical.pressed(KeyCode::KeyD),
                    physical.just_pressed(KeyCode::KeyD),
                    physical.just_released(KeyCode::KeyD),
                ));
                seen.logical.push((
                    logical.pressed(Key::Character("d".into())),
                    logical.just_pressed(Key::Character("d".into())),
                    logical.just_released(Key::Character("d".into())),
                ));
                seen.native_events += events.read().count();
            },
        );
    spawn_target(&mut app);
    let (tx, input) = mpsc::channel();
    let (output, rx) = mpsc::channel();
    install(&mut app, input, output, warp::Pace::AsFastAsPossible);
    (app, tx, rx)
}

#[cfg(feature = "headless-2d")]
fn spawn_headless_keyboard_target(app: &mut App) -> Entity {
    use bevy::camera::{ImageRenderTarget, RenderTarget};

    app.world_mut()
        .spawn((
            Camera2d,
            RenderTarget::Image(ImageRenderTarget {
                handle: Handle::default(),
                scale_factor: 1.5,
            }),
            crate::session::HeadlessCaptureCamera2d,
        ))
        .id()
}

#[cfg(feature = "headless-3d")]
fn spawn_headless_3d_keyboard_target(app: &mut App) -> Entity {
    use bevy::camera::{ImageRenderTarget, PerspectiveProjection, Projection, RenderTarget};

    app.world_mut()
        .spawn((
            Camera3d::default(),
            Camera::default(),
            Projection::Perspective(PerspectiveProjection::default()),
            RenderTarget::Image(ImageRenderTarget {
                handle: Handle::default(),
                scale_factor: 1.5,
            }),
            crate::session::HeadlessCaptureCamera3d,
        ))
        .id()
}

#[cfg(any(feature = "headless-2d", feature = "headless-3d"))]
fn assert_headless_keyboard_edges_and_movement(spawn_target: fn(&mut App) -> Entity) {
    let (mut app, tx, rx) = headless_keyboard_app(spawn_target);
    let send = |id, command: Command| tx.send(protocol::encode(id, &command)).unwrap();
    let response = |id| {
        let message = rx.try_recv().unwrap().0;
        assert_eq!(message.id(), Some(id));
        message
    };

    send(
        1,
        keyboard::Press {
            key: keyboard::Key::D,
        }
        .into(),
    );
    app.update();
    assert!(matches!(response(1), Message::Completed { .. }));
    assert_eq!(app.world().resource::<HeadlessKeyboardSeen>().ticks, 0);
    assert!(
        !app.world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::KeyD)
    );
    send(
        2,
        keyboard::Press {
            key: keyboard::Key::D,
        }
        .into(),
    );
    app.update();
    assert!(matches!(
        response(2),
        Message::Rejected { error, .. } if error.code == "key_already_pressed"
    ));

    warp(&mut app, &tx, 3);
    assert!(matches!(response(3), Message::Completed { .. }));
    send(
        4,
        warp::Start {
            ticks: 3,
            pace: None,
        }
        .into(),
    );
    app.update();
    assert!(matches!(response(4), Message::Completed { .. }));
    {
        let seen = app.world().resource::<HeadlessKeyboardSeen>();
        assert_eq!((seen.ticks, seen.position), (4, 4));
        assert_eq!(seen.physical[0], (true, true, false));
        assert_eq!(seen.logical[0], (true, true, false));
        assert_eq!(seen.physical[3], (true, false, false));
        assert_eq!(seen.logical[3], (true, false, false));
        assert_eq!(seen.native_events, 0);
    }

    send(
        5,
        keyboard::Release {
            key: keyboard::Key::D,
        }
        .into(),
    );
    app.update();
    assert!(matches!(response(5), Message::Completed { .. }));
    assert_eq!(app.world().resource::<HeadlessKeyboardSeen>().position, 4);
    send(
        6,
        keyboard::Release {
            key: keyboard::Key::D,
        }
        .into(),
    );
    app.update();
    assert!(matches!(
        response(6),
        Message::Rejected { error, .. } if error.code == "key_not_pressed"
    ));
    warp(&mut app, &tx, 7);
    assert!(matches!(response(7), Message::Completed { .. }));
    let seen = app.world().resource::<HeadlessKeyboardSeen>();
    assert_eq!((seen.ticks, seen.position), (5, 4));
    assert_eq!(seen.physical[4], (false, false, true));
    assert_eq!(seen.logical[4], (false, false, true));
    assert_eq!(seen.native_events, 0);
}

#[cfg(feature = "headless-2d")]
#[test]
fn headless_2d_keyboard_edges_and_movement_are_applied_only_by_warps() {
    assert_headless_keyboard_edges_and_movement(spawn_headless_keyboard_target);
}

#[cfg(feature = "headless-3d")]
#[test]
fn headless_3d_keyboard_edges_and_movement_are_applied_only_by_warps() {
    assert_headless_keyboard_edges_and_movement(spawn_headless_3d_keyboard_target);
}

#[cfg(feature = "headless-2d")]
#[test]
fn headless_2d_keyboard_requires_exactly_one_explicit_target() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let (tx, input) = mpsc::channel();
    let (output, rx) = mpsc::channel();
    install(&mut app, input, output, warp::Pace::AsFastAsPossible);
    let press = |id| {
        tx.send(protocol::encode(
            id,
            &keyboard::Press {
                key: keyboard::Key::D,
            }
            .into(),
        ))
        .unwrap();
    };
    let rejected = |id| {
        let Message::Rejected {
            request_id, error, ..
        } = rx.try_recv().unwrap().0
        else {
            panic!("headless keyboard target must be rejected")
        };
        assert_eq!(request_id, id);
        assert_eq!(error.code, "keyboard_window_unavailable");
    };

    press(1);
    app.update();
    rejected(1);
    let first = spawn_headless_keyboard_target(&mut app);
    let second = spawn_headless_keyboard_target(&mut app);
    press(2);
    app.update();
    rejected(2);
    app.world_mut().despawn(second);
    press(3);
    app.update();
    assert!(matches!(
        rx.try_recv().unwrap().0,
        Message::Completed { request_id: 3, .. }
    ));
    assert!(app.world().get_entity(first).is_ok());
}

#[cfg(feature = "headless-3d")]
#[test]
fn headless_3d_keyboard_requires_one_suitable_explicit_target() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let (tx, input) = mpsc::channel();
    let (output, rx) = mpsc::channel();
    install(&mut app, input, output, warp::Pace::AsFastAsPossible);
    let press = |id| {
        tx.send(protocol::encode(
            id,
            &keyboard::Press {
                key: keyboard::Key::D,
            }
            .into(),
        ))
        .unwrap();
    };
    let rejected = |id| {
        let Message::Rejected {
            request_id, error, ..
        } = rx.try_recv().unwrap().0
        else {
            panic!("headless 3D keyboard target must be rejected")
        };
        assert_eq!(request_id, id);
        assert_eq!(error.code, "keyboard_window_unavailable");
    };

    press(1);
    app.update();
    rejected(1);

    let target = spawn_headless_3d_keyboard_target(&mut app);
    app.world_mut()
        .entity_mut(target)
        .remove::<crate::session::HeadlessCaptureCamera3d>();
    press(2);
    app.update();
    rejected(2);

    app.world_mut()
        .entity_mut(target)
        .insert(crate::session::HeadlessCaptureCamera3d);
    app.world_mut().get_mut::<Camera>(target).unwrap().is_active = false;
    press(3);
    app.update();
    rejected(3);

    app.world_mut().get_mut::<Camera>(target).unwrap().is_active = true;
    app.world_mut()
        .entity_mut(target)
        .insert(Projection::Orthographic(
            OrthographicProjection::default_3d(),
        ));
    press(4);
    app.update();
    rejected(4);

    app.world_mut()
        .entity_mut(target)
        .insert(Projection::Perspective(PerspectiveProjection::default()));
    let duplicate = spawn_headless_3d_keyboard_target(&mut app);
    press(5);
    app.update();
    rejected(5);

    app.world_mut().despawn(duplicate);
    press(6);
    app.update();
    assert!(matches!(
        rx.try_recv().unwrap().0,
        Message::Completed { request_id: 6, .. }
    ));
}

#[cfg(all(feature = "headless-2d", feature = "headless-3d"))]
#[test]
fn mixed_headless_2d_and_3d_keyboard_targets_fail_closed() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    spawn_headless_keyboard_target(&mut app);
    spawn_headless_3d_keyboard_target(&mut app);
    let (tx, input) = mpsc::channel();
    let (output, rx) = mpsc::channel();
    install(&mut app, input, output, warp::Pace::AsFastAsPossible);

    tx.send(protocol::encode(
        1,
        &keyboard::Press {
            key: keyboard::Key::D,
        }
        .into(),
    ))
    .unwrap();
    app.update();
    assert!(matches!(
        rx.try_recv().unwrap().0,
        Message::Rejected { error, .. } if error.code == "keyboard_window_unavailable"
    ));
}

#[test]
fn independent_virtual_devices_ignore_native_input_without_window_focus() {
    let mut sessions = [app(), app()];
    for (index, (app, window, tx)) in sessions.iter_mut().enumerate() {
        let position = [10. + index as f32 * 20., 15.];
        tx.send(protocol::encode(1, &pointer::MoveTo { position }.into()))
            .unwrap();
        tx.send(protocol::encode(
            2,
            &pointer::Press {
                button: "left".into(),
            }
            .into(),
        ))
        .unwrap();
        native_input(app, *window);
        app.world_mut()
            .write_message(WindowEvent::WindowResized(WindowResized {
                window: *window,
                width: 1280.,
                height: 720.,
            }));
        app.update(); // accept, but no tick
        assert!(
            !app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
        native_input(app, *window);
        warp(app, tx, 3);
        let pointer = app
            .world_mut()
            .query::<(&PointerId, &PointerLocation)>()
            .iter(app.world())
            .find(|(id, _)| id.is_custom())
            .unwrap()
            .1
            .location()
            .unwrap()
            .position;
        assert_eq!(pointer, Vec2::from_array(position));
        assert_eq!(
            app.world()
                .get::<Window>(*window)
                .unwrap()
                .cursor_position(),
            Some(Vec2::new(90., 80.))
        );
        assert!(
            app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyA)
        );
        let seen = app.world().resource::<Seen>();
        assert_eq!((seen.keys, seen.text, seen.resizes), (0, 0, 1));
        native_input(app, *window);
        warp(app, tx, 4);
        assert!(
            app.world()
                .resource::<ButtonInput<MouseButton>>()
                .pressed(MouseButton::Left)
        );
        assert_eq!(app.world().resource::<Seen>().resizes, 1);
    }
    let (first, _, tx) = &mut sessions[0];
    tx.send(protocol::encode(
        5,
        &pointer::Release {
            button: "left".into(),
        }
        .into(),
    ))
    .unwrap();
    warp(first, tx, 6);
    assert!(
        !first
            .world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left)
    );
    assert!(
        sessions[1]
            .0
            .world()
            .resource::<ButtonInput<MouseButton>>()
            .pressed(MouseButton::Left)
    );
}

#[test]
fn keyboards_are_tick_bound_isolated_and_ignore_native_focus_loss() {
    let mut sessions = [app(), app()];
    for (app, window, tx) in &mut sessions {
        tx.send(protocol::encode(
            1,
            &keyboard::Press {
                key: keyboard::Key::A,
            }
            .into(),
        ))
        .unwrap();
        app.update();
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyA)
        );
        assert_eq!(app.world().resource::<Seen>().keys, 0);
        native_input(app, *window);
        warp(app, tx, 2);
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(keys.pressed(KeyCode::KeyA));
        assert!(keys.just_pressed(KeyCode::KeyA));
        for id in 3..6 {
            native_input(app, *window);
            warp(app, tx, id);
            let keys = app.world().resource::<ButtonInput<KeyCode>>();
            assert!(keys.pressed(KeyCode::KeyA));
            assert!(!keys.just_pressed(KeyCode::KeyA));
            assert!(!keys.just_released(KeyCode::KeyA));
        }
        let seen = app.world().resource::<Seen>();
        assert_eq!(seen.keys, 1);
        assert!(seen.keyboard_events[0].text.is_none());
        assert!(!seen.keyboard_events[0].repeat);
    }
    let (first, _, tx) = &mut sessions[0];
    tx.send(protocol::encode(
        6,
        &keyboard::Release {
            key: keyboard::Key::A,
        }
        .into(),
    ))
    .unwrap();
    first.update();
    assert!(
        first
            .world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::KeyA)
    );
    warp(first, tx, 7);
    let keys = first.world().resource::<ButtonInput<KeyCode>>();
    assert!(!keys.pressed(KeyCode::KeyA));
    assert!(keys.just_released(KeyCode::KeyA));
    assert_eq!(first.world().resource::<Seen>().keys, 2);
    assert!(
        sessions[1]
            .0
            .world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::KeyA)
    );
}

#[test]
fn queued_edges_and_shared_logical_modifiers_preserve_their_lifetimes() {
    let (mut app, _, tx) = app();
    let press = |id, key| {
        tx.send(protocol::encode(id, &keyboard::Press { key }.into()))
            .unwrap()
    };
    let release = |id, key| {
        tx.send(protocol::encode(id, &keyboard::Release { key }.into()))
            .unwrap()
    };
    press(1, keyboard::Key::A);
    release(2, keyboard::Key::A);
    press(3, keyboard::Key::ShiftLeft);
    press(4, keyboard::Key::ShiftRight);
    warp(&mut app, &tx, 5);
    let keys = app.world().resource::<ButtonInput<KeyCode>>();
    assert!(!keys.pressed(KeyCode::KeyA));
    assert!(keys.just_pressed(KeyCode::KeyA));
    assert!(keys.just_released(KeyCode::KeyA));
    assert!(keys.all_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]));
    assert!(
        app.world()
            .resource::<ButtonInput<Key>>()
            .pressed(Key::Shift)
    );
    let seen = &app.world().resource::<Seen>().keyboard_events;
    assert_eq!(
        seen.iter()
            .map(|event| (event.key_code, event.state))
            .collect::<Vec<_>>(),
        vec![
            (KeyCode::KeyA, ButtonState::Pressed),
            (KeyCode::KeyA, ButtonState::Released),
            (KeyCode::ShiftLeft, ButtonState::Pressed),
            (KeyCode::ShiftRight, ButtonState::Pressed)
        ]
    );
    release(6, keyboard::Key::ShiftLeft);
    warp(&mut app, &tx, 7);
    let logical = app.world().resource::<ButtonInput<Key>>();
    assert!(logical.pressed(Key::Shift));
    assert!(!logical.just_released(Key::Shift));
    assert!(!logical.just_pressed(Key::Shift));
    release(8, keyboard::Key::ShiftRight);
    warp(&mut app, &tx, 9);
    let logical = app.world().resource::<ButtonInput<Key>>();
    assert!(!logical.pressed(Key::Shift));
    assert!(logical.just_released(Key::Shift));
}

#[cfg(feature = "ui")]
#[test]
fn text_is_applied_by_bevy_only_on_ticks_and_stays_in_its_session() {
    use crate::command::input::text;
    use bevy::{
        asset::AssetPlugin,
        input_focus::InputFocus,
        text::{EditableText, TextPlugin},
    };
    let mut sessions = [app(), app()];
    let mut fields = Vec::new();
    for (index, (app, window, tx)) in sessions.iter_mut().enumerate() {
        app.add_plugins((AssetPlugin::default(), TextPlugin));
        let field = app.world_mut().spawn(EditableText::default()).id();
        fields.push(field);
        app.insert_resource(InputFocus::from_entity(field));
        let text = if index == 0 { "Grüße 🦜" } else { "東京" };
        tx.send(protocol::encode(1, &text::Input::new(text).into()))
            .unwrap();
        tx.send(protocol::encode(2, &text::Input::new("!").into()))
            .unwrap();
        app.update();
        let editable = app.world().get::<EditableText>(field).unwrap();
        assert_eq!(editable.value().to_string(), "");
        assert!(editable.pending_edits.is_empty());
        native_input(app, *window);
    }
    let (first, _, tx) = &mut sessions[0];
    warp(first, tx, 3);
    assert_eq!(
        first
            .world()
            .get::<EditableText>(fields[0])
            .unwrap()
            .value()
            .to_string(),
        "Grüße 🦜!"
    );
    assert_eq!(first.world().resource::<Seen>().text, 0);
    assert_eq!(
        sessions[1]
            .0
            .world()
            .get::<EditableText>(fields[1])
            .unwrap()
            .value()
            .to_string(),
        ""
    );
    let (second, _, tx) = &mut sessions[1];
    warp(second, tx, 3);
    assert_eq!(
        second
            .world()
            .get::<EditableText>(fields[1])
            .unwrap()
            .value()
            .to_string(),
        "東京!"
    );
    assert_eq!(second.world().resource::<Seen>().text, 0);
    warp(second, tx, 4);
    assert_eq!(
        second
            .world()
            .get::<EditableText>(fields[1])
            .unwrap()
            .value()
            .to_string(),
        "東京!"
    );
}
