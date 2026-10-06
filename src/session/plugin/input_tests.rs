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
