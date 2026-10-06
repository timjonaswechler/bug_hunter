use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    pub command: crate::command::Command,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    Unanswered,
    Completed { output: serde_json::Value },
    Rejected { code: String, message: String },
    ProtocolFailed { code: String, message: String },
    IoFailed { message: String },
}
