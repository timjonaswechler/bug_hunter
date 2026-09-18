//! Fully validated command lists executed through the shared, bound client.
mod execution;
mod launch;
#[cfg(test)]
mod tests;

use crate::{client::Client, command::Command};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};

pub(crate) use launch::run as run_cli;

#[derive(Clone, Debug)]
pub struct Script {
    commands: Vec<Command>,
}

impl Script {
    /// Parses the entire versioned document without IO or submitting any command.
    pub fn parse(source: &str) -> Result<Self, Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Document {
            version: u32,
            commands: Vec<Box<RawValue>>,
        }
        let document: Document =
            serde_json::from_str(source).map_err(|e| Error::invalid(None, e))?;
        if document.version != 1 {
            return Err(Error::invalid(None, "expected script version 1"));
        }
        let commands = document
            .commands
            .iter()
            .enumerate()
            .map(|(index, raw)| {
                serde_json::from_str(raw.get()).map_err(|e| Error::invalid(Some(index), e))
            })
            .collect::<Result<Vec<Command>, _>>()?;
        Self::new(commands)
    }

    /// Applies the same wire-shape and ordering rules to Rust-created commands.
    /// Game preconditions remain the Session's responsibility.
    pub fn new(commands: Vec<Command>) -> Result<Self, Error> {
        let last = commands.len().checked_sub(1);
        let commands = commands
            .into_iter()
            .enumerate()
            .map(|(index, command)| {
                if matches!(command, Command::Shutdown(_)) && Some(index) != last {
                    return Err(Error::invalid(
                        Some(index),
                        "shutdown must be the last command",
                    ));
                }
                // In particular, serde_json would otherwise silently encode non-finite
                // programmatic float values as null. Decode with the shared codec too.
                let json =
                    serde_json::to_string(&command).map_err(|e| Error::invalid(Some(index), e))?;
                serde_json::from_str(&json).map_err(|e| Error::invalid(Some(index), e))
            })
            .collect::<Result<Vec<Command>, _>>()?;
        Ok(Self { commands })
    }

    pub fn commands(&self) -> &[Command] {
        &self.commands
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Passed {
        completed: Vec<Completion>,
    },
    Failed {
        completed: Vec<Completion>,
        failures: Vec<Failure>,
    },
}
impl Outcome {
    pub fn passed(&self) -> bool {
        matches!(self, Self::Passed { .. })
    }
}

#[derive(Debug, Serialize)]
pub struct Completion {
    /// Zero-based position in the original script.
    pub command_index: usize,
    pub request_id: u64,
    pub command: Command,
    pub output: Value,
}

#[derive(Debug, Serialize)]
pub struct Failure {
    pub command_index: usize,
    /// None means no request ID was confirmed, not necessarily no remote acceptance.
    pub request_id: Option<u64>,
    pub command: Command,
    pub reason: Reason,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reason {
    Rejected { code: String, message: String },
    Failed { code: String, message: String },
    Unknown { message: String },
    NotSubmitted { message: String },
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Error {
    InvalidScript {
        command_index: Option<usize>,
        message: String,
    },
    Client {
        message: String,
    },
}
impl Error {
    fn invalid(command_index: Option<usize>, message: impl ToString) -> Self {
        Self::InvalidScript {
            command_index,
            message: message.to_string(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidScript {
                command_index: Some(index),
                message,
            } => write!(f, "invalid_script at command {index}: {message}"),
            Self::InvalidScript {
                command_index: None,
                message,
            } => write!(f, "invalid_script: {message}"),
            Self::Client { message } => write!(f, "script client: {message}"),
        }
    }
}
impl std::error::Error for Error {}

/// Runs a fully validated script. Does not stop accepted work on error or disconnect.
/// Ordinary commands are pipelined; only the documented boundary commands wait.
pub fn run(client: &mut Client, script: &Script) -> Result<Outcome, Error> {
    execution::run(client, script, &std::sync::atomic::AtomicBool::new(false))
}
