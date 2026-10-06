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
    fn text_codec_preserves_unicode_and_requires_a_string_and_null_output() {
        use serde_json::json;
        for text in ["", "Grüße 🦜\n東京", "\0\t"] {
            let line =
                json!({"request_id":5,"command":"input.text.input","arguments":{"text":text}})
                    .to_string();
            let (id, command) = decode(&line).unwrap();
            assert_eq!(command.name(), "input.text.input");
            assert_eq!(decode(&encode(id, &command)).unwrap().1, command);
            assert!(command.validate_output(&json!(null)));
            assert!(!command.validate_output(&json!({})));
        }
        for arguments in [
            json!({}),
            json!({"text":null}),
            json!({"text":3}),
            json!({"text":"hi","extra":1}),
        ] {
            let line = json!({"request_id":5,"command":"input.text.input","arguments":arguments})
                .to_string();
            assert!(
                matches!(decode(&line), Err(Message::Rejected { request_id: 5, error, .. }) if error.code == "invalid_arguments")
            );
        }
    }

    #[test]
    fn keyboard_codec_keeps_unknown_tokens_for_correlated_rejection() {
        use serde_json::json;
        for name in ["input.keyboard.press", "input.keyboard.release"] {
            for key in ["a", "shift_left", "KeyA"] {
                let line =
                    json!({"request_id":9,"command":name,"arguments":{"key":key}}).to_string();
                let (id, command) = decode(&line).unwrap();
                assert_eq!(command.name(), name);
                assert_eq!(decode(&encode(id, &command)).unwrap().1, command);
                assert!(command.validate_output(&json!(null)));
                assert!(!command.validate_output(&json!({})));
            }
            for arguments in [
                json!({}),
                json!({"key":null}),
                json!({"key":3}),
                json!({"key":"a","extra":true}),
                json!({"key":["a"]}),
            ] {
                let line = json!({"request_id":9,"command":name,"arguments":arguments}).to_string();
                assert!(
                    matches!(decode(&line), Err(Message::Rejected { request_id: 9, error, .. }) if error.code == "invalid_arguments")
                );
            }
        }
    }

    #[test]
    fn pointer_codec_roundtrips_and_requires_null_outputs() {
        use serde_json::json;
        for (name, arguments) in [
            ("input.pointer.move_to", json!({"position":[1.0, 2.0]})),
            ("input.pointer.move_by", json!({"delta":[-1.0, 2.0]})),
            ("input.pointer.press", json!({"button":"left"})),
            ("input.pointer.release", json!({"button":"middle"})),
            ("input.pointer.scroll", json!({"delta":[0.0, 0.0]})),
        ] {
            let line = json!({"request_id":3,"command":name,"arguments":arguments}).to_string();
            let (id, command) = decode(&line).unwrap();
            assert_eq!(command.name(), name);
            assert_eq!(decode(&encode(id, &command)).unwrap().1, command);
            assert!(command.validate_output(&json!(null)));
            assert!(!command.validate_output(&json!({})));
            for invalid in [json!({}), json!({"extra":0}), json!({"delta":[null, 2.0]})] {
                let line = json!({"request_id":3,"command":name,"arguments":invalid}).to_string();
                assert!(
                    matches!(decode(&line), Err(Message::Rejected { error, .. }) if error.code == "invalid_arguments")
                );
            }
        }
    }

    #[test]
    fn entity_inspect_codec_requires_explicit_fields_and_valid_projections() {
        use serde_json::json;
        let arguments = json!({
            "source":"entities", "entity":null, "with":[], "without":[],
            "projection":{"kind":"summary"}
        });
        for projection in [
            json!({"kind":"summary"}),
            json!({"kind":"component_names"}),
            json!({"kind":"components","selection":{"kind":"all"}}),
            json!({"kind":"components","selection":{"kind":"listed","type_paths":[]}}),
            json!({"kind":"hierarchy","depth":255}),
        ] {
            let mut args = arguments.clone();
            args["projection"] = projection;
            let line =
                json!({"request_id":7,"command":"inspect.query","arguments":args}).to_string();
            let (id, command) = decode(&line).unwrap();
            assert_eq!(id, 7);
            assert_eq!(decode(&encode(id, &command)).unwrap().1, command);
        }
        let mut invalid = Vec::new();
        for field in ["entity", "with", "without", "projection"] {
            let mut args = arguments.clone();
            args.as_object_mut().unwrap().remove(field);
            invalid.push(args);
        }
        for (field, value) in [
            ("selector", json!({"kind":"all"})),
            ("entity", json!({"index":1,"generation":0,"extra":true})),
            ("with", json!(null)),
            ("projection", json!({"kind":"value"})),
            ("projection", json!({"kind":"summary","depth":0})),
            ("projection", json!({"kind":"component_names","extra":0})),
            (
                "projection",
                json!({"kind":"components","selection":{"kind":"all","extra":0}}),
            ),
            ("projection", json!({"kind":"hierarchy","depth":256})),
            (
                "projection",
                json!({"kind":"components","selection":{"kind":"listed"}}),
            ),
        ] {
            let mut args = arguments.clone();
            args[field] = value;
            invalid.push(args);
        }
        for args in invalid {
            let line =
                json!({"request_id":7,"command":"inspect.query","arguments":args}).to_string();
            assert!(
                matches!(decode(&line), Err(Message::Rejected { error, .. }) if error.code == "invalid_arguments"),
                "{line}"
            );
        }
    }

    #[test]
    fn strict_envelopes_and_arguments() {
        for line in [
            r#"{"request_id":1,"command":"tick.warp.stop","arguments":{"extra":1}}"#,
            r#"{"request_id":1,"command":"tick.warp.stop","arguments":{},"extra":1}"#,
            r#"{"request_id":1,"request_id":2,"command":"tick.warp.stop","arguments":{}}"#,
            r#"{"request_id":1,"command":"tick.warp.start","arguments":{"ticks":1,"ticks":2}}"#,
            r#"{"request_id":1,"command":"tick.warp.stop"}"#,
            r#"{"request_id":1,"command":"inspect.query","arguments":{"source":"resources","selector":{"kind":"all","extra":0},"projection":{"kind":"metadata"}}}"#,
            r#"{"request_id":1,"command":"inspect.query","arguments":{"source":"resources","selector":{"kind":"all"},"projection":{"kind":"metadata","extra":0}}}"#,
            r#"{"request_id":1,"command":"inspect.query","arguments":{"source":"resources","selector":{"kind":"all"},"projection":{"kind":"value","extra":0}}}"#,
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
