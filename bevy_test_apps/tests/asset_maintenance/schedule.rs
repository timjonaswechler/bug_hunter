//! A version-bound CPU probe, not a production schedule adapter.
//! Move the registered publisher before schedule initialization, retaining its
//! conditions. A forwarding system keeps its original PostUpdate position.
use super::*;
use bevy::{
    app::{MainScheduleOrder, PluginsState},
    asset::AssetEventSystems,
    ecs::{
        schedule::{NodeId, ScheduleLabel, SystemWithAccess, graph::Direction},
        system::IntoSystem,
    },
};
use std::collections::HashSet;

#[path = "lifetime.rs"]
mod lifetime;

#[path = "retention.rs"]
mod retention;

#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Publisher(usize);

// Deliberately restricted to our known AssetPlugin fixture. Not a general
// schedule partitioner: relevant set conditions, ordering across publishers,
// late plugin registration and already initialized schedules require separate treatment.
fn publisher_keys(
    schedule: &Schedule,
) -> Result<Vec<bevy::ecs::schedule::SystemKey>, &'static str> {
    let graph = schedule.graph();
    let root = graph
        .system_sets
        .get_key(AssetEventSystems.intern())
        .ok_or("missing asset set")?;
    let mut pending = vec![NodeId::Set(root)];
    let mut seen = HashSet::new();
    let mut keys = Vec::new();
    while let Some(node) = pending.pop() {
        if !seen.insert(node) {
            continue;
        }
        match node {
            NodeId::System(key) => keys.push(key),
            NodeId::Set(_) => pending.extend(
                graph
                    .hierarchy()
                    .graph()
                    .neighbors_directed(node, Direction::Outgoing),
            ),
        }
    }
    if keys.is_empty() {
        return Err("no publishers");
    }
    Ok(keys)
}

fn publisher_has_conditioned_ancestor(
    graph: &bevy::ecs::schedule::ScheduleGraph,
    key: bevy::ecs::schedule::SystemKey,
) -> bool {
    let mut pending = vec![NodeId::System(key)];
    let mut seen = HashSet::new();
    while let Some(node) = pending.pop() {
        for parent in graph
            .hierarchy()
            .graph()
            .neighbors_directed(node, Direction::Incoming)
        {
            if !seen.insert(parent) {
                continue;
            }
            if let NodeId::Set(key) = parent {
                if graph.system_sets.has_conditions(key) {
                    return true;
                }
                pending.push(parent);
            }
        }
    }
    false
}

fn isolate_publishers(app: &mut App) -> Result<Vec<Publisher>, &'static str> {
    app.world_mut()
        .schedule_scope(PostUpdate, |world, schedule| {
            if schedule.systems().is_ok() || schedule.graph().systems.is_initialized() {
                return Err("install before initialization");
            }
            // Only the known fixture's native publishers belong to this set.
            // Do not use debug system names: they may be stripped by Bevy.
            let keys = publisher_keys(schedule)?;
            let graph = schedule.graph();
            if keys
                .iter()
                .any(|key| publisher_has_conditioned_ancestor(graph, *key))
            {
                return Err("publisher set conditions are not supported by this probe");
            }
            for key in &keys {
                graph.systems.get(*key).ok_or("publisher already moved")?;
            }
            let mut labels = Vec::new();
            for (index, key) in keys.into_iter().enumerate() {
                let label = Publisher(index);
                let forward =
                    IntoSystem::into_system(move |world: &mut World| world.run_schedule(label));
                let graph = schedule.graph_mut();
                let publisher = std::mem::replace(
                    graph.systems.get_mut(key).unwrap(),
                    SystemWithAccess::new(Box::new(forward)),
                );
                let conditions = std::mem::take(graph.systems.get_conditions_mut(key).unwrap());
                let mut configs = publisher.into_configs();
                for condition in conditions {
                    configs.run_if_dyn(condition.condition);
                }
                let mut isolated = Schedule::new(label);
                isolated.add_systems(configs);
                world.add_schedule(isolated);
                labels.push(label);
            }
            Ok(labels)
        })
}

fn publish(app: &mut App, labels: &[Publisher]) {
    for label in labels {
        app.world_mut().run_schedule(*label);
    }
}

fn events(app: &mut App) -> Vec<AssetEvent<TestAsset>> {
    app.world_mut()
        .resource_mut::<Messages<AssetEvent<TestAsset>>>()
        .drain()
        .collect()
}

#[test]
fn registered_publishers_flush_real_collection_events_without_simulation() {
    let mut app = fixture();
    let labels = isolate_publishers(&mut app).unwrap();
    let elapsed = app.world().resource::<Time<Virtual>>().elapsed();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(1));
    publish(&mut app, &labels);
    assert_eq!(
        events(&mut app),
        vec![AssetEvent::Added { id: handle.id() }]
    );
    app.world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .get_mut(&handle)
        .unwrap()
        .0 = 2;
    publish(&mut app, &labels);
    assert_eq!(
        events(&mut app),
        vec![AssetEvent::Modified { id: handle.id() }]
    );
    app.world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .remove(&handle);
    publish(&mut app, &labels);
    assert_eq!(
        events(&mut app),
        vec![AssetEvent::Removed { id: handle.id() }]
    );
    for _ in 0..4 {
        publish(&mut app, &labels);
    }
    assert!(events(&mut app).is_empty());
    assert_simulation_stopped(&app, elapsed);
}

#[test]
fn shader_and_custom_asset_publishers_keep_their_typed_queues_separate() {
    use bevy::shader::Shader;
    let mut app = fixture();
    app.init_asset::<Shader>();
    let labels = isolate_publishers(&mut app).unwrap();
    let elapsed = app.world().resource::<Time<Virtual>>().elapsed();
    let shader = app
        .world_mut()
        .resource_mut::<Assets<Shader>>()
        .add(Shader::from_wgsl(
            "// CPU event test only",
            "asset-maintenance-test.wgsl",
        ));
    let asset = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(8));
    publish(&mut app, &labels);
    assert_eq!(events(&mut app), vec![AssetEvent::Added { id: asset.id() }]);
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<Shader>>>()
            .drain()
            .collect::<Vec<_>>(),
        vec![AssetEvent::Added { id: shader.id() }]
    );
    *app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .get_mut(&shader)
        .unwrap() = Shader::from_wgsl(
        "// changed, not GPU compiled",
        "asset-maintenance-test.wgsl",
    );
    publish(&mut app, &labels);
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<Shader>>>()
            .drain()
            .collect::<Vec<_>>(),
        vec![AssetEvent::Modified { id: shader.id() }]
    );
    assert!(events(&mut app).is_empty());
    assert_simulation_stopped(&app, elapsed);
}

#[test]
fn real_server_load_and_drop_reach_the_registered_publisher() {
    let mut app = fixture();
    let labels = isolate_publishers(&mut app).unwrap();
    let elapsed = app.world().resource::<Time<Virtual>>().elapsed();
    let handle = app.world().resource::<AssetServer>().add(TestAsset(3));
    let id = handle.id();
    run_public_maintenance(&mut app);
    run_public_maintenance(&mut app);
    publish(&mut app, &labels);
    let loaded = events(&mut app);
    assert!(loaded.contains(&AssetEvent::LoadedWithDependencies { id }));
    assert!(loaded.contains(&AssetEvent::Added { id }));
    assert_eq!(loaded.len(), 2);
    drop(handle);
    run_public_maintenance(&mut app);
    publish(&mut app, &labels);
    let dropped = events(&mut app);
    assert!(dropped.contains(&AssetEvent::Unused { id }));
    assert!(dropped.contains(&AssetEvent::Removed { id }));
    assert_eq!(dropped.len(), 2);
    publish(&mut app, &labels);
    assert!(events(&mut app).is_empty());
    assert_simulation_stopped(&app, elapsed);
}

#[derive(Resource, Default)]
struct AllowPublish(bool);

#[test]
fn system_conditions_and_normal_post_update_forwarding_survive_the_move() {
    use bevy::ecs::schedule::ConditionWithAccess;
    let mut app = fixture();
    app.init_resource::<AllowPublish>();
    // Add an observable condition to the actual registered systems BEFORE the
    // move, not to a replacement implementation of asset_events.
    app.world_mut().schedule_scope(PostUpdate, |_, schedule| {
        let keys = publisher_keys(schedule).unwrap();
        let graph = schedule.graph_mut();
        for key in keys {
            let condition = IntoSystem::into_system(|allow: Res<AllowPublish>| allow.0);
            graph
                .systems
                .get_conditions_mut(key)
                .unwrap()
                .push(ConditionWithAccess::new(Box::new(condition)));
        }
    });
    let labels = isolate_publishers(&mut app).unwrap();
    let elapsed = app.world().resource::<Time<Virtual>>().elapsed();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .add(TestAsset(4));
    publish(&mut app, &labels);
    assert!(events(&mut app).is_empty());
    assert_simulation_stopped(&app, elapsed);
    app.world_mut().resource_mut::<AllowPublish>().0 = true;
    publish(&mut app, &labels);
    assert_eq!(
        events(&mut app),
        vec![AssetEvent::Added { id: handle.id() }]
    );
    assert_simulation_stopped(&app, elapsed);

    // Explicit simulated game step, NOT a maintenance fallback. It initializes
    // the original schedule and must still invoke the moved publisher in place.
    app.world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .get_mut(&handle)
        .unwrap()
        .0 = 5;
    app.world_mut().run_schedule(PostUpdate);
    assert_eq!(app.world().resource::<ScheduleRuns>().post_update, 1);
    assert_eq!(
        events(&mut app),
        vec![AssetEvent::Modified { id: handle.id() }]
    );
    #[derive(Resource, Default)]
    struct RebuiltRuns(u32);
    app.init_resource::<RebuiltRuns>()
        .add_systems(PostUpdate, |mut runs: ResMut<RebuiltRuns>| runs.0 += 1);
    app.world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .get_mut(&handle)
        .unwrap()
        .0 = 6;
    app.world_mut().run_schedule(PostUpdate);
    assert_eq!(app.world().resource::<ScheduleRuns>().post_update, 2);
    assert_eq!(app.world().resource::<RebuiltRuns>().0, 1);
    assert_eq!(
        events(&mut app),
        vec![AssetEvent::Modified { id: handle.id() }]
    );

    app.world_mut()
        .resource_mut::<Assets<TestAsset>>()
        .remove(&handle);
    publish(&mut app, &labels);
    assert_eq!(
        events(&mut app),
        vec![AssetEvent::Removed { id: handle.id() }]
    );
    assert_eq!(app.world().resource::<ScheduleRuns>().post_update, 2);
    assert_eq!(app.world().resource::<RebuiltRuns>().0, 1);
    assert_eq!(app.world().resource::<ScheduleRuns>().update, 0);
    assert_eq!(app.world().resource::<Time<Virtual>>().elapsed(), elapsed);
}

#[test]
fn installation_rejects_initialized_schedules_and_relevant_set_conditions() {
    let mut app = fixture();
    app.world_mut()
        .schedule_scope(PostUpdate, |world, schedule| {
            schedule.initialize(world).unwrap()
        });
    assert_eq!(
        isolate_publishers(&mut app),
        Err("install before initialization")
    );
    assert_simulation_stopped(&app, Duration::ZERO);

    let mut app = fixture();
    app.configure_sets(PostUpdate, AssetEventSystems.run_if(|| false));
    assert_eq!(
        isolate_publishers(&mut app),
        Err("publisher set conditions are not supported by this probe")
    );
    assert_simulation_stopped(&app, Duration::ZERO);

    #[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct UnrelatedSet;
    let mut app = fixture();
    app.configure_sets(PostUpdate, UnrelatedSet.run_if(|| false));
    assert!(isolate_publishers(&mut app).is_ok());
    assert_simulation_stopped(&app, Duration::ZERO);
}

#[derive(Asset, TypePath)]
struct BuildAsset;

#[derive(Asset, TypePath)]
struct FinishAsset;

#[derive(Asset, TypePath)]
struct CleanupAsset;

struct LifecycleAssetPlugin;

impl Plugin for LifecycleAssetPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<BuildAsset>();
    }

    fn finish(&self, app: &mut App) {
        app.init_asset::<FinishAsset>();
    }

    fn cleanup(&self, app: &mut App) {
        app.init_asset::<CleanupAsset>();
    }
}

#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LifecycleControl;

#[derive(Resource)]
struct LifecyclePublishers(Vec<Publisher>);

#[derive(Resource, Default)]
struct LifecycleRuns {
    control: u32,
    post_update: u32,
}

struct LifecycleInstaller;

impl Plugin for LifecycleInstaller {
    fn build(&self, app: &mut App) {
        let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
        order.labels = vec![LifecycleControl.intern()];
        app.init_resource::<LifecycleRuns>()
            .add_systems(
                Startup,
                (
                    |mut assets: ResMut<Assets<BuildAsset>>| {
                        assets.add(BuildAsset);
                    },
                    |mut assets: ResMut<Assets<FinishAsset>>| {
                        assets.add(FinishAsset);
                    },
                    |mut assets: ResMut<Assets<CleanupAsset>>| {
                        assets.add(CleanupAsset);
                    },
                ),
            )
            .add_systems(LifecycleControl, |world: &mut World| {
                world.resource_mut::<LifecycleRuns>().control += 1;
                let publishers = world.resource::<LifecyclePublishers>().0.clone();
                for publisher in publishers {
                    world.run_schedule(publisher);
                }
            })
            .add_systems(PostUpdate, |mut runs: ResMut<LifecycleRuns>| {
                runs.post_update += 1;
            });
    }

    fn cleanup(&self, app: &mut App) {
        let publishers = isolate_publishers(app).expect("cleanup installation");
        app.insert_resource(LifecyclePublishers(publishers));
    }
}

fn post_update_initialized(app: &mut App) -> bool {
    app.world_mut().schedule_scope(PostUpdate, |_, schedule| {
        schedule.systems().is_ok() || schedule.graph().systems.is_initialized()
    })
}

fn assert_one_added<A: Asset>(app: &mut App) {
    let events = app
        .world_mut()
        .resource_mut::<Messages<AssetEvent<A>>>()
        .drain()
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], AssetEvent::Added { .. }));
}

#[test]
fn cleanup_installation_sees_lifecycle_assets_before_first_control() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        LifecycleAssetPlugin,
        LifecycleInstaller,
    ));
    assert_eq!(app.plugins_state(), PluginsState::Ready);
    app.finish();
    assert!(!post_update_initialized(&mut app));
    app.cleanup();
    assert!(!post_update_initialized(&mut app));

    // This lifecycle fixture mirrors the session's retained Startup schedules and
    // reduced regular order. It does not execute the production session plugin.
    app.update();
    assert_one_added::<BuildAsset>(&mut app);
    assert_one_added::<FinishAsset>(&mut app);
    assert_one_added::<CleanupAsset>(&mut app);
    assert_eq!(app.world().resource::<LifecycleRuns>().control, 1);
    assert_eq!(app.world().resource::<LifecycleRuns>().post_update, 0);
    assert_eq!(
        app.world().resource::<Time<Virtual>>().elapsed(),
        Duration::ZERO
    );
    assert!(!post_update_initialized(&mut app));
}

#[derive(Resource, Default)]
struct ForeignRuns(u32);

#[test]
fn foreign_asset_event_set_members_would_run_during_idle_and_are_unsupported() {
    let mut app = fixture();
    app.init_resource::<ForeignRuns>().add_systems(
        PostUpdate,
        (|mut runs: ResMut<ForeignRuns>| runs.0 += 1).in_set(AssetEventSystems),
    );
    let labels = isolate_publishers(&mut app).unwrap();
    publish(&mut app, &labels);
    assert_eq!(app.world().resource::<ForeignRuns>().0, 1);
    assert_simulation_stopped(&app, Duration::ZERO);
}

#[derive(Asset, TypePath)]
struct LateAsset;

#[test]
fn asset_types_registered_after_installation_are_not_idle_published() {
    let mut app = fixture();
    let labels = isolate_publishers(&mut app).unwrap();
    app.init_asset::<LateAsset>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<LateAsset>>()
        .add(LateAsset);

    publish(&mut app, &labels);
    assert!(
        app.world()
            .resource::<Messages<AssetEvent<LateAsset>>>()
            .is_empty()
    );

    // A deliberately executed game PostUpdate still reaches the late publisher.
    // This is a boundary proof, not an idle-maintenance fallback.
    app.world_mut().run_schedule(PostUpdate);
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<LateAsset>>>()
            .drain()
            .collect::<Vec<_>>(),
        vec![AssetEvent::Added { id: handle.id() }]
    );
    assert_eq!(app.world().resource::<ScheduleRuns>().post_update, 1);
}
