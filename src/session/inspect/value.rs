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
