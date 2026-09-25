//! CPU-only prototype: retain AssetEvent for known extraction, Update and
//! FixedUpdate readers. No RenderApp, production adapter or memory policy.
use super::*;
use bevy::{
    ecs::message::{MessageCursor, MessageRegistry},
    time::TimeUpdateStrategy,
};

#[derive(Clone, Copy)]
#[repr(usize)]
enum Reader {
    Extraction,
    Update,
    Fixed,
}

#[derive(Clone, Copy, Default)]
struct Ack {
    pass: u64,
    // Exclusive message sequence boundary, not a frame number or GPU receipt.
    through: usize,
}

#[derive(Resource, Default)]
struct Retention {
    acks: [Ack; 3],
    rotated_at: [u64; 3],
    // All events which may be deleted at the NEXT Messages::update(). New
    // writes go into the other buffer and cannot raise this boundary mid-cycle.
    retire_through: usize,
}

impl Retention {
    fn acknowledge(&mut self, reader: Reader, through: usize) {
        let ack = &mut self.acks[reader as usize];
        assert!(through >= ack.through, "message sequence must not reset");
        ack.pass = ack.pass.checked_add(1).unwrap();
        ack.through = through;
    }

    fn rotate(&mut self, messages: &mut Messages<AssetEvent<TestAsset>>) -> bool {
        if messages.is_empty()
            || self
                .acks
                .iter()
                .zip(self.rotated_at)
                .any(|(ack, previous)| ack.pass <= previous || ack.through < self.retire_through)
        {
            return false;
        }
        // Snapshot before rotating: this is the end of the new old buffer.
        self.retire_through = message_end(messages);
        messages.update();
        self.rotated_at = self.acks.map(|ack| ack.pass);
        true
    }
}

// Valid for this owned, contiguous Messages buffer: only writers and our
// rotations mutate it. External clear/drain/replacement is not supported.
fn message_end<M: Message>(messages: &Messages<M>) -> usize {
    messages
        .oldest_message_count()
        .checked_add(messages.len())
        .unwrap()
}

fn detach_asset_messages(app: &mut App) {
    let world = app.world_mut();
    let messages = world
        .remove_resource::<Messages<AssetEvent<TestAsset>>>()
        .unwrap();
    let end = message_end(&messages);
    // Bevy's public deregistration also removes the resource. Save and restore
    // the SAME value rather than rewriting events and resetting their IDs.
    MessageRegistry::deregister_messages::<AssetEvent<TestAsset>>(world);
    world.insert_resource(messages);
    let mut retention = world.resource_mut::<Retention>();
    retention.retire_through = end;
    retention.rotated_at = retention.acks.map(|ack| ack.pass);
}

#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ExtractRead;

#[derive(Resource, Default)]
struct Seen([Vec<AssetEvent<TestAsset>>; 3]);

#[derive(Message)]
struct GameMessage;

fn read_for(
    who: Reader,
    events: &mut MessageReader<AssetEvent<TestAsset>>,
    messages: &Messages<AssetEvent<TestAsset>>,
    seen: &mut Seen,
    retention: &mut Retention,
) {
    // Only acknowledge after fully consuming this reader's current batch.
    seen.0[who as usize].extend(events.read().copied());
    retention.acknowledge(who, message_end(messages));
}

fn setup() -> (App, Vec<Publisher>) {
    let mut app = fixture();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        20,
    )))
    .init_resource::<Retention>()
    .init_resource::<Seen>()
    .add_message::<GameMessage>()
    .add_systems(
        ExtractRead,
        |mut events: MessageReader<AssetEvent<TestAsset>>,
         messages: Res<Messages<AssetEvent<TestAsset>>>,
         mut seen: ResMut<Seen>,
         mut retention: ResMut<Retention>| {
            read_for(
                Reader::Extraction,
                &mut events,
                &messages,
                &mut seen,
                &mut retention,
            );
        },
    )
    .add_systems(
        Update,
        |mut events: MessageReader<AssetEvent<TestAsset>>,
         messages: Res<Messages<AssetEvent<TestAsset>>>,
         mut seen: ResMut<Seen>,
         mut retention: ResMut<Retention>| {
            read_for(
                Reader::Update,
                &mut events,
                &messages,
                &mut seen,
                &mut retention,
            );
        },
    )
    .add_systems(
        FixedUpdate,
        |mut events: MessageReader<AssetEvent<TestAsset>>,
         messages: Res<Messages<AssetEvent<TestAsset>>>,
         mut seen: ResMut<Seen>,
         mut retention: ResMut<Retention>| {
            read_for(
                Reader::Fixed,
                &mut events,
                &messages,
                &mut seen,
                &mut retention,
            );
        },
    );
    app.finish();
    app.cleanup();
    let publishers = isolate_publishers(&mut app).unwrap();
    (app, publishers)
}

fn rotate(app: &mut App) -> bool {
    app.world_mut()
        .resource_scope(|world, mut retention: Mut<Retention>| {
            retention.rotate(&mut world.resource_mut::<Messages<AssetEvent<TestAsset>>>())
        })
}

fn pending(app: &App) -> usize {
    app.world()
        .resource::<Messages<AssetEvent<TestAsset>>>()
        .len()
}

fn assert_all_seen(app: &App, expected: &[AssetEvent<TestAsset>]) {
    for reader in &app.world().resource::<Seen>().0 {
        assert_eq!(reader, expected);
    }
}

#[test]
fn detaching_preserves_existing_events_ids_and_reader_positions() {
    let (mut app, publishers) = setup();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(1));
    publish(&mut app, &publishers);
    let mut cursor = MessageCursor::<AssetEvent<TestAsset>>::default();
    let before = app.world().resource::<Messages<AssetEvent<TestAsset>>>();
    let ids: Vec<_> = cursor.read_with_id(before).map(|(_, id)| id.id).collect();
    assert_eq!(ids, vec![0]);
    app.world_mut().run_schedule(ExtractRead);
    detach_asset_messages(&mut app);
    assert_eq!(pending(&app), 1);
    assert_eq!(
        cursor
            .read(app.world().resource::<Messages<AssetEvent<TestAsset>>>())
            .count(),
        0
    );
    app.world_mut().run_schedule(ExtractRead);
    assert_eq!(
        app.world().resource::<Seen>().0[Reader::Extraction as usize].len(),
        1
    );

    app.world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .get_mut(&handle)
        .unwrap()
        .0 = 2;
    publish(&mut app, &publishers);
    let ids: Vec<_> = cursor
        .read_with_id(app.world().resource::<Messages<AssetEvent<TestAsset>>>())
        .map(|(_, id)| id.id)
        .collect();
    assert_eq!(ids, vec![1]);
    assert_simulation_stopped(&app, Duration::ZERO);
}

#[test]
fn extraction_alone_cannot_age_assets_or_other_game_messages() {
    let (mut app, publishers) = setup();
    detach_asset_messages(&mut app);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(2));
    app.world_mut().write_message(GameMessage);
    publish(&mut app, &publishers);
    for _ in 0..64 {
        app.world_mut().run_schedule(ExtractRead);
        assert!(!rotate(&mut app));
    }
    assert_eq!(pending(&app), 1);
    assert_eq!(app.world().resource::<Messages<GameMessage>>().len(), 1);
    assert_simulation_stopped(&app, Duration::ZERO);

    // Explicit game steps. The first does not yet run FixedUpdate.
    app.update();
    assert!(!rotate(&mut app));
    app.update();
    assert!(rotate(&mut app));
    assert_eq!(pending(&app), 1);
    assert_all_seen(&app, &[AssetEvent::Added { id: handle.id() }]);
    // Reusing the same acknowledgements cannot rotate again.
    assert!(!rotate(&mut app));
    app.update();
    app.world_mut().run_schedule(ExtractRead);
    assert!(rotate(&mut app));
    assert_eq!(pending(&app), 0);
    assert_all_seen(&app, &[AssetEvent::Added { id: handle.id() }]);
}

#[test]
fn batched_game_steps_age_normal_messages_but_keep_unread_assets() {
    let (mut app, publishers) = setup();
    detach_asset_messages(&mut app);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(3));
    app.world_mut().write_message(GameMessage);
    publish(&mut app, &publishers);
    for _ in 0..6 {
        app.update();
        assert!(!rotate(&mut app));
    }
    assert!(app.world().resource::<Messages<GameMessage>>().is_empty());
    assert_eq!(pending(&app), 1);
    app.world_mut().run_schedule(ExtractRead);
    assert!(rotate(&mut app));
    assert_all_seen(&app, &[AssetEvent::Added { id: handle.id() }]);
    app.update();
    app.world_mut().run_schedule(ExtractRead);
    assert!(rotate(&mut app));
    assert_eq!(pending(&app), 0);
}

#[test]
fn a_write_between_acknowledgement_and_rotation_is_not_discarded() {
    let (mut app, publishers) = setup();
    detach_asset_messages(&mut app);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(4));
    publish(&mut app, &publishers);
    app.update();
    app.update();
    app.world_mut().run_schedule(ExtractRead);
    app.world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .get_mut(&handle)
        .unwrap()
        .0 = 5;
    publish(&mut app, &publishers);
    assert_eq!(pending(&app), 2);
    // Rotation only retires the OLD buffer, not the newly written event.
    assert!(rotate(&mut app));
    assert_eq!(pending(&app), 2);
    app.world_mut().run_schedule(ExtractRead);
    assert!(!rotate(&mut app), "game readers have not read Modified yet");
    app.update();
    assert!(rotate(&mut app));
    assert_eq!(pending(&app), 0);
    assert_all_seen(
        &app,
        &[
            AssetEvent::Added { id: handle.id() },
            AssetEvent::Modified { id: handle.id() },
        ],
    );
}

#[test]
fn a_stalled_game_reader_keeps_new_events_without_silent_coalescing() {
    let (mut app, publishers) = setup();
    detach_asset_messages(&mut app);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(0));
    publish(&mut app, &publishers);
    for value in 1..=32 {
        app.world_mut()
            .resource_mut::<Assets<TestAsset>>()
            .get_mut(&handle)
            .unwrap()
            .0 = value;
        publish(&mut app, &publishers);
        app.world_mut().run_schedule(ExtractRead);
        assert!(!rotate(&mut app));
    }
    assert_eq!(pending(&app), 33);
    assert_eq!(
        app.world().resource::<Seen>().0[Reader::Extraction as usize].len(),
        33
    );
    assert_simulation_stopped(&app, Duration::ZERO);
    // This finite test does NOT implement a production memory limit. The queue
    // grows with new events while a required reader is stopped.
}

#[test]
fn continuous_writes_can_reclaim_an_acknowledged_old_buffer() {
    let (mut app, publishers) = setup();
    detach_asset_messages(&mut app);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(6));
    publish(&mut app, &publishers);
    app.update();
    app.update();
    app.world_mut().run_schedule(ExtractRead);
    assert!(rotate(&mut app));
    for value in 7..=22 {
        // Confirm the previously sealed batch, then append an unread event.
        // Requiring acknowledgements of the ever-growing current END instead
        // would prevent reclamation under this repeated ordering.
        app.update();
        app.world_mut().run_schedule(ExtractRead);
        app.world_mut()
            .resource_mut::<Assets<TestAsset>>()
            .get_mut(&handle)
            .unwrap()
            .0 = value;
        publish(&mut app, &publishers);
        assert!(rotate(&mut app));
        assert_eq!(pending(&app), 1, "the new unread event must remain");
    }
    app.update();
    app.world_mut().run_schedule(ExtractRead);
    assert!(rotate(&mut app));
    assert_eq!(pending(&app), 0);
    let seen = &app.world().resource::<Seen>().0;
    assert_eq!(seen[0].len(), 17);
    assert_eq!(seen[0], seen[1]);
    assert_eq!(seen[1], seen[2]);
}
