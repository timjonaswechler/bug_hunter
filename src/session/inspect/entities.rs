use super::{Diagnostic, value};
use crate::{
    command::inspect::{Item, Output, Status, Value, component, entity},
    handle::Handle,
};
use bevy::{
    ecs::{component::ComponentId, reflect::ReflectComponent, world::EntityRef},
    prelude::{AppTypeRegistry, Children, Entity, Name, World},
    reflect::{TypeRegistration, TypeRegistry},
};
use std::collections::HashSet;

pub(super) fn query(
    world: &World,
    handle: Option<Handle>,
    with: &[String],
    without: &[String],
    projection: entity::Projection,
) -> Result<Output, Diagnostic> {
    let registry = world.resource::<AppTypeRegistry>().read();
    // Validate every supplied path before resolving handles or selecting any entities.
    let resolve = |paths: &[String]| {
        paths
            .iter()
            .map(|path| {
                registry
                    .get_with_type_path(path)
                    .ok_or_else(|| Diagnostic::new("unknown_type_path", path))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let with = resolve(with)?;
    let without = resolve(without)?;
    let listed = match &projection {
        entity::Projection::Components {
            selection: component::Selection::Listed { type_paths },
        } => resolve(type_paths)?,
        _ => Vec::new(),
    };
    let resources: HashSet<_> = world.resource_entities().iter().map(|(_, e)| e).collect();
    let mut entities = if let Some(handle) = handle {
        let id = handle
            .resolve(world)
            .ok()
            .filter(|id| !resources.contains(id))
            .ok_or_else(|| Diagnostic::new("entity_not_found", handle))?;
        vec![world.entity(id)]
    } else {
        world
            .iter_entities()
            .filter(|entity| !resources.contains(&entity.id()))
            .collect()
    };
    entities.retain(|entity| {
        let contains = |registration: &&TypeRegistration| {
            world
                .components()
                .get_id(registration.type_id())
                .is_some_and(|id| entity.contains_id(id))
        };
        with.iter().all(contains) && !without.iter().any(contains)
    });
    entities.sort_by_key(|entity| Handle::from(entity.id()));
    let items = entities
        .into_iter()
        .map(|entity| {
            let result = match &projection {
                entity::Projection::Summary {} => entity::Result::Summary {
                    name: name(entity),
                    component_count: entity.archetype().components().len() as u32,
                },
                entity::Projection::ComponentNames {} => entity::Result::ComponentNames {
                    components: components(world, entity, &registry)
                        .into_iter()
                        .map(|(_, metadata)| metadata)
                        .collect(),
                },
                entity::Projection::Components { selection } => {
                    let components = match selection {
                        component::Selection::All {} => components(world, entity, &registry)
                            .into_iter()
                            .map(|(id, component)| {
                                let registration = world
                                    .components()
                                    .get_info(id)
                                    .and_then(|info| info.type_id())
                                    .and_then(|id| registry.get(id));
                                component::Output {
                                    component,
                                    value: read(entity, registration, &registry),
                                }
                            })
                            .collect(),
                        component::Selection::Listed { .. } => listed
                            .iter()
                            .map(|registration| {
                                let id = world.components().get_id(registration.type_id());
                                let component = id
                                    .map(|id| metadata(world, id, &registry))
                                    .unwrap_or_else(|| component::Metadata {
                                        name: registration.type_info().type_path().into(),
                                        type_path: Some(
                                            registration.type_info().type_path().into(),
                                        ),
                                    });
                                component::Output {
                                    component,
                                    value: if id.is_some_and(|id| entity.contains_id(id)) {
                                        read(entity, Some(registration), &registry)
                                    } else {
                                        Value::Unavailable {
                                            reason: Status::Missing,
                                        }
                                    },
                                }
                            })
                            .collect(),
                    };
                    entity::Result::Components { components }
                }
                entity::Projection::Hierarchy { depth } => entity::Result::Hierarchy {
                    root: hierarchy(world, entity, *depth, &resources),
                },
            };
            Item::Entity {
                entity: entity.id().into(),
                result,
            }
        })
        .collect();
    Ok(Output { items })
}

fn name(entity: EntityRef<'_>) -> Option<String> {
    entity.get::<Name>().map(|name| name.as_str().to_owned())
}

fn metadata(world: &World, id: ComponentId, registry: &TypeRegistry) -> component::Metadata {
    let info = world.components().get_info(id).expect("live component");
    component::Metadata {
        name: info.name().to_string(),
        type_path: info
            .type_id()
            .and_then(|id| registry.get(id))
            .map(|registration| registration.type_info().type_path().into()),
    }
}

fn components(
    world: &World,
    entity: EntityRef<'_>,
    registry: &TypeRegistry,
) -> Vec<(ComponentId, component::Metadata)> {
    let mut components: Vec<_> = entity
        .archetype()
        .components()
        .iter()
        .map(|id| (*id, metadata(world, *id, registry)))
        .collect();
    components.sort_by(|(_, a), (_, b)| {
        a.type_path
            .as_deref()
            .unwrap_or(&a.name)
            .cmp(b.type_path.as_deref().unwrap_or(&b.name))
            .then_with(|| a.name.cmp(&b.name))
    });
    components
}

fn read(
    entity: EntityRef<'_>,
    registration: Option<&TypeRegistration>,
    registry: &TypeRegistry,
) -> Value {
    let Some(registration) = registration else {
        return Value::Unavailable {
            reason: Status::NotRegistered,
        };
    };
    match registration
        .data::<ReflectComponent>()
        .and_then(|component| component.reflect(entity.into_filtered()))
    {
        Some(reflected) => value::serialize(reflected, registry),
        None => Value::Unavailable {
            reason: Status::NotReflectable,
        },
    }
}

fn hierarchy(
    world: &World,
    entity: EntityRef<'_>,
    depth: u8,
    resources: &HashSet<Entity>,
) -> entity::hierarchy::Node {
    let children = if depth == 0 {
        Vec::new()
    } else {
        entity
            .get::<Children>()
            .into_iter()
            .flat_map(|children| children.iter())
            .filter(|id| !resources.contains(id))
            .filter_map(|id| world.get_entity(*id).ok())
            .map(|child| hierarchy(world, child, depth - 1, resources))
            .collect()
    };
    entity::hierarchy::Node {
        entity: entity.id().into(),
        name: name(entity),
        children,
    }
}

#[cfg(test)]
mod tests;
