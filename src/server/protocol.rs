//! One outer codec shared by server and client; independent of game envelopes.
use crate::{command::Command, session};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const VERSION: u32 = 1;
pub const PREFIX: &str = "/v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    pub launch: session::launch::Config,
    pub tick: crate::command::tick::Config,
    pub report: crate::report::Config,
}
impl Create {
    pub fn validate(&self) -> std::result::Result<(), session::Error> {
        self.launch.validate()?;
        self.report.validate()?;
        if !self.tick.pace.is_valid() {
            return Err(session::Error::new("invalid_config", "invalid pace"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    pub session_id: String,
    pub position: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub call: u64,
    pub operation: Operation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Snapshot {},
    Submit {
        command: Command,
    },
    Poll {
        cursor: Option<Cursor>,
        wait_ms: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub version: u32,
    pub call: u64,
    pub result: Result,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Result {
    Snapshot {
        snapshot: Snapshot,
    },
    Pending {
        request_id: u64,
        command: String,
    },
    Activity {
        entries: Vec<Activity>,
        cursor: Cursor,
    },
    Gap {
        cursor: Cursor,
        message: String,
    },
    Error {
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    pub cursor: Cursor,
    pub event: Value,
}

/// Current server-owned commands, independent of Activity retention.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub cursor: Cursor,
    pub state: Lifecycle,
    pub pending: Vec<OpenCommand>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenCommand {
    pub request_id: u64,
    pub command: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Lifecycle {
    Starting,
    Ready,
    Stopping,
    Ended,
    Failed,
}
impl Lifecycle {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Ended | Self::Failed)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Detail {
    pub id: String,
    pub state: Lifecycle,
    pub created_at: u128,
    pub artifact_dir: std::path::PathBuf,
    pub error: Option<session::Error>,
}

impl From<session::Error> for Result {
    fn from(e: session::Error) -> Self {
        Self::Error {
            code: e.code().into(),
            message: e.message().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_fixtures() {
        let text = r#"{"version":1,"call":1,"operation":{"kind":"submit","command":{"command":"tick.warp.start","arguments":{"ticks":3}}}}"#;
        let request: Request = serde_json::from_str(text).unwrap();
        assert_eq!(serde_json::to_string(&request).unwrap(), text);
        assert!(
            serde_json::from_str::<Request>(
                &text.replace("\"ticks\":3", "\"ticks\":3,\"extra\":0")
            )
            .is_err()
        );
        let response = Response {
            version: VERSION,
            call: 1,
            result: Result::Pending {
                request_id: 1,
                command: "tick.warp.start".into(),
            },
        };
        assert_eq!(
            serde_json::to_string(&response).unwrap(),
            r#"{"version":1,"call":1,"result":{"kind":"pending","request_id":1,"command":"tick.warp.start"}}"#
        );
    }
}
