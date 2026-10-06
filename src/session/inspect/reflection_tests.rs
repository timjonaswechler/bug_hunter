//! Contract fixtures run through both public inspect projections, not just serde.
use super::*;
use crate::command::inspect::{Command, Projection};
use bevy::{
    asset::{Asset, AssetApp, AssetPlugin, Assets, Handle, UntypedHandle},
    prelude::*,
    reflect::{GetTypeRegistration, Typed},
};
use serde::{Serialize, Serializer};
use serde_json::json;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Clone, Resource, Reflect)]
#[reflect(Resource)]
struct Fixture<T: Reflect + FromReflect + GetTypeRegistration + Typed>(T);

#[derive(Clone, Component, Reflect)]
#[reflect(Component)]
struct ComponentFixture<T: Reflect + FromReflect + GetTypeRegistration + Typed>(T);

fn check<T>(value: T, expected: Value)
where
    T: Reflect + FromReflect + GetTypeRegistration + Typed + Clone,
{
    let mut app = App::new();
    app.register_type::<Fixture<T>>();
    check_in(&mut app, value, expected);
}

fn check_in<T>(app: &mut App, value: T, expected: Value)
where
    T: Reflect + FromReflect + GetTypeRegistration + Typed + Clone,
{
    // Do not auto-register dependencies here: some fixtures deliberately omit them.
    app.world()
        .resource::<AppTypeRegistry>()
        .write()
        .add_registration(ComponentFixture::<T>::get_type_registration());
    app.insert_resource(Fixture(value.clone()));
    let id = app.world_mut().spawn(ComponentFixture(value)).id();
    let world = app.world();
    let tick = world.read_change_tick();
    let resource_ticks = world
        .get_resource_ref::<Fixture<T>>()
        .unwrap()
        .last_changed();
    let component_ticks = world
        .entity(id)
        .get_ref::<ComponentFixture<T>>()
        .unwrap()
        .last_changed();
    let path = Fixture::<T>::type_path();
    for _ in 0..2 {
        let resources = query(
            world,
            Command::Resources {
                selector: Selector::Type {
                    type_path: path.into(),
                },
                projection: Projection::Value {},
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(resources).unwrap(),
            json!({
                "items": [{"kind": "resource", "result": {
                    "kind": "value", "type_path": path, "value": expected
                }}]
            }),
            "{path}"
        );
        let components = query(
            world,
            Command::Entities {
                entity: Some(id.into()),
                with: vec![],
                without: vec![],
                projection: entity::Projection::Components {
                    selection: component::Selection::Listed {
                        type_paths: vec![ComponentFixture::<T>::type_path().into()],
                    },
                },
            },
        )
        .unwrap();
        let output = serde_json::to_value(components).unwrap();
        assert_eq!(
            output["items"][0]["result"]["components"][0]["value"],
            json!(expected),
            "{path}"
        );
        assert_eq!(
            output["items"][0]["result"]["components"][0]["component"]["type_path"],
            ComponentFixture::<T>::type_path()
        );
    }
    assert_eq!(world.read_change_tick(), tick);
    assert_eq!(
        world
            .get_resource_ref::<Fixture<T>>()
            .unwrap()
            .last_changed(),
        resource_ticks
    );
    assert_eq!(
        world
            .entity(id)
            .get_ref::<ComponentFixture<T>>()
            .unwrap()
            .last_changed(),
        component_ticks
    );
}

fn readable(value: serde_json::Value) -> Value {
    Value::Readable { value }
}

fn not_serializable() -> Value {
    Value::Unavailable {
        reason: Status::NotSerializable,
    }
}

#[derive(Clone, Reflect)]
struct Numbers {
    single: Vec<f32>,
    double: Option<(f64,)>,
}

#[test]
fn finite_and_nonfinite_numbers_fail_the_whole_value() {
    check(
        Numbers {
            single: vec![0.0, -1.5],
            double: Some((2.25,)),
        },
        readable(json!({"single": [0.0, -1.5], "double": [2.25]})),
    );
    for number in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        check(number, not_serializable());
        check(
            Numbers {
                single: vec![1.0, number],
                double: None,
            },
            not_serializable(),
        );
    }
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        check(number, not_serializable());
        check(
            Numbers {
                single: vec![],
                double: Some((number,)),
            },
            not_serializable(),
        );
        check(
            HashMap::from([("bad".to_string(), number)]),
            not_serializable(),
        );
    }
}

#[derive(Clone, Reflect, Serialize)]
#[reflect(opaque, Serialize)]
struct CustomNull(f64);

#[derive(Clone, Reflect)]
#[reflect(opaque)]
struct Opaque;

#[derive(Clone, Reflect)]
#[reflect(opaque, Serialize)]
struct Failing;

impl Serialize for Failing {
    fn serialize<S: Serializer>(&self, _: S) -> std::result::Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("fixture serializer failure"))
    }
}

#[test]
fn opaque_values_use_custom_serialization_without_reinterpreting_null() {
    check(CustomNull(f64::NAN), readable(json!(null)));
    check(CustomNull(3.5), readable(json!(3.5)));
    check(Opaque, not_serializable());
    check(Failing, not_serializable());
    check(vec![Failing], not_serializable());
    // Bevy 0.19.1 exposes BTreeSet as opaque, without ReflectSerialize.
    check(BTreeSet::from([1_u8, 2]), not_serializable());
}

#[test]
fn maps_keep_scalar_keys_and_reject_compound_keys() {
    check(
        HashMap::from([(12_i32, "twelve".to_string()), (-3, "minus".into())]),
        readable(json!({"12": "twelve", "-3": "minus"})),
    );
    check(
        HashMap::from([(true, 1_u8), (false, 0)]),
        readable(json!({"true": 1, "false": 0})),
    );
    check(
        HashMap::from([("ü".to_string(), vec![1_u8, 2])]),
        readable(json!({"ü": [1, 2]})),
    );
    check(
        HashMap::from([('ü', u64::MAX)]),
        readable(json!({"ü": u64::MAX})),
    );
    check(
        HashMap::from([(u64::MAX, true)]),
        readable(json!({"18446744073709551615": true})),
    );
    check(HashMap::from([((1_u8, 2_u8), 3_u8)]), not_serializable());
    check(HashMap::from([(vec![1_u8], 3_u8)]), not_serializable());
    check(HashMap::<String, u8>::new(), readable(json!({})));
}

#[derive(Clone, Reflect, PartialEq, Eq)]
struct SetEntry {
    z: u8,
    a: HashSet<u8>,
}

impl std::hash::Hash for SetEntry {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.z.hash(state);
        let mut members: Vec<_> = self.a.iter().collect();
        members.sort();
        members.hash(state);
    }
}

#[derive(Clone, Reflect, PartialEq, Eq, Hash)]
#[reflect(opaque, Serialize)]
struct BadElement;

impl Serialize for BadElement {
    fn serialize<S: Serializer>(&self, _: S) -> std::result::Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("invalid set element"))
    }
}

#[test]
fn sets_sort_compact_json_bytes_recursively_and_reject_bad_elements() {
    // Lexical JSON order deliberately differs from numeric and declaration order.
    check(HashSet::from([2_u8, 10, 1]), readable(json!([1, 10, 2])));
    for entries in [
        vec![
            SetEntry {
                z: 9,
                a: HashSet::from([2, 10]),
            },
            SetEntry {
                z: 0,
                a: HashSet::from([1]),
            },
        ],
        vec![
            SetEntry {
                z: 0,
                a: HashSet::from([1]),
            },
            SetEntry {
                z: 9,
                a: HashSet::from([10, 2]),
            },
        ],
    ] {
        check(
            entries.into_iter().collect::<HashSet<_>>(),
            readable(json!([{"a": [10, 2], "z": 9}, {"a": [1], "z": 0}])),
        );
    }
    check(HashSet::<String>::new(), readable(json!([])));
    check(HashSet::from([BadElement]), not_serializable());
}

#[test]
fn missing_nested_registration_is_not_a_query_path_error() {
    let mut app = App::new();
    // Register only the outer type, intentionally omit the opaque field's ReflectSerialize.
    app.world()
        .resource::<AppTypeRegistry>()
        .write()
        .add_registration(Fixture::<CustomNull>::get_type_registration());
    check_in(&mut app, CustomNull(1.0), not_serializable());
}

#[derive(Resource, Reflect)]
#[reflect(Resource)]
#[type_path = "inspect_fixture"]
struct Named(u8);

#[derive(Resource, Reflect)]
struct NoAccess;

#[derive(Resource)]
struct Unregistered;

#[test]
fn resource_discovery_statuses_and_exact_registered_type_paths() {
    let mut app = App::new();
    app.register_type::<Named>()
        .register_type::<NoAccess>()
        .register_type::<Fixture<Opaque>>()
        .insert_resource(NoAccess)
        .insert_resource(Unregistered)
        .insert_resource(Fixture(Opaque));
    let read = |app: &App, selector, projection| {
        serde_json::to_value(
            query(
                app.world(),
                Command::Resources {
                    selector,
                    projection,
                },
            )
            .unwrap(),
        )
        .unwrap()
    };
    let named = || Selector::Type {
        type_path: "inspect_fixture::Named".into(),
    };
    assert_eq!(Named::type_path(), "inspect_fixture::Named");
    for invalid in [
        "Named",
        std::any::type_name::<Named>(),
        "inspect_fixture::named",
    ] {
        assert_eq!(
            query(
                app.world(),
                Command::Resources {
                    selector: Selector::Type {
                        type_path: invalid.into()
                    },
                    projection: Projection::Value {},
                }
            )
            .unwrap_err()
            .code,
            "unknown_type_path"
        );
        assert_eq!(
            query(
                app.world(),
                Command::Entities {
                    entity: None,
                    with: vec![invalid.into()],
                    without: vec![],
                    projection: entity::Projection::Summary {},
                }
            )
            .unwrap_err()
            .code,
            "unknown_type_path"
        );
    }
    assert_eq!(
        read(&app, named(), Projection::Metadata {}),
        json!({"items": []})
    );
    assert_eq!(
        read(&app, named(), Projection::Value {})["items"][0]["result"]["value"],
        json!({"status": "unavailable", "reason": "missing"})
    );
    assert_eq!(
        read(
            &app,
            Selector::Type {
                type_path: NoAccess::type_path().into()
            },
            Projection::Value {}
        )["items"][0]["result"]["value"],
        json!({"status": "unavailable", "reason": "not_reflectable"})
    );

    app.insert_resource(Named(7));
    let all = read(&app, Selector::All {}, Projection::Value {});
    // Resource All excludes unregistered resources, absent resources, and types
    // without ReflectResource. A serialization failure does not erase other items.
    let paths: Vec<_> = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["result"]["type_path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec![Named::type_path(), Fixture::<Opaque>::type_path()]
    );
    assert_eq!(
        all["items"][0]["result"]["value"],
        json!(readable(json!(7)))
    );
    assert_eq!(
        all["items"][1]["result"]["value"],
        json!(not_serializable())
    );
    assert_eq!(
        read(&app, Selector::All {}, Projection::Metadata {}),
        json!({
            "items": [
                {"kind": "resource", "result": {"kind": "metadata", "type_path": Named::type_path()}},
                {"kind": "resource", "result": {"kind": "metadata", "type_path": Fixture::<Opaque>::type_path()}}
            ]
        })
    );
}

#[derive(Clone, Reflect)]
#[reflect(opaque, Serialize)]
struct MustNotSerialize;

impl Serialize for MustNotSerialize {
    fn serialize<S: Serializer>(&self, _: S) -> std::result::Result<S::Ok, S::Error> {
        panic!("metadata must not invoke the value serializer")
    }
}

#[test]
fn metadata_projections_never_call_the_serializer() {
    let mut app = App::new();
    app.register_type::<Fixture<MustNotSerialize>>()
        .register_type::<ComponentFixture<MustNotSerialize>>()
        .insert_resource(Fixture(MustNotSerialize));
    let id = app
        .world_mut()
        .spawn(ComponentFixture(MustNotSerialize))
        .id();
    let resources = query(
        app.world(),
        Command::Resources {
            selector: Selector::All {},
            projection: Projection::Metadata {},
        },
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(resources).unwrap(),
        json!({
            "items": [{"kind": "resource", "result": {
                "kind": "metadata", "type_path": Fixture::<MustNotSerialize>::type_path()
            }}]
        })
    );
    let components = query(
        app.world(),
        Command::Entities {
            entity: Some(id.into()),
            with: vec![],
            without: vec![],
            projection: entity::Projection::ComponentNames {},
        },
    )
    .unwrap();
    let output = serde_json::to_value(components).unwrap();
    assert_eq!(
        output["items"][0]["result"],
        json!({
            "kind": "component_names", "components": [{
                "name": std::any::type_name::<ComponentFixture<MustNotSerialize>>(),
                "type_path": ComponentFixture::<MustNotSerialize>::type_path()
            }]
        })
    );
}

#[derive(Asset, Reflect)]
struct TestAsset;

#[test]
fn asset_handles_preserve_uuid_and_ephemeral_identity() {
    let mut assets = Assets::<TestAsset>::default();
    let ephemeral = assets.add(TestAsset);
    let bevy::asset::AssetId::Index { index, .. } = ephemeral.id() else {
        panic!("index handle")
    };
    let reference = json!({"Ephemeral": {"id": format!("{:016x}", index.to_bits())}});
    check(ephemeral.clone(), readable(reference.clone()));
    let uuid = Handle::<TestAsset>::from(bevy::asset::uuid::Uuid::from_u128(123));
    let uuid_reference = json!({"Uuid": "00000000-0000-0000-0000-00000000007b"});
    check(uuid.clone(), readable(uuid_reference.clone()));
    let second = assets.add(TestAsset);
    assert_ne!(second.id(), ephemeral.id());
    let bevy::asset::AssetId::Index {
        index: second_index,
        ..
    } = second.id()
    else {
        panic!("index handle")
    };
    check(
        vec![ephemeral.clone(), second, ephemeral.clone()],
        readable(json!([reference, {"Ephemeral": {
            "id": format!("{:016x}", second_index.to_bits())
        }}, reference])),
    );
    for (handle, reference) in [(ephemeral, reference), (uuid, uuid_reference)] {
        let mut app = App::new();
        app.register_type::<Fixture<UntypedHandle>>();
        // UntypedHandle cannot register its runtime asset type automatically.
        check_in(&mut app, handle.clone().untyped(), not_serializable());
        app.register_type::<TestAsset>();
        check_in(
            &mut app,
            handle.untyped(),
            readable(json!({
                "asset_type": TestAsset::type_path(), "reference": reference
            })),
        );
    }
}

#[test]
fn typed_handle_without_its_registration_is_not_serializable() {
    let mut app = App::new();
    app.world()
        .resource::<AppTypeRegistry>()
        .write()
        .add_registration(Fixture::<Handle<TestAsset>>::get_type_registration());
    let mut assets = Assets::<TestAsset>::default();
    check_in(&mut app, assets.add(TestAsset), not_serializable());
    check_in(&mut app, Handle::<TestAsset>::default(), not_serializable());
}

#[test]
fn path_handles_preserve_source_and_label_without_filesystem_io() {
    use bevy::asset::io::{AssetSourceBuilder, memory::MemoryAssetReader};

    let mut app = App::new();
    app.register_asset_source(
        "fixture",
        AssetSourceBuilder::new(|| Box::new(MemoryAssetReader::default())),
    );
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<TestAsset>()
        .register_type::<Fixture<Handle<TestAsset>>>()
        .register_type::<Fixture<UntypedHandle>>()
        .register_type::<TestAsset>();
    // An in-memory source keeps the fixture independent of the filesystem.
    // Inspect needs the reference, not a successfully loaded asset.
    let handle = app
        .world()
        .resource::<AssetServer>()
        .load::<TestAsset>("fixture://example.asset#part");
    let reference = json!({"Path": "fixture://example.asset#part"});
    check_in(&mut app, handle.clone(), readable(reference.clone()));
    check_in(
        &mut app,
        handle.untyped(),
        readable(json!({
            "asset_type": TestAsset::type_path(), "reference": reference
        })),
    );
}
