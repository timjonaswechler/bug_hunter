//! Headless acceptance application. Simulation time belongs to this application.
use bevy::{app::ScheduleRunnerPlugin, prelude::*, time::TimeUpdateStrategy};
use std::time::Duration;

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
struct Counter {
    ticks: u64,
    process_id: u32,
}

fn main() {
    if let Some(delay) = std::env::args().nth(1) {
        std::thread::sleep(Duration::from_millis(
            delay.parse().expect("delay in milliseconds"),
        ));
    }
    App::new()
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(1))))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )))
        .insert_resource(Counter {
            ticks: 0,
            process_id: std::process::id(),
        })
        .register_type::<Counter>()
        .add_systems(Update, |mut counter: ResMut<Counter>| counter.ticks += 1)
        .add_plugins(woodpecker::session::Plugin)
        .run();
}
