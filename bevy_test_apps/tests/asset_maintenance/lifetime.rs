//! CPU evidence for independent message readers and the two clocks.
//! The extraction reader is a stand-in using the same MessageReader primitive;
//! these tests do NOT execute RenderApp or acknowledge GPU preparation.
use super::*;
use bevy::{ecs::message::MessageRegistry, time::TimeUpdateStrategy};

#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ReadForExtraction;

#[derive(Resource, Default)]
struct ExtractionReads(Vec<AssetEvent<TestAsset>>);

#[derive(Resource, Default)]
struct GameReads(Vec<AssetEvent<TestAsset>>);

fn readers_fixture() -> (App, Vec<Publisher>) {
    let mut app = fixture();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        20,
    )))
    .init_resource::<ExtractionReads>()
    .init_resource::<GameReads>()
    .add_systems(
        ReadForExtraction,
        |mut events: MessageReader<AssetEvent<TestAsset>>, mut reads: ResMut<ExtractionReads>| {
            reads.0.extend(events.read().copied());
        },
    )
    .add_systems(
        Update,
        |mut events: MessageReader<AssetEvent<TestAsset>>, mut reads: ResMut<GameReads>| {
            reads.0.extend(events.read().copied());
        },
    );
    app.finish();
    app.cleanup();
    let publishers = isolate_publishers(&mut app).unwrap();
    (app, publishers)
}

#[test]
fn idle_extraction_reads_do_not_consume_the_next_game_read() {
    let (mut app, publishers) = readers_fixture();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(1));
    for _ in 0..64 {
        publish(&mut app, &publishers);
        app.world_mut().run_schedule(ReadForExtraction);
    }
    let expected = vec![AssetEvent::Added { id: handle.id() }];
    assert_eq!(app.world().resource::<ExtractionReads>().0, expected);
    assert!(app.world().resource::<GameReads>().0.is_empty());
    assert_eq!(
        app.world()
            .resource::<Messages<AssetEvent<TestAsset>>>()
            .len(),
        1
    );
    assert_simulation_stopped(&app, Duration::ZERO);

    // One explicit CPU game step, not idle maintenance.
    app.update();
    assert_eq!(app.world().resource::<GameReads>().0, expected);
    app.world_mut().run_schedule(ReadForExtraction);
    assert_eq!(app.world().resource::<ExtractionReads>().0, expected);
}

#[test]
fn aging_after_extraction_alone_loses_the_event_for_the_game() {
    let (mut app, publishers) = readers_fixture();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(2));
    publish(&mut app, &publishers);
    app.world_mut().run_schedule(ReadForExtraction);
    assert_eq!(
        app.world().resource::<ExtractionReads>().0,
        vec![AssetEvent::Added { id: handle.id() }]
    );
    // Negative control: a proposed idle updater must not do this merely because
    // an extraction reader has advanced. No simulation has run yet.
    for _ in 0..2 {
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<TestAsset>>>()
            .update();
    }
    assert_simulation_stopped(&app, Duration::ZERO);
    app.update();
    assert!(app.world().resource::<GameReads>().0.is_empty());
}

#[test]
fn multiple_game_steps_before_extraction_can_expire_an_unread_render_event() {
    use bevy::ecs::message::ShouldUpdateMessages;
    let (mut app, publishers) = readers_fixture();
    // Establish real fixed-time progress so First's standard message updater is
    // enabled. TimePlugin otherwise starts with ShouldUpdateMessages::Waiting.
    app.update();
    app.update();
    assert!(matches!(
        app.world().resource::<MessageRegistry>().should_update,
        ShouldUpdateMessages::Ready
    ));
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(3));
    publish(&mut app, &publishers);

    // Two explicit simulation steps without an intervening extraction reader.
    // The real session can batch multiple game steps in one Control invocation.
    app.update();
    assert_eq!(
        app.world()
            .resource::<Messages<AssetEvent<TestAsset>>>()
            .len(),
        1
    );
    app.update();
    assert!(
        app.world()
            .resource::<Messages<AssetEvent<TestAsset>>>()
            .is_empty()
    );
    assert_eq!(
        app.world().resource::<GameReads>().0,
        vec![AssetEvent::Added { id: handle.id() }]
    );
    app.world_mut().run_schedule(ReadForExtraction);
    assert!(app.world().resource::<ExtractionReads>().0.is_empty());
}

#[test]
fn retaining_events_until_the_game_runs_accumulates_idle_changes() {
    let (mut app, publishers) = readers_fixture();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(0));
    publish(&mut app, &publishers);
    app.world_mut().run_schedule(ReadForExtraction);
    for value in 1..=32 {
        app.world_mut()
            .resource_mut::<Assets<TestAsset>>()
            .get_mut(&handle)
            .unwrap()
            .0 = value;
        publish(&mut app, &publishers);
        app.world_mut().run_schedule(ReadForExtraction);
    }
    assert_eq!(app.world().resource::<ExtractionReads>().0.len(), 33);
    assert_eq!(
        app.world()
            .resource::<Messages<AssetEvent<TestAsset>>>()
            .len(),
        33
    );
    assert!(app.world().resource::<GameReads>().0.is_empty());
    assert_simulation_stopped(&app, Duration::ZERO);
    app.update();
    assert_eq!(
        app.world().resource::<GameReads>().0,
        app.world().resource::<ExtractionReads>().0
    );
}
