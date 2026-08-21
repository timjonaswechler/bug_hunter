use super::{
    Config,
    controller::{ControllerError, ControllerSession, Mode},
};
use crate::{
    Command, PROTOCOL_VERSION, Response, ResponseStatus, RunMode,
    host::{
        RecentLogs,
        recording::{
            self, ArtifactReference, Controller, Event, Recording, SessionContext, SessionOutcome,
        },
    },
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) const REPLAY_EXIT: i32 = 6;
const RESULT_VERSION: u32 = 1;
const RESULT_NAME: &str = "replay-result.json";
static REPLAY_ARTIFACT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Passed,
    Failed,
}

#[derive(Debug, Serialize)]
struct ResultArtifact {
    version: u32,
    source: PathBuf,
    controller: &'static str,
    configuration: Value,
    artifact_root: PathBuf,
    outcome: Outcome,
    actions: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    artifacts: Vec<ArtifactReference>,
}

#[derive(Debug)]
pub(crate) struct Error {
    message: String,
    sequence: Option<u64>,
    action: Option<Value>,
    expected: Option<Value>,
    actual: Option<Value>,
    artifacts: Vec<ArtifactReference>,
}

impl Error {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            sequence: None,
            action: None,
            expected: None,
            actual: None,
            artifacts: Vec::new(),
        }
    }

    fn mismatch(
        sequence: u64,
        action: &Value,
        message: impl Into<String>,
        expected: Value,
        actual: Value,
        artifacts: Vec<ArtifactReference>,
    ) -> Self {
        Self {
            message: message.into(),
            sequence: Some(sequence),
            action: Some(action.clone()),
            expected: Some(expected),
            actual: Some(actual),
            artifacts,
        }
    }

    pub(crate) const fn exit_code(&self) -> i32 {
        REPLAY_EXIT
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)?;
        if let Some(sequence) = self.sequence {
            write!(formatter, "\nrecording sequence: {sequence}")?;
        }
        if let Some(action) = &self.action {
            write!(formatter, "\naction: {action}")?;
        }
        if let Some(expected) = &self.expected {
            write!(formatter, "\nexpected: {expected}")?;
        }
        if let Some(actual) = &self.actual {
            write!(formatter, "\nactual: {actual}")?;
        }
        for artifact in &self.artifacts {
            write!(
                formatter,
                "\nartifact: {} ({})",
                artifact.path, artifact.mime_type
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub(crate) struct Summary {
    pub(crate) actions: usize,
    pub(crate) mode: Mode,
}

struct RecordedAction<'a> {
    sequence: u64,
    action: &'a Value,
    expected: &'a [recording::Entry],
}

pub(crate) fn run(
    profile: &Config,
    source: &Path,
    artifact_dir: PathBuf,
    record: Option<PathBuf>,
    recent_logs: RecentLogs,
) -> Result<Summary, Error> {
    let session_artifact_dir = artifact_dir.join("replay-artifacts").join(format!(
        "session-{}-{}",
        std::process::id(),
        REPLAY_ARTIFACT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let recording = match Recording::parse_path(source) {
        Ok(recording) => recording,
        Err(error) => {
            return Err(persist_early_failure(
                source,
                &artifact_dir,
                &session_artifact_dir,
                &Value::Null,
                0,
                Error::invalid(format!("could not load Session Recording: {error}")),
            ));
        }
    };
    let context = match context(&recording) {
        Ok(context) => context,
        Err(error) => {
            return Err(persist_early_failure(
                source,
                &artifact_dir,
                &session_artifact_dir,
                &Value::Null,
                0,
                error,
            ));
        }
    };
    let mode = match validate_context(profile, context) {
        Ok(mode) => mode,
        Err(error) => {
            return Err(persist_early_failure(
                source,
                &artifact_dir,
                &session_artifact_dir,
                &context.configuration,
                0,
                error,
            ));
        }
    };
    if let Err(error) = validate_terminal(&recording) {
        return Err(persist_early_failure(
            source,
            &artifact_dir,
            &session_artifact_dir,
            &context.configuration,
            0,
            error,
        ));
    }
    let actions = actions(&recording);
    if actions.is_empty() {
        return Err(persist_early_failure(
            source,
            &artifact_dir,
            &session_artifact_dir,
            &context.configuration,
            0,
            Error::invalid("Session Recording contains no Controller actions"),
        ));
    }
    if let Err(error) = validate_actions(&actions) {
        return Err(persist_early_failure(
            source,
            &artifact_dir,
            &session_artifact_dir,
            &context.configuration,
            actions.len(),
            error,
        ));
    }

    let records_replay = record.is_some();
    let mut session = match ControllerSession::start_replay(
        profile,
        mode,
        artifact_dir.clone(),
        record,
        recent_logs,
        Controller::new("replay"),
        context.configuration.clone(),
        session_artifact_dir.clone(),
    ) {
        Ok(session) => session,
        Err(error) => {
            return Err(persist_early_failure(
                source,
                &artifact_dir,
                &session_artifact_dir,
                &context.configuration,
                actions.len(),
                Error::invalid(error.to_string()),
            ));
        }
    };

    let mut result = execute(&mut session, &actions);
    let source_shuts_down = actions
        .last()
        .is_some_and(|entry| entry.action["type"] == "shutdown");
    if records_replay && !source_shuts_down {
        result = match result {
            Ok(artifacts) => session
                .stop_recording()
                .map(|_| artifacts)
                .map_err(|error| Error::invalid(error.to_string())),
            Err(error) => Err(error),
        };
    }
    drop(session);

    let (outcome, error, artifacts) = match &result {
        Ok(artifacts) => (Outcome::Passed, None, artifacts.clone()),
        Err(error) => (
            Outcome::Failed,
            Some(error.to_string()),
            error.artifacts.clone(),
        ),
    };
    let artifact = result_artifact(
        source,
        &context.configuration,
        session_artifact_dir,
        actions.len(),
        outcome,
        error,
        artifacts,
    );
    write_result(&artifact_dir, &artifact).map_err(Error::invalid)?;
    result.map(|_| Summary {
        actions: actions.len(),
        mode,
    })
}

fn persist_early_failure(
    source: &Path,
    artifact_dir: &Path,
    session_artifact_dir: &Path,
    configuration: &Value,
    actions: usize,
    error: Error,
) -> Error {
    let artifact = result_artifact(
        source,
        configuration,
        session_artifact_dir.to_path_buf(),
        actions,
        Outcome::Failed,
        Some(error.to_string()),
        error.artifacts.clone(),
    );
    match write_result(artifact_dir, &artifact) {
        Ok(()) => error,
        Err(write_error) => Error::invalid(format!("{error}; {write_error}")),
    }
}

fn result_artifact(
    source: &Path,
    configuration: &Value,
    artifact_root: PathBuf,
    actions: usize,
    outcome: Outcome,
    error: Option<String>,
    artifacts: Vec<ArtifactReference>,
) -> ResultArtifact {
    ResultArtifact {
        version: RESULT_VERSION,
        source: source.to_path_buf(),
        controller: "replay",
        configuration: configuration.clone(),
        artifact_root,
        outcome,
        actions,
        error,
        artifacts,
    }
}

fn context(recording: &Recording) -> Result<&SessionContext, Error> {
    match &recording.entries[0].event {
        Event::SessionStarted { context } => Ok(context),
        _ => Err(Error::invalid("Session Recording has no session context")),
    }
}

fn validate_context(profile: &Config, context: &SessionContext) -> Result<Mode, Error> {
    if context.protocol_version != PROTOCOL_VERSION {
        return Err(Error::invalid(format!(
            "incompatible protocol version {}; expected {PROTOCOL_VERSION}",
            context.protocol_version
        )));
    }
    if let Some(recorded_profile) = context.configuration.get("profile_id") {
        let recorded_profile = recorded_profile
            .as_str()
            .ok_or_else(|| Error::invalid("recorded profile_id must be a string"))?;
        if recorded_profile != profile.profile_id {
            return Err(Error::invalid(format!(
                "recorded profile {recorded_profile:?} conflicts with configured profile {:?}",
                profile.profile_id
            )));
        }
    }
    let configured_mode = context.configuration.get("mode").and_then(Value::as_str);
    let expected_mode = match context.mode {
        RunMode::Logical => ("logical", Mode::Logical),
        RunMode::Rendered => ("rendered", Mode::Rendered),
    };
    if configured_mode != Some(expected_mode.0) {
        return Err(Error::invalid(format!(
            "recorded session configuration mode does not match {:?}",
            context.mode
        )));
    }
    let width = context
        .configuration
        .pointer("/surface/width")
        .and_then(Value::as_f64);
    let height = context
        .configuration
        .pointer("/surface/height")
        .and_then(Value::as_f64);
    if width != Some(f64::from(profile.session.surface_width))
        || height != Some(f64::from(profile.session.surface_height))
    {
        return Err(Error::invalid(format!(
            "recorded surface is incompatible; expected {}x{}",
            profile.session.surface_width, profile.session.surface_height
        )));
    }
    Ok(expected_mode.1)
}

fn validate_terminal(recording: &Recording) -> Result<(), Error> {
    match &recording
        .entries
        .last()
        .expect("validated recording is nonempty")
        .event
    {
        Event::SessionEnded {
            outcome: SessionOutcome::Aborted,
        } => Err(Error::invalid(
            "cannot replay an incomplete aborted session",
        )),
        Event::RecordingStopped
        | Event::SessionEnded {
            outcome: SessionOutcome::Completed,
        } => Ok(()),
        _ => Err(Error::invalid(
            "Session Recording has no completed terminal event",
        )),
    }
}

fn actions(recording: &Recording) -> Vec<RecordedAction<'_>> {
    let mut result = Vec::new();
    for (index, entry) in recording.entries.iter().enumerate() {
        let Event::ControllerAction { action, .. } = &entry.event else {
            continue;
        };
        let end = recording.entries[index + 1..]
            .iter()
            .position(|next| matches!(next.event, Event::ControllerAction { .. }))
            .map_or(recording.entries.len(), |offset| index + 1 + offset);
        result.push(RecordedAction {
            sequence: entry.sequence,
            action,
            expected: &recording.entries[index + 1..end],
        });
    }
    result
}

fn validate_actions(actions: &[RecordedAction<'_>]) -> Result<(), Error> {
    let mut previous_request_sequence = None;
    for recorded in actions {
        if contains_truncation_marker(recorded.action) {
            return Err(Error::invalid(format!(
                "Controller action at recording sequence {} was truncated and cannot be replayed",
                recorded.sequence
            )));
        }
        let host_action = matches!(
            recorded.action.get("type").and_then(Value::as_str),
            Some("pause" | "resume")
        );
        let command = if host_action {
            None
        } else {
            Some(
                serde_json::from_value::<Command>(recorded.action.clone()).map_err(|error| {
                    Error::invalid(format!(
                        "invalid Controller action at recording sequence {}: {error}",
                        recorded.sequence
                    ))
                })?,
            )
        };
        let outcomes = recorded
            .expected
            .iter()
            .filter_map(|entry| match &entry.event {
                Event::Observation {
                    request_sequence,
                    request,
                    ..
                } => Some((*request_sequence, Some(request))),
                Event::GameResponse {
                    request_sequence, ..
                } => Some((*request_sequence, None)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if host_action && !outcomes.is_empty() {
            return Err(Error::invalid(format!(
                "host action at recording sequence {} has an unexpected game response",
                recorded.sequence
            )));
        }
        if command.is_some() && outcomes.len() != 1 {
            return Err(Error::invalid(format!(
                "Controller action at recording sequence {} must have exactly one recorded outcome",
                recorded.sequence
            )));
        }
        let Some((request_sequence, observation)) = outcomes.first() else {
            continue;
        };
        if previous_request_sequence.is_some_and(|previous| *request_sequence != previous + 1) {
            return Err(Error::invalid(format!(
                "request sequence {request_sequence} after recording sequence {} is out of order",
                recorded.sequence
            )));
        }
        previous_request_sequence = Some(*request_sequence);
        if let (Some(Command::Observe(actual)), Some(expected)) = (command.as_ref(), *observation)
            && actual != expected
        {
            return Err(Error::invalid(format!(
                "observation request after recording sequence {} does not match its Controller action",
                recorded.sequence
            )));
        }
        let response_error = recorded
            .expected
            .iter()
            .find_map(|entry| match &entry.event {
                Event::GameResponse {
                    error: Some(error), ..
                } => Some(error.code.as_str()),
                _ => None,
            });
        for entry in recorded.expected {
            match &entry.event {
                Event::Artifact {
                    request_sequence: artifact_sequence,
                    ..
                } if artifact_sequence != request_sequence => {
                    return Err(Error::invalid(format!(
                        "artifact after recording sequence {} refers to request {artifact_sequence}, expected {request_sequence}",
                        recorded.sequence
                    )));
                }
                Event::Error { kind, .. } if Some(kind.as_str()) != response_error => {
                    return Err(Error::invalid(format!(
                        "host error {kind:?} after recording sequence {} has no replayable game response",
                        recorded.sequence
                    )));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn contains_truncation_marker(value: &Value) -> bool {
    match value {
        Value::String(value) => value.contains("[truncated "),
        Value::Array(values) => values.iter().any(contains_truncation_marker),
        Value::Object(values) => {
            values.contains_key("recording_truncated")
                || values.values().any(contains_truncation_marker)
        }
        _ => false,
    }
}

fn execute(
    session: &mut ControllerSession,
    actions: &[RecordedAction<'_>],
) -> Result<Vec<ArtifactReference>, Error> {
    let mut artifacts = Vec::new();
    for recorded in actions {
        let expected_artifacts = recorded
            .expected
            .iter()
            .filter_map(|entry| match &entry.event {
                Event::Artifact { artifact, .. } => Some(artifact.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let actual = execute_action(session, recorded.action).map_err(|error| {
            Error::mismatch(
                recorded.sequence,
                recorded.action,
                "Replay action failed",
                expected_value(recorded.expected),
                json!({"error": error.to_string()}),
                expected_artifacts.clone(),
            )
        })?;
        compare(recorded, actual.as_ref(), &expected_artifacts)?;
        artifacts.extend(expected_artifacts);
    }
    Ok(artifacts)
}

fn execute_action(
    session: &mut ControllerSession,
    action: &Value,
) -> Result<Option<Response>, ControllerError> {
    match action.get("type").and_then(Value::as_str) {
        Some("pause") => session.pause().map(|()| None),
        Some("resume") => session.resume().map(|()| None),
        Some("shutdown") => session
            .shutdown()
            .map(|()| Some(Response::completed(0, json!({})))),
        _ => {
            let command = serde_json::from_value::<Command>(action.clone()).map_err(|error| {
                ControllerError::Invalid(format!("recorded action is invalid: {error}"))
            })?;
            session.replay_command(command).map(Some)
        }
    }
}

fn compare(
    recorded: &RecordedAction<'_>,
    actual: Option<&Response>,
    artifacts: &[ArtifactReference],
) -> Result<(), Error> {
    let expected_response = recorded
        .expected
        .iter()
        .find_map(|entry| match &entry.event {
            Event::Observation { result, .. } => {
                Some((ResponseStatus::Completed, Some(result), None))
            }
            Event::GameResponse {
                status,
                result,
                error,
                ..
            } => Some((*status, result.as_ref(), error.as_ref())),
            _ => None,
        });
    let Some((status, result, error)) = expected_response else {
        if actual.is_none() {
            return Ok(());
        }
        return Err(Error::mismatch(
            recorded.sequence,
            recorded.action,
            "Replay produced an unexpected response",
            Value::Null,
            response_value(actual.unwrap()),
            artifacts.to_vec(),
        ));
    };
    let Some(actual) = actual else {
        return Err(Error::mismatch(
            recorded.sequence,
            recorded.action,
            "Replay did not produce the recorded response",
            expected_value(recorded.expected),
            Value::Null,
            artifacts.to_vec(),
        ));
    };
    let same = actual.status == status
        && match status {
            ResponseStatus::Error => {
                actual.error.as_ref().map(|value| &value.code) == error.map(|value| &value.code)
            }
            ResponseStatus::Completed if !artifacts.is_empty() => {
                artifacts.iter().all(|expected| {
                    actual
                        .result
                        .as_ref()
                        .and_then(|value| value.get("artifact"))
                        .is_some_and(|value| artifact_matches(expected, value))
                })
            }
            ResponseStatus::Completed => actual.result.as_ref() == result,
        };
    if same {
        return Ok(());
    }
    Err(Error::mismatch(
        recorded.sequence,
        recorded.action,
        "Replay result differs from the Session Recording",
        expected_value(recorded.expected),
        response_value(actual),
        artifacts.to_vec(),
    ))
}

fn artifact_matches(expected: &ArtifactReference, actual: &Value) -> bool {
    actual.get("type").and_then(Value::as_str) == Some(expected.kind.as_str())
        && actual.get("path").and_then(Value::as_str) == Some(expected.path.as_str())
        && actual.get("mime_type").and_then(Value::as_str) == Some(expected.mime_type.as_str())
        && expected.width.is_none_or(|width| {
            actual.get("width").and_then(Value::as_u64) == Some(u64::from(width))
        })
        && expected.height.is_none_or(|height| {
            actual.get("height").and_then(Value::as_u64) == Some(u64::from(height))
        })
}

fn expected_value(entries: &[recording::Entry]) -> Value {
    Value::Array(
        entries
            .iter()
            .filter_map(|entry| match &entry.event {
                Event::Observation { result, .. } => {
                    Some(json!({"status": "completed", "result": result}))
                }
                Event::GameResponse {
                    status,
                    result,
                    error,
                    ..
                } => Some(json!({"status": status, "result": result, "error": error})),
                _ => None,
            })
            .collect(),
    )
}

fn response_value(response: &Response) -> Value {
    json!({"status": response.status, "result": response.result, "error": response.error})
}

fn write_result(artifact_dir: &Path, result: &ResultArtifact) -> Result<(), String> {
    fs::create_dir_all(artifact_dir)
        .map_err(|error| format!("could not create replay artifact directory: {error}"))?;
    let path = artifact_dir.join(RESULT_NAME);
    let data = serde_json::to_vec_pretty(result).map_err(|error| error.to_string())?;
    fs::write(&path, data).map_err(|error| format!("could not write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Config {
        Config::parse(include_str!("../../tests/fixtures/host_profile.toml")).unwrap()
    }

    fn context(configuration: Value) -> SessionContext {
        SessionContext::new("alpha", RunMode::Logical, configuration)
    }

    #[test]
    fn replay_accepts_legacy_context_and_rejects_a_conflicting_profile() {
        let profile = profile();
        let legacy = context(json!({
            "mode": "logical",
            "surface": {"width": 640.0, "height": 360.0},
            "paused": false
        }));
        assert_eq!(validate_context(&profile, &legacy).unwrap(), Mode::Logical);

        let conflicting = context(json!({
            "profile_id": "different-profile",
            "mode": "logical",
            "surface": {"width": 640.0, "height": 360.0}
        }));
        assert!(
            validate_context(&profile, &conflicting)
                .unwrap_err()
                .to_string()
                .contains("conflicts")
        );
    }

    #[test]
    fn rendered_artifacts_compare_metadata_instead_of_png_bytes() {
        let expected = ArtifactReference {
            kind: "screenshot".into(),
            path: "screens/current.png".into(),
            mime_type: "image/png".into(),
            width: Some(640),
            height: Some(360),
        };
        assert!(artifact_matches(
            &expected,
            &json!({
                "type": "screenshot", "path": "screens/current.png", "mime_type": "image/png",
                "width": 640, "height": 360, "bytes": "different-platform-bytes"
            })
        ));
        assert!(!artifact_matches(
            &expected,
            &json!({
                "type": "screenshot", "path": "screens/current.png", "mime_type": "image/png",
                "width": 1, "height": 360
            })
        ));
    }
}
