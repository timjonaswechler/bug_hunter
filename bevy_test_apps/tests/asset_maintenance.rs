//! CPU-only evidence for Bevy 0.19.1 asset maintenance while game schedules are stopped.
use bevy::{
    asset::{AssetApp, AssetEvent, AssetPlugin, AssetServer, Assets, handle_internal_asset_events},
    ecs::message::Messages,
    prelude::*,
};
use std::time::Duration;

#[path = "asset_maintenance/schedule.rs"]
mod schedule;

#[derive(Asset, TypePath, Debug)]
struct TestAsset(u32);

#[derive(Resource, Default, Debug, PartialEq, Eq)]
struct ScheduleRuns {
    pre_update: u32,
    update: u32,
    post_update: u32,
    fixed_update: u32,
}

fn fixture() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<TestAsset>()
        .init_resource::<ScheduleRuns>()
        .add_systems(PreUpdate, |mut runs: ResMut<ScheduleRuns>| {
            runs.pre_update += 1;
        })
        .add_systems(Update, |mut runs: ResMut<ScheduleRuns>| {
            runs.update += 1;
        })
        .add_systems(PostUpdate, |mut runs: ResMut<ScheduleRuns>| {
            runs.post_update += 1;
        })
        .add_systems(FixedUpdate, |mut runs: ResMut<ScheduleRuns>| {
            runs.fixed_update += 1;
        });
    app
}

fn run_public_maintenance(app: &mut App) {
    handle_internal_asset_events(app.world_mut());
    app.world_mut()
        .run_system_cached(Assets::<TestAsset>::track_assets)
        .unwrap();
}

fn assert_simulation_stopped(app: &App, elapsed: Duration) {
    assert_eq!(
        app.world().resource::<ScheduleRuns>(),
        &ScheduleRuns::default()
    );
    assert_eq!(app.world().resource::<Time<Virtual>>().elapsed(), elapsed);
}

#[test]
fn public_internal_handler_finalizes_a_real_load_once_without_game_schedules() {
    let mut app = fixture();
    let elapsed = app.world().resource::<Time<Virtual>>().elapsed();
    let handle = app.world().resource::<AssetServer>().add(TestAsset(17));

    // The first call stores the real AssetServer result; a second call also covers
    // a LoadedWithDependencies event queued while handling the first event.
    run_public_maintenance(&mut app);
    run_public_maintenance(&mut app);

    assert_eq!(
        app.world()
            .resource::<Assets<TestAsset>>()
            .get(&handle)
            .unwrap()
            .0,
        17
    );
    let events = app
        .world()
        .resource::<Messages<AssetEvent<TestAsset>>>()
        .iter_current_update_messages()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        events,
        vec![AssetEvent::LoadedWithDependencies { id: handle.id() }]
    );

    for _ in 0..4 {
        run_public_maintenance(&mut app);
    }
    assert_eq!(
        app.world()
            .resource::<Messages<AssetEvent<TestAsset>>>()
            .len(),
        1,
        "repeated maintenance must not duplicate the dependency event"
    );
    assert_simulation_stopped(&app, elapsed);
}

#[test]
fn typed_message_updates_define_the_dependency_event_lifetime() {
    let mut app = fixture();
    let elapsed = app.world().resource::<Time<Virtual>>().elapsed();
    let handle = app.world().resource::<AssetServer>().add(TestAsset(23));
    run_public_maintenance(&mut app);
    run_public_maintenance(&mut app);

    let mut messages = app
        .world_mut()
        .resource_mut::<Messages<AssetEvent<TestAsset>>>();
    assert!(
        messages
            .iter_current_update_messages()
            .any(|event| event == &AssetEvent::LoadedWithDependencies { id: handle.id() })
    );
    messages.update();
    assert_eq!(messages.len(), 1, "one update boundary retains the event");
    messages.update();
    assert!(messages.is_empty(), "the second boundary expires the event");
    assert_simulation_stopped(&app, elapsed);
}

#[test]
fn public_maintenance_cannot_publish_queued_collection_events() {
    let mut app = fixture();
    let elapsed = app.world().resource::<Time<Virtual>>().elapsed();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(1));
    app.world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .get_mut(&handle)
        .unwrap()
        .0 = 2;
    assert_eq!(
        app.world_mut()
            .resource_mut::<Assets<TestAsset>>()
            .remove(&handle)
            .unwrap()
            .0,
        2
    );

    for _ in 0..3 {
        run_public_maintenance(&mut app);
    }
    assert!(
        app.world()
            .resource::<Messages<AssetEvent<TestAsset>>>()
            .is_empty(),
        "handle_internal_asset_events and track_assets do not flush Assets' private queue"
    );
    assert_simulation_stopped(&app, elapsed);
}

#[test]
fn public_tracking_removes_a_dropped_server_asset_without_game_schedules() {
    let mut app = fixture();
    let elapsed = app.world().resource::<Time<Virtual>>().elapsed();
    let handle = app.world().resource::<AssetServer>().add(TestAsset(31));
    let id = handle.id();
    run_public_maintenance(&mut app);
    run_public_maintenance(&mut app);
    app.world_mut()
        .resource_mut::<Messages<AssetEvent<TestAsset>>>()
        .clear();

    drop(handle);
    run_public_maintenance(&mut app);
    assert!(
        app.world()
            .resource::<Assets<TestAsset>>()
            .get(id)
            .is_none(),
        "track_assets must apply the real strong-handle drop"
    );
    run_public_maintenance(&mut app);
    assert!(
        app.world()
            .resource::<Messages<AssetEvent<TestAsset>>>()
            .is_empty(),
        "Unused and Removed stay queued without the private publisher"
    );
    assert_simulation_stopped(&app, elapsed);
}
