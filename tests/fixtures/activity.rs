//! Renderer-free large-output fixture; one explicit tick produces one report.
use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*};
use std::time::Duration;

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
struct Counter {
    ticks: u64,
    process_id: u32,
}

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
struct Medium {
    text: String,
}

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
struct Large {
    text: String,
}

fn main() {
    let continuous = std::env::args().any(|argument| argument == "--continuous-failures");
    App::new()
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(2))))
        .add_plugins(LogPlugin {
            custom_layer: woodpecker::session::tracing_error_layer,
            ..default()
        })
        .insert_resource(Counter {
            ticks: 0,
            process_id: std::process::id(),
        })
        .register_type::<Counter>()
        .insert_resource(Medium {
            text: "m".repeat(2 * 1024 * 1024),
        })
        .register_type::<Medium>()
        .insert_resource(Large {
            text: "x".repeat(5 * 1024 * 1024),
        })
        .register_type::<Large>()
        .add_systems(Update, move |mut counter: ResMut<Counter>| {
            counter.ticks += 1;
            if continuous || counter.ticks == 1 {
                error!(target: "activity_fixture", "large report snapshot fixture");
            }
        })
        .add_plugins(woodpecker::session::Plugin)
        .run();
}
