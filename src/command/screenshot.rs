use serde::{Deserialize, Serialize};

/// Capture the primary rendered window beneath the session's artifact directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub path: String,
}

impl Capture {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub overwritten: bool,
}

super::request!(Capture, Screenshot, Output);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::protocol;
    use serde_json::json;

    #[test]
    fn codec_requires_path_and_validates_correlated_output() {
        let command = crate::command::Command::from(Capture::new("screenshots/a.png"));
        assert_eq!(
            protocol::decode(&protocol::encode(7, &command)).unwrap(),
            (7, command.clone())
        );
        let output =
            json!({"path": "screenshots/a.png", "width": 640, "height": 360, "overwritten": false});
        assert!(command.validate_output(&output));
        for (field, value) in [
            ("path", json!("other.png")),
            ("width", json!(0)),
            ("height", json!(-1)),
            ("overwritten", json!(null)),
            ("extra", json!(1)),
        ] {
            let mut invalid = output.clone();
            invalid[field] = value;
            assert!(!command.validate_output(&invalid));
        }
        for arguments in [
            json!({}),
            json!({"path": null}),
            json!({"path": "a.png", "extra": 1}),
        ] {
            let line =
                json!({"request_id": 7, "command": "screenshot.capture", "arguments": arguments})
                    .to_string();
            assert!(matches!(protocol::decode(&line),
                Err(protocol::Message::Rejected { error, .. }) if error.code == "invalid_arguments"));
        }
    }
}
