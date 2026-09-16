//! Shared command codec and JSONL game envelopes. This build implements only the slice.
use crate::command::Command;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const VERSION: u32 = 3;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub screenshot: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
}

impl Diagnostic {
    pub fn new(code: &str, message: impl ToString) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    Ready {
        version: u32,
        capabilities: Capabilities,
    },
    Completed {
        request_id: u64,
        command: String,
        output: Value,
    },
    Rejected {
        request_id: u64,
        command: String,
        error: Diagnostic,
    },
    ProtocolError {
        request_id: Option<u64>,
        error: Diagnostic,
    },
}

impl Message {
    pub fn id(&self) -> Option<u64> {
        match self {
            Self::Completed { request_id, .. } | Self::Rejected { request_id, .. } => {
                Some(*request_id)
            }
            Self::ProtocolError { request_id, .. } => *request_id,
            Self::Ready { .. } => None,
        }
    }
}

// Explicit fields avoid serde flatten's incompatibility with deny_unknown_fields.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    request_id: u64,
    command: String,
    arguments: Box<serde_json::value::RawValue>,
}

pub fn encode(id: u64, command: &Command) -> String {
    let mut value = serde_json::to_value(command).expect("command serialization");
    value["request_id"] = id.into();
    value.to_string()
}

pub fn decode(line: &str) -> Result<(u64, Command), Message> {
    let envelope: Envelope =
        serde_json::from_str(line).map_err(|error| Message::ProtocolError {
            request_id: serde_json::from_str::<Value>(line)
                .ok()
                .and_then(|v| v["request_id"].as_u64()),
            error: Diagnostic::new("malformed_request", error),
        })?;
    let raw_command = format!(
        "{{\"command\":{},\"arguments\":{}}}",
        serde_json::to_string(&envelope.command).unwrap(),
        envelope.arguments
    );
    let command = serde_json::from_str(&raw_command).map_err(|error| Message::Rejected {
        request_id: envelope.request_id,
        command: envelope.command,
        error: Diagnostic::new("invalid_arguments", error),
    })?;
    Ok((envelope.request_id, command))
}

pub fn completed(id: u64, command: &str, output: impl Serialize) -> Message {
    Message::Completed {
        request_id: id,
        command: command.into(),
        output: serde_json::to_value(output).expect("output serialization"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_envelopes_and_arguments() {
        for line in [
            r#"{"request_id":1,"command":"tick.warp.stop","arguments":{"extra":1}}"#,
            r#"{"request_id":1,"command":"tick.warp.stop","arguments":{},"extra":1}"#,
            r#"{"request_id":1,"request_id":2,"command":"tick.warp.stop","arguments":{}}"#,
            r#"{"request_id":1,"command":"tick.warp.start","arguments":{"ticks":1,"ticks":2}}"#,
            r#"{"request_id":1,"command":"tick.warp.stop"}"#,
        ] {
            assert!(decode(line).is_err(), "{line}");
        }
        let (id, cmd) =
            decode(r#"{"request_id":17,"command":"tick.warp.start","arguments":{"ticks":3}}"#)
                .unwrap();
        assert_eq!(id, 17);
        assert_eq!(decode(&encode(id, &cmd)).unwrap().1, cmd);
        assert!(
            serde_json::from_str::<Message>(
                r#"{"status":"ready","version":3,"capabilities":{"screenshot":false},"extra":0}"#
            )
            .is_err()
        );
    }
}
