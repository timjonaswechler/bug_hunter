//! Versioned host-side Session Recording JSONL format.
//!
//! [`FORMAT_VERSION`] versions this persisted schema independently of [`crate::PROTOCOL_VERSION`].
//! Every valid recording begins with [`Event::SessionStarted`], uses strictly increasing host
//! sequences, and ends with [`Event::RecordingStopped`] or [`Event::SessionEnded`]. Host sequences
//! order recording entries; `request_sequence` fields correlate with protocol requests.
//!
//! Writers used by [`crate::driver::Session`] bound, sanitize, and redact recorded values before
//! persistence. Directly serializing the public data types does not apply those protections, and
//! public [`Recording::entries`] can be mutated into an invalid value; call [`Recording::validate`]
//! before consuming or publishing manually constructed recordings.

use crate::{Command, PROTOCOL_VERSION, ProtocolError, ResponseStatus, RunMode, observation};
use cap_std::{ambient_authority, fs::Dir};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fmt,
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// Current Session Recording schema version, independent of the wire protocol version.
pub const FORMAT_VERSION: u32 = 1;
const MAX_DEPTH: usize = 8;
const MAX_COLLECTION_ITEMS: usize = 128;
const MAX_STRING_BYTES: usize = 4 * 1024;
const MAX_ENTRY_BYTES: usize = 256 * 1024;
const REDACTED: &str = "[redacted]";

static AUTOMATIC_PATH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// One line in a Session Recording.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Entry {
    /// Persisted schema version, normally [`FORMAT_VERSION`].
    pub version: u32,
    /// Strictly increasing host recording sequence, independent of wire sequences.
    pub sequence: u64,
    /// Flattened typed event payload.
    #[serde(flatten)]
    pub event: Event,
}

/// Typed events persisted in host order.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    /// Required first event describing the Controlled Session.
    SessionStarted {
        /// Immutable context for this recording segment.
        context: SessionContext,
    },
    /// Source-neutral Controller action.
    ControllerAction {
        /// Controller origin metadata.
        controller: Controller,
        /// Opaque action JSON after Session writer sanitization.
        action: Value,
    },
    /// Non-observation protocol response.
    GameResponse {
        /// Correlated wire request sequence.
        request_sequence: u64,
        /// Response status.
        status: ResponseStatus,
        /// Optional command result.
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        /// Optional protocol failure.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<ProtocolError>,
    },
    /// Read-only observation and its result.
    Observation {
        /// Correlated wire request sequence.
        request_sequence: u64,
        /// Observation request sent to the child.
        request: observation::Request,
        /// Observation result after Session writer sanitization.
        result: Value,
    },
    /// Host or child failure summary.
    Error {
        /// Machine-readable failure kind.
        kind: String,
        /// Bounded human-readable message.
        message: String,
    },
    /// Artifact metadata extracted from a protocol result.
    Artifact {
        /// Correlated wire request sequence.
        request_sequence: u64,
        /// Child-reported artifact reference.
        artifact: ArtifactReference,
    },
    /// Terminal event for a deliberately stopped recording segment.
    RecordingStopped,
    /// Terminal event for the Controlled Session lifecycle.
    SessionEnded {
        /// Clean or aborted session outcome.
        outcome: SessionOutcome,
    },
}

/// Immutable context written in the first event of a recording segment.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionContext {
    /// Host-chosen identity for one Controlled Session.
    pub session_id: String,
    /// Expected or negotiated session mode.
    pub mode: RunMode,
    /// Wire protocol version used by the session.
    pub protocol_version: u32,
    /// Opaque host-provided configuration snapshot.
    pub configuration: Value,
}

impl SessionContext {
    /// Creates context using the crate's current wire protocol version.
    pub fn new(session_id: impl Into<String>, mode: RunMode, configuration: Value) -> Self {
        Self {
            session_id: session_id.into(),
            mode,
            protocol_version: PROTOCOL_VERSION,
            configuration,
        }
    }
}

/// Internal-default recording metadata exposed through [`crate::driver::SessionOptions`].
///
/// Its fields are intentionally not directly configurable; use
/// [`crate::driver::SessionOptions::with_recording_context`] and
/// [`crate::driver::SessionOptions::with_controller`].
#[derive(Clone, Debug)]
pub struct Options {
    pub(crate) context: SessionContext,
    pub(crate) context_explicit: bool,
    pub(crate) controller: Controller,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            context: SessionContext::new(
                "session",
                RunMode::Logical,
                Value::Object(Default::default()),
            ),
            context_explicit: false,
            controller: Controller::new("controller"),
        }
    }
}

/// Provenance for the source directing a Controlled Session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Controller {
    /// Source-neutral origin such as a REPL, agent, Session Script, or Session Replay.
    pub origin: String,
}

impl Controller {
    /// Creates Controller provenance from an origin label.
    pub fn new(origin: impl Into<String>) -> Self {
        Self {
            origin: origin.into(),
        }
    }
}

/// Artifact metadata reported by a child Controlled Session.
///
/// `path` is normally relative to the child/session artifact root; it is not a host recording path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReference {
    /// Artifact kind, for example `screenshot`.
    pub kind: String,
    /// Child-reported path.
    pub path: String,
    /// Media type reported by the child.
    pub mime_type: String,
    /// Optional pixel width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Optional pixel height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

impl ArtifactReference {
    pub(crate) fn from_result(result: &Value) -> Option<Self> {
        let artifact = result.get("artifact")?;
        Some(Self {
            kind: artifact.get("type")?.as_str()?.to_owned(),
            path: artifact.get("path")?.as_str()?.to_owned(),
            mime_type: artifact.get("mime_type")?.as_str()?.to_owned(),
            width: artifact
                .get("width")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
            height: artifact
                .get("height")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
        })
    }
}

/// Terminal outcome of a recorded Controlled Session.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcome {
    /// Clean shutdown completed successfully.
    Completed,
    /// The session ended through failure or drop.
    Aborted,
}

/// Parsed Session Recording.
#[derive(Clone, Debug, PartialEq)]
pub struct Recording {
    /// Public event lines; direct mutation can violate recording invariants.
    pub entries: Vec<Entry>,
}

impl Recording {
    /// Parses strict JSONL and validates lifecycle and sequence invariants.
    ///
    /// Empty lines, unknown event fields, unsupported versions, and malformed event shapes are
    /// rejected rather than ignored.
    pub fn parse_reader(reader: impl BufRead) -> Result<Self, Error> {
        let mut entries = Vec::new();
        for (index, line) in reader.lines().enumerate() {
            let line_number = index + 1;
            let line = line.map_err(|error| Error::Io(error.to_string()))?;
            if line.trim().is_empty() {
                return Err(Error::Invalid(format!("line {line_number} is empty")));
            }
            let value = serde_json::from_str::<Value>(&line)
                .map_err(|error| Error::Json(format!("line {line_number}: {error}")))?;
            let version = value
                .get("version")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    Error::Invalid(format!("line {line_number}: version must be an integer"))
                })?;
            if version != u64::from(FORMAT_VERSION) {
                return Err(Error::UnsupportedVersion(version));
            }
            let entry = serde_json::from_value::<Entry>(value)
                .map_err(|error| Error::Json(format!("line {line_number}: {error}")))?;
            entries.push(entry);
        }
        let recording = Self { entries };
        recording.validate()?;
        Ok(recording)
    }

    /// Opens and parses a Session Recording path.
    pub fn parse_path(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let file = File::open(path)
            .map_err(|error| Error::Io(format!("failed to open {}: {error}", path.display())))?;
        Self::parse_reader(BufReader::new(file))
    }

    /// Validates first/terminal events, versions, increasing host sequences, and event shapes.
    pub fn validate(&self) -> Result<(), Error> {
        let Some(first) = self.entries.first() else {
            return Err(Error::Invalid("recording is empty".into()));
        };
        if !matches!(first.event, Event::SessionStarted { .. }) {
            return Err(Error::Invalid(
                "recording must begin with session_started".into(),
            ));
        }
        let Some(last) = self.entries.last() else {
            unreachable!("the recording has a first entry")
        };
        if !matches!(
            last.event,
            Event::RecordingStopped | Event::SessionEnded { .. }
        ) {
            return Err(Error::Invalid(
                "recording must end with recording_stopped or session_ended".into(),
            ));
        }

        let mut previous = None;
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.version != FORMAT_VERSION {
                return Err(Error::UnsupportedVersion(u64::from(entry.version)));
            }
            if let Some(previous) = previous
                && entry.sequence <= previous
            {
                return Err(Error::Invalid(format!(
                    "host sequence {} is not greater than {previous}",
                    entry.sequence
                )));
            }
            if index > 0 && matches!(entry.event, Event::SessionStarted { .. }) {
                return Err(Error::Invalid(
                    "session_started may only appear as the first entry".into(),
                ));
            }
            if matches!(
                entry.event,
                Event::RecordingStopped | Event::SessionEnded { .. }
            ) && index + 1 != self.entries.len()
            {
                return Err(Error::Invalid(
                    "recording terminal event must be the last entry".into(),
                ));
            }
            validate_event(&entry.event)?;
            previous = Some(entry.sequence);
        }
        Ok(())
    }
}

fn validate_event(event: &Event) -> Result<(), Error> {
    match event {
        Event::SessionStarted { context } => {
            require_nonempty("session_id", &context.session_id)?;
            if context.protocol_version == 0 {
                return Err(Error::Invalid("protocol_version must be positive".into()));
            }
        }
        Event::ControllerAction { controller, .. } => {
            require_nonempty("controller.origin", &controller.origin)?;
        }
        Event::Error { kind, message } => {
            require_nonempty("error.kind", kind)?;
            require_nonempty("error.message", message)?;
        }
        Event::GameResponse {
            status,
            result,
            error,
            ..
        } => match status {
            ResponseStatus::Completed => {
                if result.is_none() {
                    return Err(Error::Invalid(
                        "completed game_response must contain result".into(),
                    ));
                }
                if error.is_some() {
                    return Err(Error::Invalid(
                        "completed game_response must not contain error".into(),
                    ));
                }
            }
            ResponseStatus::Error => {
                if error.is_none() {
                    return Err(Error::Invalid(
                        "error game_response must contain error".into(),
                    ));
                }
            }
        },
        Event::Artifact { artifact, .. } => {
            require_nonempty("artifact.kind", &artifact.kind)?;
            require_nonempty("artifact.path", &artifact.path)?;
            require_nonempty("artifact.mime_type", &artifact.mime_type)?;
        }
        Event::Observation { .. } | Event::RecordingStopped | Event::SessionEnded { .. } => {}
    }
    Ok(())
}

fn require_nonempty(name: &str, value: &str) -> Result<(), Error> {
    if value.trim().is_empty() {
        Err(Error::Invalid(format!("{name} must not be empty")))
    } else {
        Ok(())
    }
}

pub(crate) fn sanitize_event(event: Event) -> Event {
    match event {
        Event::SessionStarted { mut context } => {
            context.session_id = sanitize_message(&context.session_id);
            context.configuration = sanitize_value(&context.configuration, 0);
            Event::SessionStarted { context }
        }
        Event::ControllerAction {
            mut controller,
            action,
        } => {
            controller.origin = sanitize_message(&controller.origin);
            Event::ControllerAction {
                controller,
                action: sanitize_value(&action, 0),
            }
        }
        Event::GameResponse {
            request_sequence,
            status,
            result,
            error,
        } => Event::GameResponse {
            request_sequence,
            status,
            result: result.map(|value| sanitize_value(&value, 0)),
            error: error.map(|error| ProtocolError {
                code: sanitize_text(&error.code),
                message: sanitize_message(&error.message),
            }),
        },
        Event::Observation {
            request_sequence,
            request,
            result,
        } => Event::Observation {
            request_sequence,
            request,
            result: sanitize_value(&result, 0),
        },
        Event::Error { kind, message } => Event::Error {
            kind: sanitize_text(&kind),
            message: sanitize_message(&message),
        },
        Event::Artifact {
            request_sequence,
            mut artifact,
        } => {
            artifact.kind = sanitize_message(&artifact.kind);
            artifact.path = sanitize_message(&artifact.path);
            artifact.mime_type = sanitize_message(&artifact.mime_type);
            Event::Artifact {
                request_sequence,
                artifact,
            }
        }
        Event::RecordingStopped => Event::RecordingStopped,
        Event::SessionEnded { outcome } => Event::SessionEnded { outcome },
    }
}

fn sanitize_value(value: &Value, depth: usize) -> Value {
    if depth >= MAX_DEPTH {
        return serde_json::json!({"recording_truncated": "maximum depth reached"});
    }
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
        Value::String(value) => Value::String(sanitize_message(value)),
        Value::Array(values) => {
            let mut sanitized = values
                .iter()
                .take(MAX_COLLECTION_ITEMS)
                .map(|value| sanitize_value(value, depth + 1))
                .collect::<Vec<_>>();
            if values.len() > MAX_COLLECTION_ITEMS {
                sanitized.push(Value::String(format!(
                    "[truncated {} items]",
                    values.len() - MAX_COLLECTION_ITEMS
                )));
            }
            Value::Array(sanitized)
        }
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            let mut sanitized = serde_json::Map::new();
            for key in keys.iter().take(MAX_COLLECTION_ITEMS) {
                let value = if sensitive_key(key) {
                    Value::String(REDACTED.into())
                } else {
                    sanitize_value(&values[*key], depth + 1)
                };
                sanitized.insert(sanitize_text(key), value);
            }
            if keys.len() > MAX_COLLECTION_ITEMS {
                sanitized.insert(
                    "recording_truncated".into(),
                    Value::String(format!(
                        "{} object fields omitted",
                        keys.len() - MAX_COLLECTION_ITEMS
                    )),
                );
            }
            Value::Object(sanitized)
        }
    }
}

fn sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();
    [
        "secret",
        "token",
        "password",
        "credential",
        "apikey",
        "authorization",
        "authentication",
        "sessioncookie",
        "privatekey",
        "prompt",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
        || matches!(normalized.as_str(), "auth" | "bearer" | "cookie")
}

fn sanitize_message(message: &str) -> String {
    let lowercase = message.to_ascii_lowercase();
    if [
        "secret",
        "token",
        "password",
        "credential",
        "api_key",
        "api key",
        "authorization",
        "bearer ",
        "session cookie",
        "session_cookie",
        "private key",
        "private_key",
        "-----begin ",
        "prompt",
    ]
    .iter()
    .any(|needle| lowercase.contains(needle))
        || contains_opaque_token(message)
    {
        return "[redacted sensitive recording message]".into();
    }
    sanitize_text(message)
}

fn contains_opaque_token(value: &str) -> bool {
    value
        .split(|character: char| {
            character.is_whitespace() || matches!(character, '"' | '\'' | ',' | ';' | ':' | '=')
        })
        .any(|part| {
            let lowercase = part.to_ascii_lowercase();
            lowercase.starts_with("sk-") && part.len() >= 12
        })
}

fn sanitize_text(value: &str) -> String {
    if value.len() <= MAX_STRING_BYTES {
        return value.to_owned();
    }
    let mut end = MAX_STRING_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}[truncated {} bytes]", &value[..end], value.len() - end)
}

fn compact_event(event: &Event, original_bytes: usize) -> Event {
    let marker = serde_json::json!({
        "recording_truncated": {
            "original_bytes": original_bytes,
            "maximum_bytes": MAX_ENTRY_BYTES
        }
    });
    match event {
        Event::SessionStarted { context } => Event::SessionStarted {
            context: SessionContext {
                configuration: marker,
                ..context.clone()
            },
        },
        Event::ControllerAction { controller, .. } => Event::ControllerAction {
            controller: controller.clone(),
            action: marker,
        },
        Event::GameResponse {
            request_sequence,
            status,
            error,
            ..
        } => Event::GameResponse {
            request_sequence: *request_sequence,
            status: *status,
            result: Some(marker),
            error: error.clone(),
        },
        Event::Observation {
            request_sequence,
            request,
            ..
        } => Event::Observation {
            request_sequence: *request_sequence,
            request: request.clone(),
            result: marker,
        },
        _ => event.clone(),
    }
}

/// Recording parse, validation, path, or persistence failure.
#[derive(Debug)]
pub enum Error {
    /// Recording filesystem operation failed.
    Io(String),
    /// Create-new semantics found an existing requested recording.
    AlreadyExists(PathBuf),
    /// JSON serialization or deserialization failed.
    Json(String),
    /// Recording lifecycle or schema invariant failed.
    Invalid(String),
    /// Recording schema version is unsupported.
    UnsupportedVersion(u64),
    /// Requested relative `.jsonl` path is invalid or unsafe for the writer.
    InvalidPath(String),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => formatter.write_str(message),
            Self::AlreadyExists(path) => {
                write!(formatter, "recording {} already exists", path.display())
            }
            Self::Json(message) => write!(formatter, "invalid recording JSON: {message}"),
            Self::Invalid(message) => write!(formatter, "invalid recording: {message}"),
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "unsupported recording version {version}; expected {FORMAT_VERSION}"
            ),
            Self::InvalidPath(message) => write!(formatter, "invalid recording path: {message}"),
        }
    }
}

impl std::error::Error for Error {}

pub(crate) struct State {
    pub(crate) writer: Option<Writer>,
    pub(crate) context: SessionContext,
    pub(crate) context_explicit: bool,
    pub(crate) context_written: bool,
    pub(crate) ready: bool,
    pub(crate) artifact_root: PathBuf,
    pub(crate) controller: Controller,
    pub(crate) host_sequence: u64,
}

#[derive(Debug)]
pub(crate) struct Writer {
    file: cap_std::fs::File,
    path: PathBuf,
}

impl Writer {
    pub(crate) fn create(artifact_root: &Path, requested: Option<&Path>) -> Result<Self, Error> {
        fs::create_dir_all(artifact_root).map_err(|error| {
            Error::Io(format!(
                "failed to create artifact root {}: {error}",
                artifact_root.display()
            ))
        })?;
        reject_symlink(artifact_root)?;
        let canonical_root = fs::canonicalize(artifact_root).map_err(|error| {
            Error::Io(format!(
                "failed to resolve artifact root {}: {error}",
                artifact_root.display()
            ))
        })?;

        if let Some(requested) = requested {
            let relative = validate_relative_path(requested)?;
            return Self::create_relative(&canonical_root, &relative);
        }

        for _ in 0..1000 {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| Error::Io(format!("system clock is before Unix epoch: {error}")))?
                .as_nanos();
            let sequence = AUTOMATIC_PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let relative = PathBuf::from(format!(
                "recordings/session-{nanos}-{}-{sequence}.jsonl",
                std::process::id()
            ));
            match Self::create_relative(&canonical_root, &relative) {
                Ok(writer) => return Ok(writer),
                Err(Error::AlreadyExists(_)) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(Error::Io(
            "could not allocate a collision-free recording path".into(),
        ))
    }

    fn create_relative(canonical_root: &Path, relative: &Path) -> Result<Self, Error> {
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        let mut current = canonical_root.to_path_buf();
        for component in parent.components() {
            let Component::Normal(component) = component else {
                return Err(Error::InvalidPath(
                    "path must contain only normal components".into(),
                ));
            };
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(Error::InvalidPath(format!(
                        "path crosses symbolic link {}",
                        current.display()
                    )));
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err(Error::InvalidPath(format!(
                        "parent {} is not a directory",
                        current.display()
                    )));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&current).map_err(|error| {
                        Error::Io(format!(
                            "failed to create recording directory {}: {error}",
                            current.display()
                        ))
                    })?;
                }
                Err(error) => {
                    return Err(Error::Io(format!(
                        "failed to inspect recording directory {}: {error}",
                        current.display()
                    )));
                }
            }
            let canonical = fs::canonicalize(&current).map_err(|error| {
                Error::Io(format!("failed to resolve {}: {error}", current.display()))
            })?;
            if !canonical.starts_with(canonical_root) {
                return Err(Error::InvalidPath("path leaves the artifact root".into()));
            }
        }

        let target = canonical_root.join(relative);
        if let Ok(metadata) = fs::symlink_metadata(&target)
            && metadata.file_type().is_symlink()
        {
            return Err(Error::InvalidPath(format!(
                "target is a symbolic link: {}",
                target.display()
            )));
        }
        let directory =
            Dir::open_ambient_dir(canonical_root, ambient_authority()).map_err(|error| {
                Error::Io(format!(
                    "failed to open artifact root {}: {error}",
                    canonical_root.display()
                ))
            })?;
        let mut options = cap_std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        let file = directory.open_with(relative, &options).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                Error::AlreadyExists(target.clone())
            } else {
                Error::Io(format!(
                    "failed to create recording {}: {error}",
                    target.display()
                ))
            }
        })?;
        Ok(Self { file, path: target })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn write(&mut self, entry: &Entry) -> Result<(), Error> {
        let mut encoded =
            serde_json::to_vec(entry).map_err(|error| Error::Json(error.to_string()))?;
        if encoded.len() > MAX_ENTRY_BYTES {
            let compact = Entry {
                version: entry.version,
                sequence: entry.sequence,
                event: compact_event(&entry.event, encoded.len()),
            };
            encoded =
                serde_json::to_vec(&compact).map_err(|error| Error::Json(error.to_string()))?;
        }
        if encoded.len() > MAX_ENTRY_BYTES {
            let fallback = Entry {
                version: entry.version,
                sequence: entry.sequence,
                event: Event::Error {
                    kind: "recording_entry_truncated".into(),
                    message: format!("entry exceeded the {MAX_ENTRY_BYTES}-byte recording limit"),
                },
            };
            encoded =
                serde_json::to_vec(&fallback).map_err(|error| Error::Json(error.to_string()))?;
        }
        self.file
            .write_all(&encoded)
            .and_then(|_| self.file.write_all(b"\n"))
            .and_then(|_| self.file.flush())
            .and_then(|_| self.file.sync_data())
            .map_err(|error| Error::Io(format!("failed to flush recording: {error}")))
    }
}

fn validate_relative_path(path: &Path) -> Result<PathBuf, Error> {
    if path.as_os_str().is_empty() {
        return Err(Error::InvalidPath("path must not be empty".into()));
    }
    let wire = path
        .to_str()
        .ok_or_else(|| Error::InvalidPath("path must be valid UTF-8".into()))?;
    if path.is_absolute() || wire.starts_with(['/', '\\']) || wire.as_bytes().get(1) == Some(&b':')
    {
        return Err(Error::InvalidPath("path must be relative".into()));
    }
    if wire.contains('\\') {
        return Err(Error::InvalidPath("path must use forward slashes".into()));
    }
    if wire.split('/').any(|component| component.is_empty()) {
        return Err(Error::InvalidPath(
            "path must not contain empty components".into(),
        ));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::CurDir | Component::ParentDir => {
                return Err(Error::InvalidPath(
                    "path must not contain '.' or '..'".into(),
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(Error::InvalidPath("path must be relative".into()));
            }
        }
    }
    if normalized.extension().and_then(|value| value.to_str()) != Some("jsonl") {
        return Err(Error::InvalidPath(
            "recording path must end in .jsonl".into(),
        ));
    }
    Ok(normalized)
}

/// Lexically validates a relative forward-slash `.jsonl` path and joins it to a root.
///
/// This helper does not create or canonicalize the root, inspect symlinks, or guarantee canonical
/// filesystem containment. The internal Session Recording writer performs stronger checks before
/// creating files.
pub fn path_below_artifact_root(
    artifact_root: impl AsRef<Path>,
    requested: impl AsRef<Path>,
) -> Result<PathBuf, Error> {
    Ok(artifact_root
        .as_ref()
        .join(validate_relative_path(requested.as_ref())?))
}

fn reject_symlink(path: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Error::Io(format!("failed to inspect {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() {
        Err(Error::InvalidPath(format!(
            "artifact root is a symbolic link: {}",
            path.display()
        )))
    } else {
        Ok(())
    }
}

pub(crate) fn command_value(command: &Command) -> Result<Value, Error> {
    serde_json::to_value(command).map_err(|error| Error::Json(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    fn context() -> SessionContext {
        SessionContext::new("alpha", RunMode::Logical, json!({"surface": [640, 360]}))
    }

    #[test]
    fn parses_and_roundtrips_version_one_entries() {
        let source = concat!(
            "{\"version\":1,\"sequence\":4,\"type\":\"session_started\",\"context\":{\"session_id\":\"alpha\",\"mode\":\"logical\",\"protocol_version\":2,\"configuration\":{}}}\n",
            "{\"version\":1,\"sequence\":5,\"type\":\"recording_stopped\"}\n"
        );
        let recording = Recording::parse_reader(Cursor::new(source)).unwrap();
        assert_eq!(recording.entries.len(), 2);
        let encoded = recording
            .entries
            .iter()
            .map(|entry| serde_json::to_string(entry).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        let reparsed = Recording::parse_reader(Cursor::new(format!("{encoded}\n"))).unwrap();
        assert_eq!(recording, reparsed);
    }

    #[test]
    fn rejects_malformed_json_unknown_versions_and_non_increasing_sequences() {
        let malformed = Recording::parse_reader(Cursor::new("not-json\n")).unwrap_err();
        assert!(matches!(malformed, Error::Json(_)));
        let unknown_field = concat!(
            "{\"version\":1,\"sequence\":1,\"type\":\"session_started\",",
            "\"context\":{\"session_id\":\"alpha\",\"mode\":\"logical\",",
            "\"protocol_version\":2,\"configuration\":{}},\"secret\":true}\n"
        );
        assert!(matches!(
            Recording::parse_reader(Cursor::new(unknown_field)),
            Err(Error::Json(_))
        ));

        let unsupported = Entry {
            version: 99,
            sequence: 1,
            event: Event::SessionStarted { context: context() },
        };
        let source = format!("{}\n", serde_json::to_string(&unsupported).unwrap());
        assert!(matches!(
            Recording::parse_reader(Cursor::new(source)),
            Err(Error::UnsupportedVersion(99))
        ));
        assert!(matches!(
            Recording::parse_reader(Cursor::new(
                "{\"version\":99,\"sequence\":1,\"type\":\"future_event\"}\n"
            )),
            Err(Error::UnsupportedVersion(99))
        ));

        let entries = [
            Entry {
                version: FORMAT_VERSION,
                sequence: 8,
                event: Event::SessionStarted { context: context() },
            },
            Entry {
                version: FORMAT_VERSION,
                sequence: 8,
                event: Event::RecordingStopped,
            },
        ];
        let source = entries
            .iter()
            .map(|entry| serde_json::to_string(entry).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(matches!(
            Recording::parse_reader(Cursor::new(format!("{source}\n"))),
            Err(Error::Invalid(message)) if message.contains("not greater")
        ));
    }

    #[test]
    fn validation_requires_a_terminal_event_and_consistent_response_payloads() {
        let started = Entry {
            version: FORMAT_VERSION,
            sequence: 1,
            event: Event::SessionStarted { context: context() },
        };
        let incomplete = Recording {
            entries: vec![started.clone()],
        };
        assert!(matches!(
            incomplete.validate(),
            Err(Error::Invalid(message)) if message.contains("must end")
        ));

        for event in [
            Event::GameResponse {
                request_sequence: 1,
                status: ResponseStatus::Completed,
                result: None,
                error: None,
            },
            Event::GameResponse {
                request_sequence: 1,
                status: ResponseStatus::Error,
                result: Some(json!({"partial": true})),
                error: None,
            },
        ] {
            let recording = Recording {
                entries: vec![
                    started.clone(),
                    Entry {
                        version: FORMAT_VERSION,
                        sequence: 2,
                        event,
                    },
                    Entry {
                        version: FORMAT_VERSION,
                        sequence: 3,
                        event: Event::RecordingStopped,
                    },
                ],
            };
            assert!(matches!(recording.validate(), Err(Error::Invalid(_))));
        }
    }

    #[test]
    fn sanitization_redacts_neutral_credential_carriers_and_opaque_tokens() {
        let sanitized = sanitize_event(Event::ControllerAction {
            controller: Controller::new("repl"),
            action: json!({
                "auth": "opaque-auth-value",
                "session_cookie": "opaque-cookie-value",
                "private_key": "opaque-key-value",
                "note": "use sk-live-1234567890 for this request",
                "text": "ordinary controller text remains"
            }),
        });
        let encoded = serde_json::to_string(&sanitized).unwrap();
        for secret in [
            "opaque-auth-value",
            "opaque-cookie-value",
            "opaque-key-value",
            "sk-live-1234567890",
        ] {
            assert!(!encoded.contains(secret));
        }
        assert!(encoded.contains("ordinary controller text remains"));
    }

    #[test]
    fn creates_recordings_only_below_the_artifact_root_without_overwriting() {
        let root = std::env::temp_dir().join(format!(
            "automation-control-recording-path-{}-{}",
            std::process::id(),
            AUTOMATIC_PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let first = Writer::create(&root, Some(Path::new("records/session.jsonl"))).unwrap();
        assert!(first.path().starts_with(fs::canonicalize(&root).unwrap()));
        drop(first);
        assert!(Writer::create(&root, Some(Path::new("records/session.jsonl"))).is_err());
        assert!(Writer::create(&root, Some(Path::new("../outside.jsonl"))).is_err());
        assert!(Writer::create(&root, Some(Path::new("/outside.jsonl"))).is_err());
        let automatic = Writer::create(&root, None).unwrap();
        let next_automatic = Writer::create(&root, None).unwrap();
        assert!(
            automatic
                .path()
                .starts_with(fs::canonicalize(&root).unwrap())
        );
        assert_ne!(automatic.path(), next_automatic.path());
        fs::remove_dir_all(root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symbolic_link_below_the_artifact_root() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "automation-control-recording-symlink-{}-{}",
            std::process::id(),
            AUTOMATIC_PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let outside = std::env::temp_dir().join(format!(
            "automation-control-recording-outside-{}-{}",
            std::process::id(),
            AUTOMATIC_PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("linked")).unwrap();
        let error = Writer::create(&root, Some(Path::new("linked/session.jsonl"))).unwrap_err();
        assert!(matches!(error, Error::InvalidPath(message) if message.contains("symbolic link")));
        fs::remove_dir_all(root).ok();
        fs::remove_dir_all(outside).ok();
    }
}
