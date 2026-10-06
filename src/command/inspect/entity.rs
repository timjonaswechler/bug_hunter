use crate::handle::Handle;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Projection {
    Summary {},
    ComponentNames {},
    Components {
        selection: super::component::Selection,
    },
    Hierarchy {
        depth: u8,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Result {
    Summary {
        name: Option<String>,
        component_count: u32,
    },
    ComponentNames {
        components: Vec<super::component::Metadata>,
    },
    Components {
        components: Vec<super::component::Output>,
    },
    Hierarchy {
        root: hierarchy::Node,
    },
}

pub mod hierarchy {
    use super::*;

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Node {
        pub entity: Handle,
        pub name: Option<String>,
        pub children: Vec<Node>,
    }
}
