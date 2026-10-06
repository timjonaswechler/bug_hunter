use super::*;
use bevy::window::PrimaryWindow;

fn world() -> (World, Entity) {
    let mut world = World::new();
    let window = world
        .spawn((
            Window {
                resolution: bevy::window::WindowResolution::new(200, 100)
                    .with_scale_factor_override(2.0),
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    (world, window)
}

#[test]
fn coordinates_are_logical_finite_and_rejection_preserves_accepted_state() {
    let (world, window) = world();
    let mut state = State::default();
    assert_eq!(
        state.move_by(&world, [0., 0.]).unwrap_err().code,
        "pointer_location_unavailable"
    );
    for position in [[f32::NAN, 0.], [0., f32::INFINITY]] {
        assert_eq!(
            state.move_to(&world, position).unwrap_err().code,
            "invalid_arguments"
        );
    }
    state.move_to(&world, [10., 20.]).unwrap();
    state.move_by(&world, [2., -3.]).unwrap();
    for position in [[-1., 0.], [0., -1.], [100., 0.], [0., 50.]] {
        assert_eq!(
            state.move_to(&world, position).unwrap_err().code,
            "pointer_position_out_of_bounds"
        );
        assert_eq!(state.location, Some((window, Vec2::new(12., 17.))));
        assert_eq!(state.pending.len(), 2);
    }
    assert_eq!(
        state.move_by(&world, [100., 0.]).unwrap_err().code,
        "pointer_position_out_of_bounds"
    );
    assert!(
        world
            .get::<Window>(window)
            .unwrap()
            .cursor_position()
            .is_none()
    );
}

#[test]
fn buttons_scroll_windows_and_isolation() {
    let (mut world, window) = world();
    let mut state = State::default();
    assert_eq!(
        state.button(&world, "other", true).unwrap_err().code,
        "invalid_pointer_button"
    );
    assert_eq!(
        state.button(&world, "left", true).unwrap_err().code,
        "pointer_location_unavailable"
    );
    assert_eq!(
        state.scroll(&world, [0., 0.]).unwrap_err().code,
        "pointer_location_unavailable"
    );
    state.move_to(&world, [12., 17.]).unwrap();
    for button in ["left", "right", "middle"] {
        assert_eq!(
            state.button(&world, button, false).unwrap_err().code,
            "pointer_button_not_pressed"
        );
        state.button(&world, button, true).unwrap();
        assert_eq!(
            state.button(&world, button, true).unwrap_err().code,
            "pointer_button_already_pressed"
        );
        state.button(&world, button, false).unwrap();
    }
    state.scroll(&world, [-2., 3.]).unwrap();
    state.scroll(&world, [0., 0.]).unwrap();
    let len = state.pending.len();
    assert_eq!(
        state.scroll(&world, [f32::INFINITY, 0.]).unwrap_err().code,
        "invalid_arguments"
    );
    assert_eq!(state.pending.len(), len);
    let PointerAction::Scroll { unit, x, y, .. } = &state.pending[len - 2].action else {
        panic!("scroll")
    };
    assert_eq!(*unit, MouseScrollUnit::Line);
    assert_eq!([*x, *y], [-2., 3.]);
    assert_eq!(
        State::default()
            .button(&world, "left", true)
            .unwrap_err()
            .code,
        "pointer_location_unavailable"
    );
    let second = world.spawn((Window::default(), PrimaryWindow)).id();
    assert_eq!(
        state.move_to(&world, [0., 0.]).unwrap_err().code,
        "pointer_window_unavailable"
    );
    assert_eq!(
        state.scroll(&world, [0., 0.]).unwrap_err().code,
        "pointer_window_unavailable"
    );
    assert_eq!(
        state.button(&world, "left", true).unwrap_err().code,
        "pointer_location_unavailable"
    );
    world.despawn(second);
    world
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set(1., 1.);
    assert_eq!(
        state.button(&world, "left", true).unwrap_err().code,
        "pointer_location_unavailable"
    );
    world.despawn(window);
    assert_eq!(
        state.move_to(&world, [0., 0.]).unwrap_err().code,
        "pointer_window_unavailable"
    );
    world.spawn((Window::default(), PrimaryWindow));
    assert_eq!(
        state.move_by(&world, [0., 0.]).unwrap_err().code,
        "pointer_location_unavailable"
    );
}
