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
pub struct Completion {
    pub outcome: Outcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Completed {},
    Stopped {},
    Blocked { code: String, message: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stopped {
    pub was_running: bool,
}

super::request!(Start, ReplayStart, Completion);
super::request!(Stop, ReplayStop, Stopped);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::Command, session::protocol};
    use serde_json::json;

    #[test]
    fn controls_and_outputs_are_strict_and_never_recorded() {
        for command in [
            Command::from(Start {
                path: "a.jsonl".into(),
            }),
            Stop {}.into(),
        ] {
            assert!(!command.is_recordable());
            assert_eq!(
                protocol::decode(&protocol::encode(7, &command)).unwrap(),
                (7, command)
            );
        }
        for (name, arguments) in [
            ("replay.start", json!({})),
            ("replay.start", json!({"path":null})),
            ("replay.start", json!({"path":"a.jsonl","extra":0})),
            ("replay.stop", json!({"extra":0})),
        ] {
            assert!(
                protocol::decode(
                    &json!({"request_id":7,"command":name,"arguments":arguments}).to_string()
                )
                .is_err()
            );
        }
        let command = Command::from(Start {
            path: "a.jsonl".into(),
        });
        for outcome in [
            json!({"kind":"completed"}),
            json!({"kind":"stopped"}),
            json!({"kind":"blocked","code":"session_ended","message":"ended"}),
        ] {
            assert!(command.validate_output(&json!({"outcome":outcome})));
        }
        for outcome in [
            json!({"kind":"completed","extra":0}),
            json!({"kind":"stopped","extra":0}),
            json!({"kind":"blocked","code":"session_ended"}),
            json!({"kind":"other"}),
        ] {
            assert!(!command.validate_output(&json!({"outcome":outcome})));
        }
    }
}
