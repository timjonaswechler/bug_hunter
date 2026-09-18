use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub column: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BacktraceStatus {
    Captured,
    Disabled,
    Unsupported,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Origin {
    Panic {
        location: Option<Location>,
        backtrace: Option<String>,
        backtrace_status: BacktraceStatus,
    },
    ProcessExit {
        status: String,
    },
    TracingError {
        target: Option<String>,
        location: Option<Location>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    origin: Origin,
    message: Option<String>,
}

impl Failure {
    pub fn panic(
        message: Option<String>,
        location: Option<Location>,
        backtrace: Option<String>,
    ) -> Self {
        let status = if backtrace.is_some() {
            BacktraceStatus::Captured
        } else {
            BacktraceStatus::Unavailable
        };
        Self::captured_panic(message, location, backtrace, status)
    }

    pub(crate) fn captured_panic(
        message: Option<String>,
        location: Option<Location>,
        backtrace: Option<String>,
        backtrace_status: BacktraceStatus,
    ) -> Self {
        Self {
            origin: Origin::Panic {
                location,
                backtrace,
                backtrace_status,
            },
            message,
        }
    }

    pub fn process_exit(status: String) -> Self {
        Self {
            origin: Origin::ProcessExit { status },
            message: Some("process exited unexpectedly".into()),
        }
    }

    pub fn tracing_error(
        message: String,
        target: Option<String>,
        location: Option<Location>,
    ) -> Self {
        Self {
            origin: Origin::TracingError { target, location },
            message: Some(message),
        }
    }

    pub fn origin(&self) -> &Origin {
        &self.origin
    }
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}
