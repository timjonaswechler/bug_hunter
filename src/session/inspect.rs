use super::protocol::Diagnostic;
use crate::command::inspect::*;
use bevy::{
    ecs::reflect::{ReflectComponent, ReflectResource},
    prelude::{AppTypeRegistry, World},
};
mod entities;
mod value;

pub(super) fn query(world: &World, query: Command) -> std::result::Result<Output, Diagnostic> {
    match query {
        Command::Entities {
            entity,
            with,
            without,
            projection,
        } => entities::query(world, entity, &with, &without, projection),
        Command::Resources {
            selector,
            projection,
        } => resources(world, selector, projection),
    }
}

fn resources(
    world: &World,
    selector: Selector,
    projection: Projection,
) -> std::result::Result<Output, Diagnostic> {
    let registry = world.resource::<AppTypeRegistry>().read();
    let mut registrations: Vec<_> = match &selector {
        Selector::All {} => registry
            .iter()
            .filter(|r| r.data::<ReflectResource>().is_some())
            .collect(),
        Selector::Type { type_path } => vec![
            registry
                .get_with_type_path(type_path)
                .ok_or_else(|| Diagnostic::new("unknown_type_path", type_path))?,
        ],
    };
    registrations.sort_by_key(|registration| registration.type_info().type_path());
    let mut items = Vec::new();
    for registration in registrations {
        let type_path = registration.type_info().type_path().to_string();
        let entity = world
            .components()
            .get_id(registration.type_id())
            .and_then(|id| world.resource_entities().get(id))
            .and_then(|entity| world.get_entity(entity).ok());
        if entity.is_none()
            && (matches!(projection, Projection::Metadata {})
                || matches!(selector, Selector::All {}))
        {
            continue;
        }
        let result = match projection {
            Projection::Metadata {} => Result::Metadata { type_path },
            Projection::Value {} => {
                let value = match (entity, registration.data::<ReflectComponent>()) {
                    (None, _) => Value::Unavailable {
                        reason: Status::Missing,
                    },
                    (_, None) => Value::Unavailable {
                        reason: Status::NotReflectable,
                    },
                    (Some(entity), Some(component)) => {
                        match component.reflect(entity.into_filtered()) {
                            None => Value::Unavailable {
                                reason: Status::NotReflectable,
                            },
                            Some(reflected) => value::serialize(reflected, &registry),
                        }
                    }
                };
                Result::Value { type_path, value }
            }
        };
        items.push(Item::Resource { result });
    }
    Ok(Output { items })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::inspect::{Command, Projection, Result, Value};
    use bevy::prelude::*;

    #[derive(Resource, Reflect)]
    #[reflect(Resource)]
    struct Sample {
        number: f64,
        labels: std::collections::HashSet<String>,
    }

    #[test]
    fn resource_values_are_strict_read_only_and_sets_are_sorted() {
        let mut app = App::new();
        app.register_type::<Sample>();
        let selector = Selector::Type {
            type_path: std::any::type_name::<Sample>().into(),
        };
        assert!(
            query(
                app.world(),
                Command::Resources {
                    selector: Selector::Type {
                        type_path: "Sample".into()
                    },
                    projection: Projection::Value {},
                }
            )
            .is_err()
        );
        let missing = query(
            app.world(),
            Command::Resources {
                selector: selector.clone(),
                projection: Projection::Value {},
            },
        )
        .unwrap();
        assert!(matches!(
            &missing.items[0],
            Item::Resource {
                result: Result::Value {
                    value: Value::Unavailable {
                        reason: Status::Missing
                    },
                    ..
                }
            }
        ));
        app.insert_resource(Sample {
            number: 2.0,
            labels: ["z".into(), "a".into()].into_iter().collect(),
        });
        let output = query(
            app.world(),
            Command::Resources {
                selector: selector.clone(),
                projection: Projection::Value {},
            },
        )
        .unwrap();
        let value = serde_json::to_value(output).unwrap();
        assert_eq!(
            value["items"][0]["result"]["value"]["value"]["labels"],
            serde_json::json!(["a", "z"])
        );
        app.world_mut().resource_mut::<Sample>().number = f64::NAN;
        let output = query(
            app.world(),
            Command::Resources {
                selector,
                projection: Projection::Value {},
            },
        )
        .unwrap();
        assert!(matches!(
            &output.items[0],
            Item::Resource {
                result: Result::Value {
                    value: Value::Unavailable {
                        reason: Status::NotSerializable
                    },
                    ..
                }
            }
        ));
    }
}
