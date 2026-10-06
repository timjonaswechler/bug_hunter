use super::protocol::Diagnostic;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "Diagnostic", into = "Diagnostic")]
pub enum Error {
    InvalidConfig { message: String },
    Launch { message: String },
    Io { message: String },
    InvalidPending { message: String },
    RequestIdExhausted,
    ShutdownBlocked { code: String, message: String },
    Rejected { code: String, message: String },
    Protocol { code: String, message: String },
    Ended,
}
impl Error {
    pub(crate) fn new(code: &str, message: impl ToString) -> Self {
        let message = message.to_string();
        match code {
            "invalid_config" | "unsupported_feature" => Self::InvalidConfig { message },
            "launch" => Self::Launch { message },
            "io" => Self::Io { message },
            "invalid_pending" => Self::InvalidPending { message },
            "request_id_exhausted" => Self::RequestIdExhausted,
            "ended" => Self::Ended,
            "shutdown_commands_pending" | "shutdown_recording_active" => Self::ShutdownBlocked {
                code: code.into(),
                message,
            },
            "invalid_ready" | "unsupported_protocol_version" | "invalid_response" => {
                Self::Protocol {
                    code: code.into(),
                    message,
                }
            }
            _ => Self::Rejected {
                code: code.into(),
                message,
            },
        }
    }
    pub(crate) fn io(message: impl ToString) -> Self {
        Self::Io {
            message: message.to_string(),
        }
    }
    pub(crate) fn ended() -> Self {
        Self::Ended
    }
    pub fn code(&self) -> &str {
        match self {
            Self::InvalidConfig { .. } => "invalid_config",
            Self::Launch { .. } => "launch",
            Self::Io { .. } => "io",
            Self::InvalidPending { .. } => "invalid_pending",
            Self::RequestIdExhausted => "request_id_exhausted",
            Self::ShutdownBlocked { code, .. }
            | Self::Rejected { code, .. }
            | Self::Protocol { code, .. } => code,
            Self::Ended => "ended",
        }
    }
    pub fn message(&self) -> &str {
        match self {
            Self::InvalidConfig { message }
            | Self::Launch { message }
            | Self::Io { message }
            | Self::InvalidPending { message }
            | Self::ShutdownBlocked { message, .. }
            | Self::Rejected { message, .. }
            | Self::Protocol { message, .. } => message,
            Self::RequestIdExhausted => "normal request IDs exhausted",
            Self::Ended => "session has ended",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}
impl std::error::Error for Error {}
impl From<Diagnostic> for Error {
    fn from(value: Diagnostic) -> Self {
        Self::new(&value.code, value.message)
    }
}
impl From<Error> for Diagnostic {
    fn from(value: Error) -> Self {
        Self {
            code: value.code().into(),
            message: value.message().into(),
        }
    }
}
