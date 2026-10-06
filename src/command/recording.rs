use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stop {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Started {
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stopped {
    pub path: String,
    pub recorded_commands: u64,
}

super::request!(Start, RecordingStart, Started);
super::request!(Stop, RecordingStop, Stopped);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::Command, session::protocol};
    use serde_json::json;

    #[test]
    fn controls_have_strict_arguments_and_correlated_outputs() {
        let start = Command::from(Start {
            path: "records/a.jsonl".into(),
        });
        let stop = Command::from(Stop {});
        for command in [&start, &stop] {
            assert_eq!(
                protocol::decode(&protocol::encode(17, command)).unwrap(),
                (17, command.clone())
            );
            assert!(!command.is_recordable());
        }
        assert!(start.validate_output(&json!({"path":"records/a.jsonl"})));
        assert!(!start.validate_output(&json!({"path":"other.jsonl"})));
        assert!(stop.validate_output(&json!({"path":"records/a.jsonl","recorded_commands":0})));
        assert!(!stop.validate_output(&json!({"path":"records/a.jsonl","recorded_commands":-1})));
        for (command, arguments) in [
            ("recording.start", json!({})),
            ("recording.start", json!({"path":null})),
            ("recording.start", json!({"path":"a.jsonl","extra":0})),
            ("recording.stop", json!({"path":"a.jsonl"})),
        ] {
            assert!(matches!(protocol::decode(&json!({
                "request_id":17,"command":command,"arguments":arguments
            }).to_string()), Err(protocol::Message::Rejected { error, .. }) if error.code == "invalid_arguments"));
        }
    }
}
