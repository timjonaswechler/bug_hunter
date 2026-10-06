//! The versioned file schema stops here. Execution receives only a validated command plan.
use super::super::{Error, recording::file};
use crate::command::{Command, tick::warp};
use cap_std::fs::Dir;
use serde::{Deserialize, de};
use serde_json::Value;
use std::{
    collections::VecDeque,
    fmt,
    io::{BufRead, BufReader},
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) type Plan = VecDeque<Command>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    r#type: String,
    format_version: u64,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Line {
    Command {
        command: String,
        arguments: Value,
        outcome: RecordedOutcome,
    },
    RecordingEnded {
        outcome: End,
        recorded_commands: u64,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum End {
    Stopped,
    SessionEnded,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum RecordedOutcome {
    Completed {
        output: Value,
    },
    Rejected {
        error: super::super::protocol::Diagnostic,
    },
    ProtocolFailed {
        error: super::super::protocol::Diagnostic,
    },
    IoFailed {
        error: IoDiagnostic,
    },
    Unanswered {},
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IoDiagnostic {
    message: String,
}

pub(super) fn load(root: &Dir, path: &str, cancel: &AtomicBool) -> Result<Option<Plan>, Error> {
    if cancel.load(Ordering::Acquire) {
        return Ok(None);
    }
    parse(BufReader::new(file::open_read(root, path)?), path, cancel)
}

fn invalid(path: &str, line: usize, message: impl fmt::Display) -> Error {
    Error::new(
        "invalid_recording",
        format!("{path}: line {line}: {message}"),
    )
}

pub(super) fn parse(
    mut reader: impl BufRead,
    path: &str,
    cancel: &AtomicBool,
) -> Result<Option<Plan>, Error> {
    let mut text = String::new();
    let mut line_number = 0;
    let mut count = 0u64;
    let mut ended = false;
    let mut unanswered = false;
    let mut stopped_warp = false;
    let mut plan = Plan::new();
    loop {
        if cancel.load(Ordering::Acquire) {
            return Ok(None);
        }
        text.clear();
        line_number += 1;
        let size = reader
            .read_line(&mut text)
            .map_err(|e| Error::io(format!("{path}: line {line_number}: {e}")))?;
        if size == 0 {
            break;
        }
        // Value normally discards duplicate object keys, including inside arbitrary Inspect
        // outputs. Check recursively before any typed deserialization can discard evidence.
        let value: Unique =
            serde_json::from_str(&text).map_err(|e| invalid(path, line_number, e))?;
        if !value.0.is_object() {
            return Err(invalid(path, line_number, "expected an object"));
        }
        if line_number == 1 {
            let header: Header =
                serde_json::from_str(&text).map_err(|e| invalid(path, line_number, e))?;
            if header.r#type != "recording_started" {
                return Err(invalid(path, line_number, "expected recording_started"));
            }
            if header.format_version != 1 {
                return Err(Error::new(
                    "unsupported_recording_version",
                    format!("{path}: line 1: version {}", header.format_version),
                ));
            }
            continue;
        }
        if ended {
            return Err(invalid(path, line_number, "data after footer"));
        }
        if value.0["outcome"]["status"] == "completed" && value.0["outcome"].get("output").is_none()
        {
            return Err(invalid(path, line_number, "missing output"));
        }
        let line: Line = serde_json::from_str(&text).map_err(|e| invalid(path, line_number, e))?;
        match line {
            Line::RecordingEnded {
                outcome,
                recorded_commands,
            } => {
                if recorded_commands != count {
                    return Err(invalid(
                        path,
                        line_number,
                        "recorded_commands does not match",
                    ));
                }
                if unanswered && !matches!(outcome, End::SessionEnded) {
                    return Err(invalid(
                        path,
                        line_number,
                        "unanswered requires session_ended",
                    ));
                }
                ended = true;
            }
            Line::Command {
                command,
                arguments,
                outcome,
            } => {
                let mut command: Command = serde_json::from_value(
                    serde_json::json!({"command": command, "arguments": arguments}),
                )
                .map_err(|e| invalid(path, line_number, e))?;
                if !command.is_recordable() {
                    return Err(invalid(path, line_number, "command is not recordable"));
                }
                count = count
                    .checked_add(1)
                    .ok_or_else(|| invalid(path, line_number, "command count overflow"))?;
                let uncertain_stop = stopped_warp
                    && matches!(command, Command::Stop(_))
                    && matches!(
                        &outcome,
                        RecordedOutcome::ProtocolFailed { .. }
                            | RecordedOutcome::IoFailed { .. }
                            | RecordedOutcome::Unanswered {}
                    );
                match outcome {
                    RecordedOutcome::Completed { output } => {
                        if !command.validate_output(&output) {
                            return Err(invalid(
                                path,
                                line_number,
                                "output does not match command",
                            ));
                        }
                        match &mut command {
                            Command::Start(start) => {
                                let completion: warp::Completion =
                                    serde_json::from_value(output).unwrap();
                                stopped_warp = completion.outcome == warp::Outcome::Stopped;
                                start.ticks = completion.executed_ticks;
                                if start.ticks == 0 {
                                    continue;
                                }
                            }
                            Command::Stop(_) if stopped_warp => {
                                let stop: warp::Stopped = serde_json::from_value(output).unwrap();
                                if stop.was_running {
                                    stopped_warp = false;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    RecordedOutcome::Unanswered {} => unanswered = true,
                    // Diagnostics describe the old run, not expectations for the new one.
                    RecordedOutcome::Rejected { error }
                    | RecordedOutcome::ProtocolFailed { error } => {
                        let _ = error;
                    }
                    RecordedOutcome::IoFailed { error } => {
                        let _ = error.message;
                    }
                }
                // The Warp's successful completion establishes the effective tick count even
                // if the corresponding Stop response was lost or malformed.
                if uncertain_stop {
                    stopped_warp = false;
                    continue;
                }
                plan.push_back(command);
            }
        }
    }
    if !ended {
        return Err(invalid(path, line_number, "missing complete footer"));
    }
    Ok(Some(plan))
}

struct Unique(Value);

impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Unique, A::Error> {
                let mut values = Vec::new();
                while let Some(Unique(value)) = seq.next_element()? {
                    values.push(value);
                }
                Ok(Unique(Value::Array(values)))
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Unique, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some((key, Unique(value))) = map.next_entry::<String, Unique>()? {
                    if values.insert(key.clone(), value).is_some() {
                        return Err(de::Error::custom(format!("duplicate field {key}")));
                    }
                }
                Ok(Unique(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}
