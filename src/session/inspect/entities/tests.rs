use super::*;
use crate::handle::Handle;
use bevy::prelude::*;
use serde_json::json;

#[derive(Component, Reflect)]
#[reflect(Component)]
struct Sample {
    number: f64,
    labels: std::collections::HashSet<String>,
}

#[derive(Component, Reflect)]
struct Opaque;

#[derive(Component)]
struct Unregistered;

#[derive(Component, Reflect)]
#[reflect(Component)]
struct Missing;

fn app() -> App {
    let mut app = App::new();
    app.register_type::<Sample>()
        .register_type::<Opaque>()
        .register_type::<Missing>();
    app
}

fn path<T: TypePath>() -> String {
    T::type_path().into()
}

fn output(
    world: &World,
    handle: Option<Handle>,
    projection: entity::Projection,
) -> serde_json::Value {
    serde_json::to_value(query(world, handle, &[], &[], projection).unwrap()).unwrap()
}

#[test]
fn summaries_filters_and_resource_isolation() {
    let mut app = app();
    let first = app.world_mut().spawn(Name::new("context-menu-button")).id();
    let second = app.world_mut().spawn((Opaque, Unregistered)).id();
    let third = app.world_mut().spawn(Opaque).id();
    let world = app.world();
    let summaries = output(world, None, entity::Projection::Summary {});
    assert_eq!(
        summaries,
        json!({"items": [
            {"kind":"entity", "entity":Handle::from(first),
             "result":{"kind":"summary","name":"context-menu-button","component_count":1}},
            {"kind":"entity", "entity":Handle::from(second),
             "result":{"kind":"summary","name":null,"component_count":2}},
            {"kind":"entity", "entity":Handle::from(third),
             "result":{"kind":"summary","name":null,"component_count":1}}
        ]})
    );
    let filtered = query(
        world,
        None,
        &[path::<Opaque>()],
        &[],
        entity::Projection::Summary {},
    )
    .unwrap();
    assert_eq!(filtered.items.len(), 2);
    for (with, without, handle) in [
        (vec![path::<Opaque>()], vec![path::<Opaque>()], None),
        (vec![path::<Missing>()], vec![], None),
        (vec![path::<Opaque>()], vec![], Some(first.into())),
    ] {
        assert!(
            query(
                world,
                handle,
                &with,
                &without,
                entity::Projection::Summary {}
            )
            .unwrap()
            .items
            .is_empty()
        );
    }
    let excluded = query(
        world,
        None,
        &[],
        &[path::<Opaque>()],
        entity::Projection::Summary {},
    )
    .unwrap();
    assert_eq!(excluded.items.len(), 1);
    let resource = world.resource_entities().iter().next().unwrap().1;
    assert_eq!(
        query(
            world,
            Some(resource.into()),
            &[],
            &[],
            entity::Projection::Summary {}
        )
        .unwrap_err()
        .code,
        "entity_not_found"
    );
}

#[test]
fn validates_all_paths_before_resolving_handles_or_filtering() {
    let mut app = app();
    let dead = app.world_mut().spawn_empty().id();
    app.world_mut().despawn(dead);
    app.world_mut().spawn_empty();
    let world = app.world();
    for handle in [Handle::from(dead), Handle::new(u32::MAX, 0)] {
        assert_eq!(
            query(
                world,
                Some(handle),
                &[],
                &[],
                entity::Projection::Summary {}
            )
            .unwrap_err()
            .code,
            "entity_not_found"
        );
        for (with, without, projection) in [
            (
                vec!["Opaque".into()],
                vec![],
                entity::Projection::Summary {},
            ),
            (
                vec![],
                vec!["unknown".into()],
                entity::Projection::Summary {},
            ),
            (
                vec![],
                vec![],
                entity::Projection::Components {
                    selection: component::Selection::Listed {
                        type_paths: vec![path::<Sample>(), "unknown".into()],
                    },
                },
            ),
        ] {
            assert_eq!(
                query(world, Some(handle), &with, &without, projection)
                    .unwrap_err()
                    .code,
                "unknown_type_path"
            );
        }
    }
}

#[test]
fn component_values_preserve_list_order_and_distinguish_unavailable_states() {
    let mut app = app();
    let id = app
        .world_mut()
        .spawn((
            Sample {
                number: 2.0,
                labels: ["z".into(), "a".into()].into_iter().collect(),
            },
            Opaque,
            Unregistered,
        ))
        .id();
    let projection = entity::Projection::Components {
        selection: component::Selection::Listed {
            type_paths: vec![
                path::<Missing>(),
                path::<Sample>(),
                path::<Opaque>(),
                path::<Sample>(),
            ],
        },
    };
    let listed = output(app.world(), Some(id.into()), projection.clone());
    let components = &listed["items"][0]["result"]["components"];
    assert_eq!(
        components[0]["value"],
        json!({"status":"unavailable","reason":"missing"})
    );
    assert_eq!(
        components[1]["value"],
        json!({"status":"readable","value":{"number":2.0,"labels":["a","z"]}})
    );
    assert_eq!(
        components[2]["value"],
        json!({"status":"unavailable","reason":"not_reflectable"})
    );
    assert_eq!(components[3], components[1]);
    assert_eq!(components[0]["component"]["type_path"], path::<Missing>());

    let all = output(
        app.world(),
        Some(id.into()),
        entity::Projection::Components {
            selection: component::Selection::All {},
        },
    );
    let values = all["items"][0]["result"]["components"].as_array().unwrap();
    assert_eq!(values.len(), 3);
    let unregistered = values
        .iter()
        .find(|value| value["component"]["type_path"].is_null())
        .unwrap();
    assert_eq!(
        unregistered["value"],
        json!({"status":"unavailable","reason":"not_registered"})
    );
    let names = output(
        app.world(),
        Some(id.into()),
        entity::Projection::ComponentNames {},
    );
    assert_eq!(
        names["items"][0]["result"]["components"],
        json!(
            values
                .iter()
                .map(|value| &value["component"])
                .collect::<Vec<_>>()
        )
    );
    let keys: Vec<_> = values
        .iter()
        .map(|value| {
            let metadata = &value["component"];
            (
                metadata["type_path"]
                    .as_str()
                    .unwrap_or(metadata["name"].as_str().unwrap()),
                metadata["name"].as_str().unwrap(),
            )
        })
        .collect();
    assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]));

    app.world_mut().get_mut::<Sample>(id).unwrap().number = f64::INFINITY;
    let nonfinite = output(app.world(), Some(id.into()), projection);
    assert_eq!(
        nonfinite["items"][0]["result"]["components"][1]["value"],
        json!({"status":"unavailable","reason":"not_serializable"})
    );
    assert_eq!(
        output(
            app.world(),
            Some(id.into()),
            entity::Projection::ComponentNames {}
        ),
        names
    );
}

#[test]
fn hierarchy_depth_counts_edges_and_preserves_children_order() {
    let mut app = app();
    let root = app.world_mut().spawn(Name::new("root")).id();
    let first = app.world_mut().spawn_empty().id();
    let second = app.world_mut().spawn(Name::new("second")).id();
    let grandchild = app.world_mut().spawn_empty().id();
    app.world_mut()
        .entity_mut(root)
        .add_children(&[second, first]);
    app.world_mut().entity_mut(first).add_child(grandchild);
    let node = |id: Entity, name: Option<&str>, children: Vec<serde_json::Value>| json!({"entity":Handle::from(id),"name":name,"children":children});
    for (depth, expected) in [
        (0, node(root, Some("root"), vec![])),
        (
            1,
            node(
                root,
                Some("root"),
                vec![
                    node(second, Some("second"), vec![]),
                    node(first, None, vec![]),
                ],
            ),
        ),
        (
            2,
            node(
                root,
                Some("root"),
                vec![
                    node(second, Some("second"), vec![]),
                    node(first, None, vec![node(grandchild, None, vec![])]),
                ],
            ),
        ),
    ] {
        let result = output(
            app.world(),
            Some(root.into()),
            entity::Projection::Hierarchy { depth },
        );
        assert_eq!(result["items"][0]["result"]["root"], expected);
    }
    // Component filters select roots, not the descendants of each selected root.
    app.register_type::<Name>();
    let filtered = query(
        app.world(),
        Some(root.into()),
        &[path::<Name>()],
        &[],
        entity::Projection::Hierarchy { depth: 255 },
    )
    .unwrap();
    let result = serde_json::to_value(filtered).unwrap();
    assert_eq!(
        result["items"][0]["result"]["root"]["children"][1]["entity"],
        json!(Handle::from(first))
    );
}
