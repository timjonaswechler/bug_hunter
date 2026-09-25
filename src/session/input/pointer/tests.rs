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
        assert_eq!(
            state.location,
            Some((
                Target::Window {
                    entity: window,
                    logical_size: Vec2::new(100., 50.),
                },
                Vec2::new(12., 17.),
            ))
        );
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
fn window_resize_preserves_target_identity_but_revalidates_current_bounds() {
    let (mut world, window) = world();
    let mut state = State::default();
    state.move_to(&world, [40., 20.]).unwrap();

    world
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set(400., 200.);
    state.move_by(&world, [10., 5.]).unwrap();
    state.button(&world, "left", true).unwrap();
    state.scroll(&world, [1., -2.]).unwrap();

    world
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set(120., 80.);
    state.move_by(&world, [1., 1.]).unwrap();
    state.button(&world, "left", false).unwrap();
    state.scroll(&world, [-3., 4.]).unwrap();

    let accepted = state.pending.len();
    world
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set(1., 1.);
    for result in [
        state.move_by(&world, [0., 0.]),
        state.button(&world, "left", true),
        state.scroll(&world, [0., 0.]),
    ] {
        assert_eq!(result.unwrap_err().code, "pointer_location_unavailable");
    }
    assert_eq!(state.pending.len(), accepted);

    assert!(state.pending.iter().all(|event| {
        matches!(
            event.location.target,
            NormalizedRenderTarget::Window(target) if target.entity() == window
        )
    }));
    assert!(matches!(
        state.pending[1].action,
        PointerAction::Move { delta } if delta == Vec2::new(10., 5.)
    ));
    assert!(matches!(
        state.pending[2].action,
        PointerAction::Press(PointerButton::Primary)
    ));
    assert!(matches!(
        state.pending[3].action,
        PointerAction::Scroll { x: 1., y: -2., .. }
    ));
    assert!(matches!(
        state.pending[4].action,
        PointerAction::Move { delta } if delta == Vec2::ONE
    ));
    assert!(matches!(
        state.pending[5].action,
        PointerAction::Release(PointerButton::Primary)
    ));
    assert!(matches!(
        state.pending[6].action,
        PointerAction::Scroll { x: -3., y: 4., .. }
    ));

    world.init_resource::<Messages<PointerInput>>();
    world.init_resource::<Messages<MouseButtonInput>>();
    world.init_resource::<Messages<MouseWheel>>();
    state.flush(&mut world);
    assert_eq!(world.resource::<Messages<PointerInput>>().len(), 7);
    assert_eq!(world.resource::<Messages<MouseButtonInput>>().len(), 2);
    assert_eq!(world.resource::<Messages<MouseWheel>>().len(), 2);
}

#[test]
fn window_entity_change_or_removal_invalidates_the_accepted_location() {
    let (mut world, window) = world();
    let mut state = State::default();
    state.move_to(&world, [12., 17.]).unwrap();

    world.despawn(window);
    let replacement = world
        .spawn((
            Window {
                resolution: bevy::window::WindowResolution::new(200, 100)
                    .with_scale_factor_override(2.0),
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    assert_ne!(replacement, window);
    assert_eq!(
        state.move_by(&world, [0., 0.]).unwrap_err().code,
        "pointer_location_unavailable"
    );
    assert_eq!(
        state.button(&world, "left", true).unwrap_err().code,
        "pointer_location_unavailable"
    );
    assert_eq!(
        state.scroll(&world, [0., 0.]).unwrap_err().code,
        "pointer_location_unavailable"
    );

    world.despawn(replacement);
    assert_eq!(
        state.move_by(&world, [0., 0.]).unwrap_err().code,
        "pointer_window_unavailable"
    );
    assert_eq!(
        state.button(&world, "left", true).unwrap_err().code,
        "pointer_location_unavailable"
    );
    assert_eq!(
        state.scroll(&world, [0., 0.]).unwrap_err().code,
        "pointer_window_unavailable"
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

#[cfg(feature = "headless-2d")]
fn image_world(scale_factor: f32) -> (World, Entity, ImageRenderTarget) {
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo};
    use bevy::render::render_resource::TextureFormat;

    let mut world = World::new();
    world.init_resource::<Assets<Image>>();
    let handle = world
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            800,
            600,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let target = ImageRenderTarget {
        handle,
        scale_factor,
    };
    let camera = world
        .spawn((
            Camera2d,
            Camera {
                computed: ComputedCameraValues {
                    target_info: Some(RenderTargetInfo {
                        physical_size: UVec2::new(800, 600),
                        scale_factor,
                    }),
                    ..default()
                },
                ..default()
            },
            RenderTarget::Image(target.clone()),
            crate::session::HeadlessCaptureCamera2d,
        ))
        .id();
    (world, camera, target)
}

#[cfg(feature = "headless-2d")]
#[test]
fn image_coordinates_are_logical_and_keep_the_scaled_image_identity() {
    let (mut world, camera, target) = image_world(2.0);
    let mut state = State::default();

    state.move_to(&world, [399.5, 299.5]).unwrap();
    assert_eq!(
        state.move_to(&world, [400.0, 0.0]).unwrap_err().code,
        "pointer_position_out_of_bounds"
    );
    state.button(&world, "left", true).unwrap();
    let events = state.pending.clone();
    assert!(
        events.iter().all(|event| {
            event.location.target == NormalizedRenderTarget::Image(target.clone())
        })
    );

    world.init_resource::<Messages<PointerInput>>();
    world.init_resource::<Messages<MouseButtonInput>>();
    world.init_resource::<Messages<MouseWheel>>();
    state.flush(&mut world);
    assert_eq!(
        world.resource::<Messages<PointerInput>>().len(),
        events.len()
    );
    assert!(world.resource::<Messages<MouseButtonInput>>().is_empty());
    assert!(world.resource::<Messages<MouseWheel>>().is_empty());

    {
        let mut render_target = world.get_mut::<RenderTarget>(camera).unwrap();
        let RenderTarget::Image(changed_target) = &mut *render_target else {
            panic!("image camera target changed kind")
        };
        changed_target.scale_factor = 1.0;
    }
    world
        .get_mut::<Camera>(camera)
        .unwrap()
        .computed
        .target_info
        .as_mut()
        .unwrap()
        .scale_factor = 1.0;
    assert_eq!(
        state.move_by(&world, [0.0, 0.0]).unwrap_err().code,
        "pointer_location_unavailable"
    );
}

#[cfg(feature = "headless-2d")]
#[test]
fn image_pointer_rejects_uninitialized_unsuitable_and_ambiguous_targets() {
    let (mut world, camera, _) = image_world(1.5);
    let mut state = State::default();

    world
        .get_mut::<Camera>(camera)
        .unwrap()
        .computed
        .target_info = None;
    assert_eq!(
        state.move_to(&world, [1.0, 1.0]).unwrap_err().code,
        "pointer_window_unavailable"
    );
    world
        .get_mut::<Camera>(camera)
        .unwrap()
        .computed
        .target_info = Some(bevy::camera::RenderTargetInfo {
        physical_size: UVec2::new(800, 600),
        scale_factor: 1.5,
    });
    world
        .entity_mut(camera)
        .remove::<crate::session::HeadlessCaptureCamera2d>();
    assert_eq!(
        state.move_to(&world, [1.0, 1.0]).unwrap_err().code,
        "pointer_window_unavailable"
    );
    world
        .entity_mut(camera)
        .insert(crate::session::HeadlessCaptureCamera2d);
    state.move_to(&world, [1.0, 1.0]).unwrap();

    let duplicate_target = world.get::<RenderTarget>(camera).unwrap().clone();
    world.spawn((Camera2d, duplicate_target));
    assert_eq!(
        state.move_to(&world, [1.0, 1.0]).unwrap_err().code,
        "pointer_window_unavailable"
    );
    assert_eq!(
        state.button(&world, "left", true).unwrap_err().code,
        "pointer_location_unavailable"
    );
}
