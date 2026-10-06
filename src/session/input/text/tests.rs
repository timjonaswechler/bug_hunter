use super::*;
use bevy::window::PrimaryWindow;

#[test]
fn rejects_oversized_utf8_and_unavailable_windows_before_focus() {
    let mut state = State::default();
    let mut world = World::new();
    for text in ["x".repeat(MAX_BYTES + 1), "🦜".repeat(MAX_BYTES / 4 + 1)] {
        assert_eq!(
            state.input(&world, Input::new(text)).unwrap_err().code,
            "text_too_large"
        );
    }
    assert_eq!(
        state.input(&world, Input::new("")).unwrap_err().code,
        "text_window_unavailable"
    );
    world.spawn(PrimaryWindow); // A marker without Window is not a usable window.
    assert_eq!(
        state.input(&world, Input::new("a")).unwrap_err().code,
        "text_window_unavailable"
    );
    world.spawn((Window::default(), PrimaryWindow));
    let duplicate = world.spawn((Window::default(), PrimaryWindow)).id();
    assert_eq!(
        state.input(&world, Input::new("a")).unwrap_err().code,
        "text_window_unavailable"
    );
    world.despawn(duplicate);
    assert_eq!(
        state.input(&world, Input::new("a")).unwrap_err().code,
        "text_focus_unavailable"
    );
}

#[cfg(feature = "ui")]
#[test]
fn validates_focus_and_queues_exact_utf8_limit_without_application_mutation() {
    let mut state = State::default();
    let mut world = World::new();
    world.spawn((
        Window {
            focused: false,
            ..default()
        },
        PrimaryWindow,
    ));
    world.init_resource::<InputFocus>();
    assert_eq!(
        state.input(&world, Input::new("a")).unwrap_err().code,
        "text_focus_unavailable"
    );
    let ordinary = world.spawn_empty().id();
    world.insert_resource(InputFocus::from_entity(ordinary));
    assert_eq!(
        state.input(&world, Input::new("a")).unwrap_err().code,
        "text_focus_unavailable"
    );
    world.despawn(ordinary);
    assert_eq!(
        state.input(&world, Input::new("a")).unwrap_err().code,
        "text_focus_unavailable"
    );
    let field = world.spawn(EditableText::default()).id();
    world.insert_resource(InputFocus::from_entity(field));
    let changed = world
        .entity(field)
        .get_change_ticks::<EditableText>()
        .unwrap()
        .changed;
    let text = "🦜".repeat(MAX_BYTES / 4);
    state.input(&world, Input::new(&text)).unwrap();
    state.input(&world, Input::new("")).unwrap();
    assert!(state.input(&world, Input::new(format!("{text}x"))).is_err());
    assert_eq!(state.pending.len(), 2);
    assert_eq!(
        world
            .get::<EditableText>(field)
            .unwrap()
            .value()
            .to_string(),
        ""
    );
    assert!(
        world
            .get::<EditableText>(field)
            .unwrap()
            .pending_edits
            .is_empty()
    );
    assert_eq!(
        world
            .entity(field)
            .get_change_ticks::<EditableText>()
            .unwrap()
            .changed,
        changed
    );
    state.flush(&mut world);
    assert_eq!(
        world.get::<EditableText>(field).unwrap().pending_edits,
        vec![
            TextEdit::ImeCommit { value: text.into() },
            TextEdit::ImeCommit { value: "".into() }
        ]
    );
    assert_eq!(
        world
            .get::<EditableText>(field)
            .unwrap()
            .value()
            .to_string(),
        ""
    );
    state.flush(&mut world);
    assert_eq!(
        world
            .get::<EditableText>(field)
            .unwrap()
            .pending_edits
            .len(),
        2
    );
}

#[cfg(feature = "ui")]
#[test]
fn focus_changes_and_despawns_do_not_redirect_accepted_text() {
    let mut state = State::default();
    let mut world = World::new();
    world.spawn((Window::default(), PrimaryWindow));
    let first = world.spawn(EditableText::default()).id();
    let second = world.spawn(EditableText::default()).id();
    world.insert_resource(InputFocus::from_entity(first));
    state.input(&world, Input::new("first")).unwrap();
    world.insert_resource(InputFocus::from_entity(second));
    state.input(&world, Input::new("second")).unwrap();
    state.flush(&mut world);
    assert_eq!(
        world.get::<EditableText>(first).unwrap().pending_edits,
        vec![TextEdit::ImeCommit {
            value: "first".into()
        }]
    );
    assert_eq!(
        world.get::<EditableText>(second).unwrap().pending_edits,
        vec![TextEdit::ImeCommit {
            value: "second".into()
        }]
    );
    state.input(&world, Input::new("discarded")).unwrap();
    world.despawn(second);
    let replacement = world.spawn(EditableText::default()).id();
    world.insert_resource(InputFocus::from_entity(replacement));
    state.flush(&mut world);
    assert!(
        world
            .get::<EditableText>(replacement)
            .unwrap()
            .pending_edits
            .is_empty()
    );
}
