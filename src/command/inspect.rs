use serde::{Deserialize, Serialize};

pub mod component;
pub mod entity;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Entities {
        #[serde(deserialize_with = "Option::deserialize")]
        entity: Option<crate::handle::Handle>,
        with: Vec<String>,
        without: Vec<String>,
        projection: entity::Projection,
    },
    Resources {
        selector: Selector,
        projection: Projection,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Selector {
    All {},
    Type { type_path: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Projection {
    Metadata {},
    Value {},
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Item {
    Entity {
        entity: crate::handle::Handle,
        result: entity::Result,
    },
    Resource {
        result: Result,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Result {
    Metadata { type_path: String },
    Value { type_path: String, value: Value },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Value {
    Readable { value: serde_json::Value },
    Unavailable { reason: Status },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Missing,
    NotRegistered,
    NotReflectable,
    NotSerializable,
}

crate::command::request!(Command, Inspect, Output);
