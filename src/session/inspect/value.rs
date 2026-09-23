use bevy::{
    asset::{
        EphemeralHandleBehavior, HandleSerializeProcessor, ReflectHandle, UntypedAssetId,
        UntypedHandle,
    },
    reflect::{
        PartialReflect, ReflectRef, TypeRegistry,
        serde::{ReflectSerializerProcessor, TypedReflectSerializer},
    },
};
use serde::{Serialize, ser::Error};

pub(super) fn serialize(
    reflected: &dyn PartialReflect,
    registry: &TypeRegistry,
) -> crate::command::inspect::Value {
    use crate::command::inspect::{Status, Value};
    match serde_json::to_value(TypedReflectSerializer::with_processor(
        reflected, registry, &Processor,
    )) {
        Ok(value) => Value::Readable { value },
        Err(_) => Value::Unavailable {
            reason: Status::NotSerializable,
        },
    }
}

pub(super) struct Processor;

impl ReflectSerializerProcessor for Processor {
    fn try_serialize<S: serde::Serializer>(
        &self,
        value: &dyn PartialReflect,
        registry: &TypeRegistry,
        serializer: S,
    ) -> Result<Result<S::Ok, S>, S::Error> {
        if value
            .try_downcast_ref::<f32>()
            .is_some_and(|n| !n.is_finite())
            || value
                .try_downcast_ref::<f64>()
                .is_some_and(|n| !n.is_finite())
        {
            return Err(S::Error::custom("non-finite reflected float"));
        }
        if let ReflectRef::Set(set) = value.reflect_ref() {
            let mut items = set
                .iter()
                .map(|item| {
                    serde_json::to_value(TypedReflectSerializer::with_processor(
                        item, registry, self,
                    ))
                    .map_err(S::Error::custom)
                })
                .collect::<Result<Vec<_>, _>>()?;
            items.sort_by_cached_key(key);
            return Ok(Ok(items.serialize(serializer)?));
        }
        if let Some(reflected) = value.try_as_reflect() {
            let untyped = reflected.downcast_ref::<UntypedHandle>();
            let typed = registry
                .get_type_data::<ReflectHandle>(reflected.type_id())
                .and_then(|r| r.downcast_handle_untyped(reflected.as_any()));
            if let Some(handle) = untyped.or(typed.as_ref())
                && handle.path().is_none()
                && let UntypedAssetId::Index { index, .. } = handle.id()
            {
                let reference =
                    serde_json::json!({"Ephemeral":{"id":format!("{:016x}", index.to_bits())}});
                let output = if untyped.is_some() {
                    let registration = registry
                        .get(handle.type_id())
                        .ok_or_else(|| S::Error::custom("missing asset registration"))?;
                    serde_json::json!({"asset_type":registration.type_info().type_path(),"reference":reference})
                } else {
                    reference
                };
                return Ok(Ok(output.serialize(serializer)?));
            }
        }
        HandleSerializeProcessor {
            ephemeral_handle_behavior: EphemeralHandleBehavior::Error,
        }
        .try_serialize(value, registry, serializer)
    }
}

fn key(value: &serde_json::Value) -> Vec<u8> {
    // Do not depend on whether a downstream crate enables serde_json/preserve_order.
    fn ordered(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut entries: Vec<_> = map.iter().collect();
                entries.sort_by_key(|(name, _)| *name);
                serde_json::Value::Object(
                    entries
                        .into_iter()
                        .map(|(key, value)| (key.clone(), ordered(value)))
                        .collect(),
                )
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(ordered).collect())
            }
            value => value.clone(),
        }
    }
    serde_json::to_vec(&ordered(value)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::key;
    use serde_json::{Map, Value, json};

    #[test]
    fn set_sort_keys_order_objects_recursively_without_reordering_arrays() {
        // Explicit insertion order also exercises serde_json/preserve_order builds.
        let object = |entries: Vec<(&str, Value)>| {
            let mut map = Map::new();
            for (name, value) in entries {
                map.insert(name.into(), value);
            }
            Value::Object(map)
        };
        let value = object(vec![
            ("z", json!(0)),
            (
                "a",
                json!([object(vec![("z", json!(2)), ("a", json!(1))]), 10, 2]),
            ),
        ]);
        assert_eq!(key(&value), br#"{"a":[{"a":1,"z":2},10,2],"z":0}"#);
    }
}
