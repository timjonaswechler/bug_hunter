use bevy::window::WindowResolution;
use bevy::{prelude::*, time::Fixed};
use bevy_test_apps::composition;

const SURFACE_WIDTH: u32 = 640;
const SURFACE_HEIGHT: u32 = 360;
const FIXED_STEP: std::time::Duration = std::time::Duration::from_millis(10);
const TIMER_PERIOD: std::time::Duration = std::time::Duration::from_millis(40);

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
struct SessionObservation {
    updates: u64,
    fixed_updates: u64,
    timer_finishes: u64,
    pointer_presses: u64,
    key_a_held: bool,
    key_a_presses: u64,
    key_a_releases: u64,
}

#[derive(Resource)]
struct UpdateTimer(Timer);

fn main() {
    build_app().run();
}

fn build_app() -> App {
    let mut app = App::new();
    composition::rendered(
        &mut app,
        Window {
            title: "Logical state test".into(),
            resolution: WindowResolution::new(SURFACE_WIDTH, SURFACE_HEIGHT)
                .with_scale_factor_override(1.0),
            resizable: false,
            focused: !cfg!(feature = "slice"),
            ..default()
        },
    );
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .set_timestep(FIXED_STEP);
    app.insert_resource(ClearColor(Color::BLACK))
        .register_type::<SessionObservation>()
        .insert_resource(UpdateTimer(Timer::new(TIMER_PERIOD, TimerMode::Repeating)))
        .add_systems(Startup, setup)
        .add_systems(Update, record_update)
        .add_systems(FixedUpdate, record_fixed_update);
    #[cfg(feature = "slice")]
    if std::env::args().any(|arg| arg == "--investigation") {
        install_investigation(&mut app);
    }
    #[cfg(feature = "slice")]
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(20),
    ))
    .add_plugins(woodpecker::session::Plugin);
    app
}

fn setup(mut commands: Commands) {
    commands.spawn((Name::new("logical-camera"), Camera2d));
    commands.spawn((Name::new("logical-state"), SessionObservation::default()));
    commands
        .spawn((
            Name::new("logical-button"),
            Button,
            // A known fixture color makes technical captures distinguishable
            // from an empty readback without changing the observed state.
            BackgroundColor(Color::srgb(0.0, 1.0, 0.0)),
            Node {
                position_type: PositionType::Absolute,
                left: px(220),
                top: px(130),
                width: px(200),
                height: px(100),
                ..default()
            },
        ))
        .observe(
            |_: On<Pointer<Press>>, mut state: Single<&mut SessionObservation>| {
                state.pointer_presses += 1;
            },
        );
}

fn record_update(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut timer: ResMut<UpdateTimer>,
    mut state: Single<&mut SessionObservation>,
) {
    state.updates += 1;
    state.key_a_held = keys.pressed(KeyCode::KeyA);
    if keys.just_pressed(KeyCode::KeyA) {
        state.key_a_presses += 1;
    }
    if keys.just_released(KeyCode::KeyA) {
        state.key_a_releases += 1;
    }
    timer.0.tick(time.delta());
    state.timer_finishes += u64::from(timer.0.times_finished_this_tick());
}

fn record_fixed_update(mut state: Single<&mut SessionObservation>) {
    state.fixed_updates += 1;
}

// Opt-in acceptance fixture only: the recorded B press reproduces one tracing
// failure on its first explicit tick. Normal/native logical_state is unchanged.
#[cfg(feature = "slice")]
#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
struct Investigation {
    process_id: u32,
    ticks: u64,
    failures: u64,
}

#[cfg(feature = "slice")]
fn install_investigation(app: &mut App) {
    app.insert_resource(Investigation {
        process_id: std::process::id(),
        ..default()
    })
    .register_type::<Investigation>()
    .add_systems(Update, investigation_tick);
}

#[cfg(feature = "slice")]
fn investigation_tick(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<Investigation>) {
    state.ticks += 1;
    if keys.just_pressed(KeyCode::KeyB) {
        state.failures += 1;
        error!(target: "investigation_fixture", "recorded B press reproduced the fixture failure");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "slice")]
    #[test]
    fn investigation_failure_requires_a_tick_and_does_not_repeat_while_held() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        install_investigation(&mut app);
        app.world_mut().run_schedule(Update);
        assert_eq!(app.world().resource::<Investigation>().failures, 0);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyB);
        assert_eq!(app.world().resource::<Investigation>().failures, 0);
        app.world_mut().run_schedule(Update);
        assert_eq!(app.world().resource::<Investigation>().failures, 1);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut().run_schedule(Update);
        assert_eq!(app.world().resource::<Investigation>().failures, 1);
        assert_eq!(app.world().resource::<Investigation>().ticks, 3);
    }

    #[test]
    fn logical_state_starts_with_an_empty_observation() {
        let state = SessionObservation::default();
        assert_eq!(state.updates, 0);
        assert_eq!(state.fixed_updates, 0);
        assert_eq!(state.timer_finishes, 0);
        assert_eq!(state.pointer_presses, 0);
        assert!(!state.key_a_held);
        assert_eq!(FIXED_STEP, std::time::Duration::from_millis(10));
    }
}
