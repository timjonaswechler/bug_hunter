//! Read-only World observations with selectors, projections, and stateless pagination.
//!
//! Entity-backed selectors are ordered deterministically by session-local [`Handle`]. Results use
//! `{items,total,next_cursor}` pages. Reflection projections bound component output so a Controller
//! cannot accidentally request unbounded serialized state.

use crate::{
    entity::Handle, keyboard::State as KeyboardState, pointer::State as PointerState,
    target::AutomationTarget, text::State as TextState, time::Clock,
};
use bevy::{
    camera::{
        NormalizedRenderTarget,
        visibility::{InheritedVisibility, Visibility},
    },
    ecs::{
        entity::ContainsEntity,
        hierarchy::{ChildOf, Children},
        reflect::{AppTypeRegistry, ReflectComponent},
    },
    picking::pointer::{PointerId, PointerInteraction, PointerLocation, PointerPress},
    prelude::*,
    reflect::serde::TypedReflectSerializer,
    ui::{ComputedNode, Node, UiGlobalTransform},
};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer, de::Error as DeError, ser::SerializeMap,
};
use serde_json::{Map, Value, json};
use std::{collections::BTreeSet, fmt};

/// Default maximum items returned by one observation page.
pub const DEFAULT_LIMIT: u32 = 64;
/// Largest accepted page limit.
pub const MAX_LIMIT: u32 = 256;
/// Largest accepted descendant depth for [`Projection::Hierarchy`].
pub const MAX_HIERARCHY_DEPTH: u8 = 32;
/// Maximum serialized JSON bytes returned for one reflected component value.
pub const MAX_COMPONENT_BYTES: usize = 64 * 1024;

/// Selector, projection, and stateless page position for one observation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Set of entities or session resource to inspect.
    pub selector: Selector,
    /// Shape requested for each selected item.
    pub projection: Projection,
    /// Maximum page size, from one through [`MAX_LIMIT`].
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Zero-based offset into the current deterministic selection.
    #[serde(default)]
    pub cursor: Option<u32>,
}

/// Compatibility name for [`Request`].
pub type ObservationRequest = Request;

impl Request {
    /// Creates a first-page request using [`DEFAULT_LIMIT`].
    pub fn new(selector: Selector, projection: Projection) -> Self {
        Self {
            selector,
            projection,
            limit: DEFAULT_LIMIT,
            cursor: None,
        }
    }

    /// Validates pagination and hierarchy bounds.
    pub fn validate(&self) -> Result<(), Error> {
        if self.limit == 0 || self.limit > MAX_LIMIT {
            return Err(Error::InvalidLimit(self.limit));
        }
        if self
            .cursor
            .is_some_and(|cursor| cursor > MAX_LIMIT * MAX_LIMIT)
        {
            return Err(Error::InvalidCursor);
        }
        if let Projection::Hierarchy { depth } = self.projection {
            if depth > MAX_HIERARCHY_DEPTH {
                return Err(Error::InvalidDepth(depth));
            }
        }
        Ok(())
    }
}

fn default_limit() -> u32 {
    DEFAULT_LIMIT
}

/// Read-only selection scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Selector {
    /// Entities marked with [`AutomationTarget`].
    Targets,
    /// Entities containing Bevy UI [`Node`] components.
    Ui,
    /// Entities containing Bevy [`PointerId`] components, distinct from Virtual Input state.
    Pointers,
    /// One live session-local entity.
    Entity(Handle),
    /// Session-local pointer, keyboard, and text resources; supports only summary projection.
    VirtualInput,
    /// Session-local controlled [`Clock`]; supports only summary projection.
    Clock,
}

impl Serialize for Selector {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        match self {
            Self::Targets => map.serialize_entry("type", "targets")?,
            Self::Ui => map.serialize_entry("type", "ui")?,
            Self::Pointers => map.serialize_entry("type", "pointers")?,
            Self::Entity(handle) => {
                map.serialize_entry("type", "entity")?;
                map.serialize_entry("index", &handle.index)?;
                map.serialize_entry("generation", &handle.generation)?;
            }
            Self::VirtualInput => map.serialize_entry("type", "virtual_input")?,
            Self::Clock => map.serialize_entry("type", "clock")?,
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Selector {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut object = Map::<String, Value>::deserialize(deserializer)?;
        let kind = object
            .remove("type")
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or_else(|| D::Error::custom("observation selector.type must be a string"))?;
        match kind.as_str() {
            "targets" if object.is_empty() => Ok(Self::Targets),
            "ui" if object.is_empty() => Ok(Self::Ui),
            "pointers" if object.is_empty() => Ok(Self::Pointers),
            "entity" => serde_json::from_value(Value::Object(object))
                .map(Self::Entity)
                .map_err(D::Error::custom),
            "virtual_input" if object.is_empty() => Ok(Self::VirtualInput),
            "clock" if object.is_empty() => Ok(Self::Clock),
            other => Err(D::Error::custom(format!(
                "unsupported observation selector {other:?}"
            ))),
        }
    }
}

/// Output shape for selected entities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Projection {
    /// Stable built-in identity, target/UI/pointer metadata, and visibility summary.
    Summary,
    /// Sorted reflected/registered type paths plus stable fallback names for opaque or
    /// unregistered components attached to the entity.
    ComponentNames,
    /// Status and optional serialized value for each requested reflected component.
    ///
    /// Statuses are `available`, `not_present`, `not_registered`, `not_reflectable`,
    /// `not_serializable`, and `value_too_large`.
    Components {
        /// Fully qualified reflected component type paths.
        type_paths: Vec<String>,
    },
    /// Nested `{ "entity": handle, "children": [...] }` nodes through `depth` child edges.
    ///
    /// Depth zero returns the selected entity with an empty `children` array.
    Hierarchy {
        /// Maximum number of child edges below each selected entity.
        depth: u8,
    },
}

/// Observation validation or resolution failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    /// Page limit is zero or exceeds [`MAX_LIMIT`].
    InvalidLimit(u32),
    /// Cursor is outside the supported or current selection range.
    InvalidCursor,
    /// Hierarchy depth exceeds [`MAX_HIERARCHY_DEPTH`].
    InvalidDepth(u8),
    /// Entity handle does not resolve in the current World.
    UnknownEntity(Handle),
    /// A requested reflected component type path is empty.
    InvalidComponentPath(String),
    /// Virtual Input resources support only [`Projection::Summary`].
    UnsupportedVirtualInputProjection,
    /// Controlled clock resources support only [`Projection::Summary`].
    UnsupportedClockProjection,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimit(limit) => write!(
                formatter,
                "limit must be between 1 and {MAX_LIMIT}, got {limit}"
            ),
            Self::InvalidCursor => formatter.write_str("cursor is outside the supported range"),
            Self::InvalidDepth(depth) => write!(
                formatter,
                "hierarchy depth must be at most {MAX_HIERARCHY_DEPTH}, got {depth}"
            ),
            Self::UnknownEntity(handle) => write!(formatter, "entity handle {handle} is not live"),
            Self::InvalidComponentPath(path) => {
                write!(formatter, "component type path must not be empty: {path:?}")
            }
            Self::UnsupportedVirtualInputProjection => {
                formatter.write_str("virtual_input supports only the summary projection")
            }
            Self::UnsupportedClockProjection => {
                formatter.write_str("clock supports only the summary projection")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Computes an observation directly from the current World without a cache.
///
/// Entity-backed results have the JSON shape `{ "items": [...], "total": n,
/// "next_cursor": number|null }` and are sorted by [`Handle`]. Cursors are stateless offsets, so
/// changes to the World between calls may change page membership. Virtual Input and clock summary
/// selectors use the same page envelope with one resource item.
///
/// Reflected component values are limited by [`MAX_COMPONENT_BYTES`] after JSON serialization.
/// Hierarchy results contain nested `{ "entity": handle, "children": [...] }` nodes. Depth counts
/// child edges, and depth zero returns the selected entity with an empty `children` array.
///
/// Target discovery and a second page can be requested as follows:
///
/// ```
/// use automation_control::observation::{Projection, Request, Selector};
/// let first = Request { limit: 10, ..Request::new(Selector::Targets, Projection::Summary) };
/// let second = Request { cursor: Some(10), ..first.clone() };
/// assert!(first.validate().is_ok() && second.validate().is_ok());
/// ```
///
/// Component and clock requests:
///
/// ```
/// use automation_control::{Handle, observation::{Projection, Request, Selector}};
/// let components = Request::new(
///     Selector::Entity(Handle::new(1, 1)),
///     Projection::Components { type_paths: vec!["my_app::Health".into()] },
/// );
/// let clock = Request::new(Selector::Clock, Projection::Summary);
/// assert!(components.validate().is_ok() && clock.validate().is_ok());
/// ```
pub fn observe_world(world: &World, request: &Request) -> Result<Value, Error> {
    request.validate()?;
    if request.selector == Selector::VirtualInput {
        return observe_virtual_input(world, request);
    }
    if request.selector == Selector::Clock {
        return observe_clock(world, request);
    }
    let entities = select_entities(world, &request.selector)?;
    let cursor = request.cursor.unwrap_or(0) as usize;
    if cursor > entities.len() {
        return Err(Error::InvalidCursor);
    }
    let limit = request.limit as usize;
    let end = (cursor + limit).min(entities.len());
    let page = &entities[cursor..end];
    let items = page
        .iter()
        .map(|entity| project_entity(world, *entity, &request.projection))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "items": items,
        "total": entities.len(),
        "next_cursor": (end < entities.len()).then_some(end as u32),
    }))
}

fn select_entities(world: &World, selector: &Selector) -> Result<Vec<Entity>, Error> {
    let mut entities = match selector {
        Selector::Targets => world
            .iter_entities()
            .filter(|entity| entity.contains::<AutomationTarget>())
            .map(|entity| entity.id())
            .collect(),
        Selector::Ui => world
            .iter_entities()
            .filter(|entity| entity.contains::<Node>())
            .map(|entity| entity.id())
            .collect(),
        Selector::Pointers => world
            .iter_entities()
            .filter(|entity| entity.contains::<PointerId>())
            .map(|entity| entity.id())
            .collect(),
        Selector::Entity(handle) => vec![
            handle
                .resolve(world)
                .map_err(|_| Error::UnknownEntity(*handle))?,
        ],
        Selector::VirtualInput => unreachable!("virtual input is observed from resources"),
        Selector::Clock => unreachable!("clock is observed from a resource"),
    };
    entities.sort_by_key(|entity| Handle::from(*entity));
    Ok(entities)
}

fn observe_virtual_input(world: &World, request: &Request) -> Result<Value, Error> {
    if request.projection != Projection::Summary {
        return Err(Error::UnsupportedVirtualInputProjection);
    }
    let cursor = request.cursor.unwrap_or(0);
    if cursor > 1 {
        return Err(Error::InvalidCursor);
    }
    let items = if cursor == 0 {
        vec![json!({
            "pointer": world
                .get_resource::<PointerState>()
                .map(PointerState::observation)
                .unwrap_or_else(|| json!({
                    "position": null,
                    "surface": null,
                    "pressed": [],
                    "scroll_delta": [0.0, 0.0],
                })),
            "keyboard": world
                .get_resource::<KeyboardState>()
                .map(KeyboardState::observation)
                .unwrap_or_else(|| json!({"pressed": []})),
            "text": world
                .get_resource::<TextState>()
                .map(|state| state.observation(world))
                .unwrap_or_else(|| json!({
                    "focused": null,
                    "last_target": null,
                    "last_text": null,
                    "commits": 0,
                })),
        })]
    } else {
        Vec::new()
    };
    Ok(json!({
        "items": items,
        "total": 1,
        "next_cursor": Option::<u32>::None,
    }))
}

fn observe_clock(world: &World, request: &Request) -> Result<Value, Error> {
    if request.projection != Projection::Summary {
        return Err(Error::UnsupportedClockProjection);
    }
    let cursor = request.cursor.unwrap_or(0);
    if cursor > 1 {
        return Err(Error::InvalidCursor);
    }
    let items = if cursor == 0 {
        vec![
            world
                .get_resource::<Clock>()
                .map(Clock::observation)
                .unwrap_or_else(|| Clock::default().observation()),
        ]
    } else {
        Vec::new()
    };
    Ok(json!({
        "items": items,
        "total": 1,
        "next_cursor": Option::<u32>::None,
    }))
}

fn project_entity(world: &World, entity: Entity, projection: &Projection) -> Result<Value, Error> {
    match projection {
        Projection::Summary => {
            if world
                .get_entity(entity)
                .is_ok_and(|value| value.contains::<PointerId>())
            {
                Ok(pointer_summary(world, entity))
            } else {
                Ok(entity_summary(world, entity))
            }
        }
        Projection::ComponentNames => Ok(json!({
            "entity": Handle::from(entity),
            "components": component_names(world, entity),
        })),
        Projection::Components { type_paths } => component_values(world, entity, type_paths),
        Projection::Hierarchy { depth } => hierarchy(world, entity, *depth),
    }
}

fn entity_summary(world: &World, entity: Entity) -> Value {
    let entity_ref = world
        .get_entity(entity)
        .expect("selected entities are live");
    let name = entity_ref
        .get::<Name>()
        .map(|name| name.as_str().to_owned());
    let parent = entity_ref.get::<ChildOf>().and_then(|parent| {
        world
            .get_entity(parent.parent())
            .is_ok()
            .then(|| Handle::from(parent.parent()))
    });
    let mut children = entity_ref
        .get::<Children>()
        .map(|children| {
            children
                .iter()
                .filter(|child| world.get_entity(*child).is_ok())
                .map(Handle::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    children.sort();
    let visible = entity_ref
        .get::<InheritedVisibility>()
        .map(|visibility| visibility.get())
        .or_else(|| {
            entity_ref
                .get::<Visibility>()
                .map(|visibility| *visibility == Visibility::Visible)
        });
    let bounds = ui_bounds(
        entity_ref.get::<ComputedNode>(),
        entity_ref.get::<UiGlobalTransform>(),
    );
    json!({
        "entity": Handle::from(entity),
        "name": name,
        "parent": parent,
        "children": children,
        "visible": visible,
        "bounds": bounds,
    })
}

fn ui_bounds(node: Option<&ComputedNode>, transform: Option<&UiGlobalTransform>) -> Option<Value> {
    let (Some(node), Some(transform)) = (node, transform) else {
        return None;
    };
    let inverse_scale_factor = node.inverse_scale_factor;
    let (scale, _rotation, physical_translation) = transform.to_scale_angle_translation();
    let logical_size = node.size() * scale.abs() * inverse_scale_factor;
    let logical_translation = physical_translation * inverse_scale_factor;
    (inverse_scale_factor.is_finite()
        && inverse_scale_factor > 0.0
        && logical_size.is_finite()
        && logical_translation.is_finite())
    .then(|| {
        json!({
            "x": logical_translation.x - logical_size.x / 2.0,
            "y": logical_translation.y - logical_size.y / 2.0,
            "width": logical_size.x,
            "height": logical_size.y,
        })
    })
}

fn pointer_summary(world: &World, entity: Entity) -> Value {
    let entity_ref = world.get_entity(entity).expect("selected pointer is live");
    let id = entity_ref.get::<PointerId>().map(pointer_id);
    let location = entity_ref.get::<PointerLocation>().and_then(|location| {
        location.location().map(|location| {
            json!({
                "position": [location.position.x, location.position.y],
                "surface": surface_handle(&location.target),
            })
        })
    });
    let pressed = entity_ref.get::<PointerPress>().map(|press| {
        [
            ("primary", press.is_primary_pressed()),
            ("secondary", press.is_secondary_pressed()),
            ("middle", press.is_middle_pressed()),
        ]
        .into_iter()
        .filter_map(|(button, pressed)| pressed.then_some(button))
        .collect::<Vec<_>>()
    });
    let interactions = entity_ref.get::<PointerInteraction>().map(|interaction| {
        interaction
            .iter()
            .map(|(entity, _)| Handle::from(*entity))
            .collect::<Vec<_>>()
    });
    let hovered = entity_ref
        .get::<PointerInteraction>()
        .map(|interaction| !interaction.is_empty());
    Ok::<_, ()>(json!({
        "entity": Handle::from(entity),
        "id": id,
        "location": location,
        "pressed": pressed,
        "hovered": hovered,
        "interactions": interactions,
    }))
    .unwrap()
}

fn pointer_id(id: &PointerId) -> Value {
    match id {
        PointerId::Mouse => json!("mouse"),
        PointerId::Touch(value) => json!({"touch": value}),
        PointerId::Custom(value) => json!({"custom": value.to_string()}),
    }
}

fn surface_handle(target: &NormalizedRenderTarget) -> Option<Handle> {
    match target {
        NormalizedRenderTarget::Window(window) => Some(Handle::from(window.entity())),
        _ => None,
    }
}

fn component_names(world: &World, entity: Entity) -> Vec<String> {
    let registry = world
        .get_resource::<AppTypeRegistry>()
        .map(|registry| registry.0.read());
    let entity_ref = world.get_entity(entity).expect("selected entity is live");
    let mut names = entity_ref
        .archetype()
        .iter_components()
        .filter_map(|component| world.components().get_info(component))
        .map(|info| {
            registry
                .as_ref()
                .and_then(|registry| {
                    info.type_id().and_then(|type_id| {
                        registry
                            .iter()
                            .find(|registration| registration.type_id() == type_id)
                            .map(|registration| registration.type_info().type_path().to_owned())
                    })
                })
                .unwrap_or_else(|| info.name().to_string())
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn component_values(world: &World, entity: Entity, requested: &[String]) -> Result<Value, Error> {
    if requested.iter().any(|path| path.trim().is_empty()) {
        return Err(Error::InvalidComponentPath("".into()));
    }
    let mut values = Map::new();
    let registry_guard = world
        .get_resource::<AppTypeRegistry>()
        .map(|registry| registry.0.read());
    let entity_ref = world.get_entity(entity).expect("selected entity is live");
    for path in requested {
        let Some(registry) = registry_guard.as_ref() else {
            values.insert(path.clone(), json!({"status": "not_registered"}));
            continue;
        };
        let Some(registration) = registry
            .iter()
            .find(|registration| registration.type_info().type_path() == path)
        else {
            values.insert(path.clone(), json!({"status": "not_registered"}));
            continue;
        };
        let Some(component) = registration.data::<ReflectComponent>() else {
            values.insert(path.clone(), json!({"status": "not_reflectable"}));
            continue;
        };
        let type_id = registration.type_id();
        if !entity_ref.contains_type_id(type_id) {
            values.insert(path.clone(), json!({"status": "not_present"}));
            continue;
        }
        let filtered = entity_ref.clone().into_filtered();
        let Some(value) = component.reflect(filtered) else {
            values.insert(path.clone(), json!({"status": "not_reflectable"}));
            continue;
        };
        match serde_json::to_value(TypedReflectSerializer::new(value, registry)) {
            Ok(value) => {
                if serde_json::to_vec(&value)
                    .map_or(true, |bytes| bytes.len() > MAX_COMPONENT_BYTES)
                {
                    values.insert(path.clone(), json!({"status": "value_too_large"}));
                } else {
                    values.insert(path.clone(), json!({"status": "available", "value": value}));
                }
            }
            Err(_) => {
                values.insert(path.clone(), json!({"status": "not_serializable"}));
            }
        }
    }
    Ok(json!({"entity": Handle::from(entity), "components": values}))
}

fn hierarchy(world: &World, entity: Entity, depth: u8) -> Result<Value, Error> {
    fn visit(world: &World, entity: Entity, remaining: u8, seen: &mut BTreeSet<Handle>) -> Value {
        let handle = Handle::from(entity);
        if !seen.insert(handle) {
            return json!({"entity": handle, "cycle": true, "children": []});
        }
        let children = if remaining == 0 {
            Vec::new()
        } else {
            world
                .get_entity(entity)
                .ok()
                .and_then(|entity| entity.get::<Children>())
                .map(|children| {
                    let mut children = children
                        .iter()
                        .filter(|child| world.get_entity(*child).is_ok())
                        .collect::<Vec<_>>();
                    children.sort_by_key(|child| Handle::from(*child));
                    children
                        .into_iter()
                        .map(|child| visit(world, child, remaining - 1, seen))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        json!({"entity": handle, "children": children})
    }
    Ok(visit(world, entity, depth, &mut BTreeSet::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        camera::RenderTarget,
        picking::pointer::{Location, PointerLocation},
        reflect::TypePath,
        window::{PrimaryWindow, Window, WindowRef},
    };

    #[derive(Component)]
    struct OpaqueComponent;

    #[derive(Component, Reflect)]
    #[reflect(Component)]
    struct ReflectValue {
        value: u32,
    }

    #[derive(Component, Reflect)]
    #[reflect(Component)]
    struct OtherReflectValue {
        value: u32,
    }

    #[derive(Component, Reflect)]
    struct RegisteredWithoutComponentReflection;

    #[derive(Clone, Component, Reflect)]
    #[reflect(opaque, Component)]
    struct UnserializableReflectValue;

    #[derive(Component, Reflect)]
    #[reflect(Component)]
    struct LargeReflectValue {
        value: String,
    }

    fn register_reflection(world: &mut World) {
        world.init_resource::<AppTypeRegistry>();
        let registry = world.resource::<AppTypeRegistry>().clone();
        let mut registry = registry.write();
        registry.register::<u32>();
        registry.register::<String>();
        registry.register::<ReflectValue>();
        registry.register::<OtherReflectValue>();
        registry.register::<RegisteredWithoutComponentReflection>();
        registry.register::<UnserializableReflectValue>();
        registry.register::<LargeReflectValue>();
    }

    #[test]
    fn target_ui_pointer_and_entity_selectors_return_contextual_summaries() {
        let mut world = World::new();
        let window = world.spawn((Window::default(), PrimaryWindow)).id();
        let target = world
            .spawn((
                AutomationTarget,
                Name::new("target"),
                Node::default(),
                ComputedNode {
                    size: Vec2::new(200.0, 80.0),
                    inverse_scale_factor: 0.5,
                    ..default()
                },
                UiGlobalTransform::from_translation(Vec2::new(200.0, 100.0)),
                Visibility::Visible,
            ))
            .id();
        let surface = RenderTarget::Window(WindowRef::Entity(window))
            .normalize(Some(window))
            .unwrap();
        world.spawn((
            PointerId::Mouse,
            PointerLocation::new(Location {
                target: surface,
                position: Vec2::new(75.0, 30.0),
            }),
        ));

        let targets = observe_world(
            &world,
            &Request::new(Selector::Targets, Projection::Summary),
        )
        .unwrap();
        assert_eq!(targets["items"][0]["entity"], json!(Handle::from(target)));
        assert_eq!(targets["items"][0]["name"], "target");

        let ui = observe_world(&world, &Request::new(Selector::Ui, Projection::Summary)).unwrap();
        assert_eq!(ui["items"][0]["bounds"]["x"], 50.0);
        assert_eq!(ui["items"][0]["bounds"]["y"], 30.0);
        assert_eq!(ui["items"][0]["bounds"]["width"], 100.0);
        assert_eq!(ui["items"][0]["bounds"]["height"], 40.0);

        let pointers = observe_world(
            &world,
            &Request::new(Selector::Pointers, Projection::Summary),
        )
        .unwrap();
        assert_eq!(pointers["items"][0]["id"], "mouse");
        assert_eq!(
            pointers["items"][0]["location"]["position"],
            json!([75.0, 30.0])
        );
        assert_eq!(
            pointers["items"][0]["location"]["surface"],
            json!(Handle::from(window))
        );

        let entity = observe_world(
            &world,
            &Request::new(Selector::Entity(Handle::from(target)), Projection::Summary),
        )
        .unwrap();
        assert_eq!(entity["total"], 1);
    }

    #[test]
    fn observations_are_sorted_limited_and_have_stateless_cursors() {
        let mut world = World::new();
        world.spawn((AutomationTarget, Name::new("second")));
        world.spawn((AutomationTarget, Name::new("first")));
        let request = Request {
            selector: Selector::Targets,
            projection: Projection::Summary,
            limit: 1,
            cursor: None,
        };
        let first = observe_world(&world, &request).unwrap();
        assert_eq!(first["items"].as_array().unwrap().len(), 1);
        assert_eq!(first["next_cursor"], 1);
        let second = observe_world(
            &world,
            &Request {
                cursor: Some(1),
                ..request
            },
        )
        .unwrap();
        assert_eq!(second["items"].as_array().unwrap().len(), 1);
        assert_eq!(
            observe_world(
                &world,
                &Request {
                    selector: Selector::Targets,
                    projection: Projection::Summary,
                    limit: 1,
                    cursor: Some(3),
                },
            )
            .unwrap_err(),
            Error::InvalidCursor
        );
    }

    #[test]
    fn component_names_include_opaque_components_and_values_report_individual_statuses() {
        let mut world = World::new();
        register_reflection(&mut world);
        let entity = world
            .spawn((
                OpaqueComponent,
                ReflectValue { value: 7 },
                RegisteredWithoutComponentReflection,
                UnserializableReflectValue,
                LargeReflectValue {
                    value: "x".repeat(MAX_COMPONENT_BYTES + 1),
                },
            ))
            .id();
        let handle = Handle::from(entity);

        let names = observe_world(
            &world,
            &Request::new(Selector::Entity(handle), Projection::ComponentNames),
        )
        .unwrap();
        let component_names = names["items"][0]["components"].as_array().unwrap();
        assert!(
            component_names.iter().any(|name| name
                .as_str()
                .is_some_and(|name| name.ends_with("OpaqueComponent"))),
            "opaque component missing from {component_names:?}"
        );

        let paths = [
            ReflectValue::type_path(),
            OtherReflectValue::type_path(),
            RegisteredWithoutComponentReflection::type_path(),
            UnserializableReflectValue::type_path(),
            LargeReflectValue::type_path(),
            "not::registered::Component",
        ];
        let values = observe_world(
            &world,
            &Request::new(
                Selector::Entity(handle),
                Projection::Components {
                    type_paths: paths.iter().map(|path| (*path).to_owned()).collect(),
                },
            ),
        )
        .unwrap();
        let components = &values["items"][0]["components"];
        assert_eq!(components[ReflectValue::type_path()]["status"], "available");
        assert_eq!(components[ReflectValue::type_path()]["value"]["value"], 7);
        assert_eq!(
            components[OtherReflectValue::type_path()]["status"],
            "not_present"
        );
        assert_eq!(
            components[RegisteredWithoutComponentReflection::type_path()]["status"],
            "not_reflectable"
        );
        assert_eq!(
            components[UnserializableReflectValue::type_path()]["status"],
            "not_serializable"
        );
        assert_eq!(
            components[LargeReflectValue::type_path()]["status"],
            "value_too_large"
        );
        assert_eq!(
            components["not::registered::Component"]["status"],
            "not_registered"
        );
    }

    #[test]
    fn virtual_input_summary_observes_session_resources() {
        let mut world = World::new();
        world.insert_resource(PointerState {
            position: Some([12.0, 24.0]),
            scroll_delta: [0.0, -3.0],
            ..default()
        });
        world.init_resource::<KeyboardState>();
        world.init_resource::<TextState>();

        let value = observe_world(
            &world,
            &Request::new(Selector::VirtualInput, Projection::Summary),
        )
        .unwrap();
        assert_eq!(
            value["items"][0]["pointer"]["position"],
            json!([12.0, 24.0])
        );
        assert_eq!(
            value["items"][0]["pointer"]["scroll_delta"],
            json!([0.0, -3.0])
        );
        assert_eq!(value["items"][0]["keyboard"]["pressed"], json!([]));
        assert_eq!(value["items"][0]["text"]["commits"], 0);

        assert_eq!(
            observe_world(
                &world,
                &Request::new(Selector::VirtualInput, Projection::ComponentNames),
            )
            .unwrap_err(),
            Error::UnsupportedVirtualInputProjection
        );
    }

    #[test]
    fn clock_summary_reads_the_session_clock_without_mutating_it() {
        let mut world = World::new();
        let mut clock = Clock::default();
        clock.complete_frame(std::time::Duration::from_nanos(20));
        world.insert_resource(clock);

        let value =
            observe_world(&world, &Request::new(Selector::Clock, Projection::Summary)).unwrap();
        assert_eq!(value["items"][0]["frame_index"], 1);
        assert_eq!(value["items"][0]["elapsed_nanoseconds"], 20);
        assert_eq!(value["items"][0]["last_step_nanoseconds"], 20);
        assert_eq!(world.resource::<Clock>().frame_index(), 1);
        assert_eq!(
            observe_world(
                &world,
                &Request::new(Selector::Clock, Projection::ComponentNames),
            )
            .unwrap_err(),
            Error::UnsupportedClockProjection
        );
    }

    #[test]
    fn hierarchy_depth_is_bounded_and_missing_handles_are_typed() {
        let mut world = World::new();
        let parent = world.spawn_empty().id();
        let child = world.spawn_empty().set_parent_in_place(parent).id();
        world.spawn_empty().set_parent_in_place(child);
        let handle = Handle::from(parent);
        let value = observe_world(
            &world,
            &Request::new(Selector::Entity(handle), Projection::Hierarchy { depth: 1 }),
        )
        .unwrap();
        assert_eq!(value["items"][0]["children"].as_array().unwrap().len(), 1);
        assert!(
            value["items"][0]["children"][0]["children"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            Request::new(
                Selector::Entity(handle),
                Projection::Hierarchy {
                    depth: MAX_HIERARCHY_DEPTH + 1,
                },
            )
            .validate()
            .unwrap_err(),
            Error::InvalidDepth(MAX_HIERARCHY_DEPTH + 1)
        );
        let missing = Handle::new(u32::MAX, 0);
        assert_eq!(
            observe_world(
                &world,
                &Request {
                    selector: Selector::Entity(missing),
                    projection: Projection::Summary,
                    limit: 1,
                    cursor: None,
                },
            )
            .unwrap_err(),
            Error::UnknownEntity(missing)
        );
    }
}
