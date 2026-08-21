use super::{
    FailureReport,
    recording::{Event, Recording, SessionOutcome},
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fmt, fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize)]
pub(crate) struct Report {
    artifact_dir: PathBuf,
    sessions: Vec<Session>,
    #[serde(skip_serializing_if = "Option::is_none")]
    replay: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<Failure>,
}

#[derive(Debug, Serialize)]
struct Session {
    recording: PathBuf,
    configuration: Value,
    controllers: Vec<String>,
    result: &'static str,
    errors: Vec<RecordedError>,
    artifacts: Vec<Value>,
}

#[derive(Debug, Serialize)]
struct RecordedError {
    kind: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct Failure {
    kind: String,
    message: String,
    cli_error: Option<String>,
}

impl Report {
    pub(crate) fn load(artifact_dir: impl AsRef<Path>) -> Result<Self, String> {
        let artifact_dir = artifact_dir.as_ref();
        if !artifact_dir.is_dir() {
            return Err(format!(
                "artifact directory {} does not exist",
                artifact_dir.display()
            ));
        }
        let mut replay = read_optional_json(&artifact_dir.join("replay-result.json"))?;
        let replay_artifact_root = replay
            .as_ref()
            .and_then(|replay| replay.get("artifact_root"))
            .and_then(Value::as_str)
            .map(PathBuf::from);
        let mut recordings = Vec::new();
        find_recordings(artifact_dir, &mut recordings)?;
        recordings.sort();
        let sessions = recordings
            .into_iter()
            .map(|path| load_session(artifact_dir, &path, replay_artifact_root.as_deref()))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(replay) = &mut replay {
            annotate_replay_artifacts(replay);
        }
        let failure = match FailureReport::load(artifact_dir.join("failure.json")) {
            Ok(failure) => Some(Failure {
                kind: failure.kind,
                message: failure.message,
                cli_error: failure.cli_error,
            }),
            Err(_error) if !artifact_dir.join("failure.json").exists() => None,
            Err(error) => return Err(error.to_string()),
        };
        Ok(Self {
            artifact_dir: artifact_dir.to_path_buf(),
            sessions,
            replay,
            failure,
        })
    }

    pub(crate) fn json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|error| error.to_string())
    }
}

impl fmt::Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "Artifact report: {}",
            self.artifact_dir.display()
        )?;
        if self.sessions.is_empty() {
            writeln!(formatter, "Sessions: none")?;
        }
        for session in &self.sessions {
            writeln!(formatter, "Session: {}", session.recording.display())?;
            writeln!(
                formatter,
                "  controllers: {}",
                session.controllers.join(", ")
            )?;
            writeln!(formatter, "  result: {}", session.result)?;
            writeln!(formatter, "  configuration: {}", session.configuration)?;
            writeln!(formatter, "  errors: {}", session.errors.len())?;
            writeln!(formatter, "  artifacts: {}", session.artifacts.len())?;
        }
        if let Some(replay) = &self.replay {
            writeln!(formatter, "Replay:")?;
            writeln!(formatter, "  controller: {}", replay["controller"])?;
            writeln!(formatter, "  result: {}", replay["outcome"])?;
            writeln!(formatter, "  configuration: {}", replay["configuration"])?;
            writeln!(
                formatter,
                "  errors: {}",
                usize::from(!replay["error"].is_null())
            )?;
            writeln!(
                formatter,
                "  artifacts: {}",
                replay["artifacts"].as_array().map_or(0, Vec::len)
            )?;
        }
        if let Some(failure) = &self.failure {
            writeln!(formatter, "Failure: {}: {}", failure.kind, failure.message)?;
        }
        Ok(())
    }
}

fn find_recordings(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            find_recordings(&path, found)?;
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("jsonl")
        {
            found.push(path);
        }
    }
    Ok(())
}

fn load_session(
    root: &Path,
    path: &Path,
    replay_artifact_root: Option<&Path>,
) -> Result<Session, String> {
    let recording =
        Recording::parse_path(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let configuration = match &recording.entries[0].event {
        Event::SessionStarted { context } => context.configuration.clone(),
        _ => Value::Null,
    };
    let controllers: Vec<String> = recording
        .entries
        .iter()
        .filter_map(|entry| match &entry.event {
            Event::ControllerAction { controller, .. } => Some(controller.origin.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let artifact_root = if controllers.iter().any(|controller| controller == "replay") {
        replay_artifact_root.unwrap_or(root)
    } else {
        root
    };
    let errors = recording
        .entries
        .iter()
        .filter_map(|entry| match &entry.event {
            Event::Error { kind, message } => Some(RecordedError {
                kind: kind.clone(),
                message: message.clone(),
            }),
            _ => None,
        })
        .collect();
    let artifacts = recording
        .entries
        .iter()
        .filter_map(|entry| match &entry.event {
            Event::Artifact { artifact, .. } => Some(serde_json::json!({
                "reference": artifact,
                "status": if artifact_root.join(&artifact.path).is_file() { "present" } else { "missing" },
            })),
            _ => None,
        })
        .collect();
    let result = match &recording
        .entries
        .last()
        .expect("validated recording has entries")
        .event
    {
        Event::SessionEnded {
            outcome: SessionOutcome::Completed,
        } => "completed",
        Event::SessionEnded {
            outcome: SessionOutcome::Aborted,
        } => "aborted",
        Event::RecordingStopped => "recording_stopped",
        _ => "unknown",
    };
    Ok(Session {
        recording: path.strip_prefix(root).unwrap_or(path).to_path_buf(),
        configuration,
        controllers,
        result,
        errors,
        artifacts,
    })
}

fn annotate_replay_artifacts(replay: &mut Value) {
    let Some(root) = replay
        .get("artifact_root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
    else {
        return;
    };
    let Some(artifacts) = replay.get_mut("artifacts").and_then(Value::as_array_mut) else {
        return;
    };
    for artifact in artifacts {
        let status = artifact
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| root.join(path).is_file());
        *artifact = serde_json::json!({
            "reference": artifact.take(),
            "status": if status { "present" } else { "missing" },
        });
    }
}

fn read_optional_json(path: &Path) -> Result<Option<Value>, String> {
    match fs::read(path) {
        Ok(data) => serde_json::from_slice(&data)
            .map(Some)
            .map_err(|error| format!("invalid JSON in {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("could not read {}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn replay_session_artifacts_are_resolved_below_the_replay_root() {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "star-sim-debug-report-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let replay_root = root.join("replay-artifacts/session-test");
        fs::create_dir_all(replay_root.join("captures")).unwrap();
        fs::write(replay_root.join("captures/frame.png"), b"png").unwrap();
        let recording_path = root.join("replay.jsonl");
        fs::write(
            &recording_path,
            concat!(
                "{\"version\":1,\"sequence\":1,\"type\":\"session_started\",\"context\":{\"session_id\":\"alpha\",\"mode\":\"rendered\",\"protocol_version\":2,\"configuration\":{\"mode\":\"rendered\"}}}\n",
                "{\"version\":1,\"sequence\":2,\"type\":\"controller_action\",\"controller\":{\"origin\":\"replay\"},\"action\":{\"type\":\"screenshot\",\"path\":\"captures/frame.png\"}}\n",
                "{\"version\":1,\"sequence\":3,\"type\":\"game_response\",\"request_sequence\":1,\"status\":\"completed\",\"result\":{\"artifact\":{\"type\":\"screenshot\",\"path\":\"captures/frame.png\",\"mime_type\":\"image/png\",\"width\":640,\"height\":360}}}\n",
                "{\"version\":1,\"sequence\":4,\"type\":\"artifact\",\"request_sequence\":1,\"artifact\":{\"kind\":\"screenshot\",\"path\":\"captures/frame.png\",\"mime_type\":\"image/png\",\"width\":640,\"height\":360}}\n",
                "{\"version\":1,\"sequence\":5,\"type\":\"recording_stopped\"}\n"
            ),
        )
        .unwrap();

        let session = load_session(&root, &recording_path, Some(&replay_root)).unwrap();

        assert_eq!(session.artifacts[0]["status"], "present");
        fs::remove_dir_all(root).ok();
    }
}
