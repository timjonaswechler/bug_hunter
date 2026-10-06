use super::{Completion, Error, Failure, Outcome, Reason, Script};
use crate::{
    client::{self, Client},
    command::Command,
    server::protocol::{Cursor, Result as Reply},
    session::protocol::Diagnostic,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, Ordering},
};

struct Execution<'a> {
    script: &'a Script,
    next: usize,
    cursor: Cursor,
    pending: BTreeMap<u64, usize>,
    seen: BTreeSet<u64>,
    completed: Vec<Completion>,
    failures: Vec<Failure>,
}

pub(super) fn run(
    client: &mut Client,
    script: &Script,
    cancel: &AtomicBool,
) -> Result<Outcome, Error> {
    if script.commands().is_empty() {
        return Ok(Outcome::Passed { completed: vec![] });
    }
    let snapshot = client.snapshot().map_err(|e| Error::Client {
        message: e.to_string(),
    })?;
    let mut execution = Execution {
        script,
        next: 0,
        cursor: snapshot.cursor,
        pending: BTreeMap::new(),
        seen: BTreeSet::new(),
        completed: vec![],
        failures: vec![],
    };
    if let Err(message) = execution.execute(client, cancel) {
        execution.abort(&message);
    }
    execution.completed.sort_by_key(|c| c.command_index);
    execution.failures.sort_by_key(|f| f.command_index);
    Ok(if execution.failures.is_empty() {
        Outcome::Passed {
            completed: execution.completed,
        }
    } else {
        Outcome::Failed {
            completed: execution.completed,
            failures: execution.failures,
        }
    })
}

impl Execution<'_> {
    fn execute(&mut self, client: &mut Client, cancel: &AtomicBool) -> Result<(), String> {
        while self.next < self.script.commands().len() {
            let command = &self.script.commands()[self.next];
            if matches!(
                command,
                Command::RecordingStart(_) | Command::RecordingStop(_) | Command::Shutdown(_)
            ) {
                self.drain(client, cancel)?;
            }
            check(cancel)?;
            let index = self.next;
            match client.submit(command.clone()) {
                Ok(Reply::Pending { request_id, .. }) => {
                    self.next += 1;
                    if !self.seen.insert(request_id) {
                        self.fail(
                            index,
                            Some(request_id),
                            Reason::Unknown {
                                message: "server reused a request ID".into(),
                            },
                        );
                        return Err("server reused a request ID".into());
                    }
                    self.pending.insert(request_id, index);
                }
                Ok(Reply::Error { code, message }) => {
                    self.next += 1;
                    self.fail(index, None, Reason::Rejected { code, message });
                }
                Err(error) => {
                    self.next += 1;
                    let message = error.to_string();
                    let reason = if matches!(error, client::Error::SubmissionUnknown(_)) {
                        Reason::Unknown {
                            message: message.clone(),
                        }
                    } else {
                        Reason::NotSubmitted {
                            message: message.clone(),
                        }
                    };
                    self.fail(index, None, reason);
                    return Err(message);
                }
                _ => unreachable!("Client validates submission responses"),
            }
            // Consume available results to avoid accumulating Activity during a
            // long script, but do not wait for ordinary commands to finish.
            self.poll(client, 0)?;
        }
        self.drain(client, cancel)
    }
    fn drain(&mut self, client: &mut Client, cancel: &AtomicBool) -> Result<(), String> {
        while !self.pending.is_empty() {
            check(cancel)?;
            self.poll(client, 1000)?;
        }
        Ok(())
    }
    fn poll(&mut self, client: &mut Client, wait_ms: u64) -> Result<(), String> {
        let reply = client
            .poll(Some(self.cursor.clone()), wait_ms)
            .map_err(|e| e.to_string())?;
        let (entries, cursor) = match reply {
            Reply::Activity { entries, cursor } => (entries, cursor),
            Reply::Gap { message, .. } => {
                return Err(format!(
                    "Activity gap: {message}; missing outcomes are unknown"
                ));
            }
            Reply::Error { code, message } => return Err(format!("{code}: {message}")),
            _ => unreachable!("Client validates poll responses"),
        };
        if cursor.session_id != self.cursor.session_id || cursor.position < self.cursor.position {
            return Err("invalid Activity continuation cursor".into());
        }
        let mut position = self.cursor.position;
        let mut ended = false;
        for entry in entries {
            if entry.cursor.session_id != cursor.session_id
                || entry.cursor.position <= position
                || entry.cursor.position > cursor.position
            {
                return Err("invalid Activity entry cursor".into());
            }
            position = entry.cursor.position;
            let event = entry.event;
            ended |= event["kind"] == "lifecycle"
                && matches!(event["state"].as_str(), Some("Ended" | "Failed"));
            let Some(id) = event["request_id"].as_u64() else {
                continue;
            };
            let Some(&index) = self.pending.get(&id) else {
                continue;
            };
            let command = &self.script.commands()[index];
            if event["command"].as_str() != Some(command.name()) {
                return Err(format!("uncorrelated command for request {id}"));
            }
            if event["kind"] == "pending" {
                continue;
            }
            let terminal: Terminal = serde_json::from_value(event)
                .map_err(|e| format!("invalid outcome for request {id}: {e}"))?;
            match terminal {
                Terminal::Completed { output } => {
                    if !command.validate_output(&output) {
                        return Err(format!("invalid output for request {id}"));
                    }
                    self.completed.push(Completion {
                        command_index: index,
                        request_id: id,
                        command: command.clone(),
                        output,
                    });
                }
                Terminal::Rejected { error } => self.fail(
                    index,
                    Some(id),
                    Reason::Rejected {
                        code: error.code,
                        message: error.message,
                    },
                ),
                Terminal::Failed { error } => self.fail(
                    index,
                    Some(id),
                    Reason::Failed {
                        code: error.code,
                        message: error.message,
                    },
                ),
            }
            self.pending.remove(&id);
        }
        self.cursor = cursor;
        if ended && !self.pending.is_empty() {
            return Err("session ended with unanswered script commands".into());
        }
        Ok(())
    }
    fn fail(&mut self, index: usize, request_id: Option<u64>, reason: Reason) {
        self.failures.push(Failure {
            command_index: index,
            request_id,
            command: self.script.commands()[index].clone(),
            reason,
        });
    }
    fn abort(&mut self, message: &str) {
        for (id, index) in std::mem::take(&mut self.pending) {
            self.fail(
                index,
                Some(id),
                Reason::Unknown {
                    message: message.into(),
                },
            );
        }
        for index in self.next..self.script.commands().len() {
            self.fail(
                index,
                None,
                Reason::NotSubmitted {
                    message: message.into(),
                },
            );
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Terminal {
    Completed { output: Value },
    Rejected { error: Diagnostic },
    Failed { error: Diagnostic },
}
fn check(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        Err("script client interrupted; accepted work continues".into())
    } else {
        Ok(())
    }
}
