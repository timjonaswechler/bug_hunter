use automation_control::{
    Command, RunMode,
    driver::{
        RecentLogs, Session, SessionConfig, SessionOptions,
        recording::{Controller, Event, Recording, SessionOutcome},
    },
    observation::{Projection, Request as ObservationRequest, Selector},
    time::Command as TimeCommand,
};
use serde_json::json;
use std::{fs, io::Cursor, path::PathBuf, process::Command as ProcessCommand};

#[test]
fn parses_the_public_version_one_fixture() {
    let recording = Recording::parse_reader(Cursor::new(include_str!(
        "fixtures/session_recording_v1.jsonl"
    )))
    .unwrap();
    assert_eq!(recording.entries.len(), 4);
    assert_eq!(recording.entries[0].sequence, 41);
    assert!(matches!(
        recording.entries.last().unwrap().event,
        Event::RecordingStopped
    ));
}

#[test]
fn records_canonical_events_across_repeated_segments_with_continuous_host_sequences() {
    let artifact_root = unique_temp_path("artifacts");
    let mut command = ProcessCommand::new("sh");
    command.args([
        "-c",
        r#"printf '%s\n' '{"type":"ready","version":2,"mode":"logical","controls":["pointer","time"],"observation_scopes":["clock"]}'
while IFS= read -r line; do
  sequence=$(printf '%s' "$line" | sed -n 's/.*"sequence":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *'"type":"observe"'*) printf '{"sequence":%s,"status":"completed","result":{"items":[{"frame":1}]}}\n' "$sequence" ;;
    *'"type":"time"'*) printf '{"sequence":%s,"status":"completed","result":{"artifact":{"type":"screenshot","path":"captures/frame.png","mime_type":"image/png","width":1,"height":1}}}\n' "$sequence" ;;
    *) printf '{"sequence":%s,"status":"completed","result":{}}\n' "$sequence" ;;
  esac
  case "$line" in *'"type":"shutdown"'*) exit 0 ;; esac
done"#,
    ]);
    let options = SessionOptions::new()
        .with_artifact_dir(&artifact_root)
        .with_record(Some(PathBuf::from("first.jsonl")))
        .with_recording_context("alpha", RunMode::Logical, json!({"surface": [640, 360]}))
        .with_controller(Controller::new("repl"));
    let mut session = Session::spawn_command(command, options).expect("child should start");
    assert_eq!(session.ready().unwrap().mode, RunMode::Logical);
    session
        .request(Command::Observe(ObservationRequest::new(
            Selector::Clock,
            Projection::Summary,
        )))
        .unwrap();
    let first_path = session.stop_recording().unwrap();

    session
        .request(Command::Time(TimeCommand::advance(1, 16_666_667)))
        .unwrap();
    let second_path = session
        .start_recording(Some(PathBuf::from("second.jsonl")))
        .unwrap();
    session
        .request(Command::Time(TimeCommand::advance(1, 16_666_667)))
        .unwrap();
    session.shutdown().unwrap();

    let first = Recording::parse_path(first_path).unwrap();
    let second = Recording::parse_path(second_path).unwrap();
    assert!(matches!(
        first.entries[0].event,
        Event::SessionStarted { .. }
    ));
    assert!(first.entries.iter().any(|entry| matches!(
        entry.event,
        Event::ControllerAction { ref controller, .. } if controller.origin == "repl"
    )));
    assert!(
        first
            .entries
            .iter()
            .any(|entry| matches!(entry.event, Event::Observation { .. }))
    );
    assert!(matches!(
        first.entries.last().unwrap().event,
        Event::RecordingStopped
    ));
    assert!(matches!(
        second.entries[0].event,
        Event::SessionStarted { .. }
    ));
    assert!(second.entries.iter().any(|entry| matches!(
        entry.event,
        Event::Artifact { ref artifact, .. }
            if artifact.path == "captures/frame.png" && artifact.mime_type == "image/png"
    )));
    assert!(matches!(
        second.entries.last().unwrap().event,
        Event::SessionEnded {
            outcome: SessionOutcome::Completed
        }
    ));
    assert!(
        second.entries[0].sequence > first.entries.last().unwrap().sequence + 1,
        "events performed while recording was stopped must still consume host sequences"
    );
    fs::remove_dir_all(artifact_root).ok();
}

#[test]
fn recording_redacts_sensitive_values_and_bounds_entries() {
    let artifact_root = unique_temp_path("private-artifacts");
    let mut command = ProcessCommand::new("sh");
    command.args([
        "-c",
        r#"printf '%s\n' '{"type":"ready","version":2,"mode":"logical","controls":["time"],"observation_scopes":[]}'
while IFS= read -r line; do
  sequence=$(printf '%s' "$line" | sed -n 's/.*"sequence":\([0-9][0-9]*\).*/\1/p')
  printf '{"sequence":%s,"status":"completed","result":{"api_key":"provider-key-value","raw_model_prompt":"private prompt body","auth":"neutral-auth-value","note":"sk-live-1234567890"}}\n' "$sequence"
  case "$line" in *'"type":"shutdown"'*) exit 0 ;; esac
done"#,
    ]);
    let options = SessionOptions::new()
        .with_artifact_dir(&artifact_root)
        .with_record(Some(PathBuf::from("private.jsonl")))
        .with_recording_context(
            "alpha",
            RunMode::Logical,
            json!({"credential": "context-secret-value"}),
        );
    let mut session = Session::spawn_command(command, options).unwrap();
    session.ready().unwrap();
    session
        .capture_controller_action(json!({
            "password": "controller-password-value",
            "session_cookie": "session-cookie-value",
            "private_key": "private-key-value",
            "text": "ordinary controller text"
        }))
        .unwrap();
    session
        .capture_controller_action(json!({
            "items": vec!["x".repeat(4096); 200]
        }))
        .unwrap();
    session
        .capture_error(
            "provider_error",
            "authorization: Bearer provider-token-value",
        )
        .unwrap();
    session
        .request(Command::Time(TimeCommand::advance(1, 16_666_667)))
        .unwrap();
    session.shutdown().unwrap();

    let path = artifact_root.join("private.jsonl");
    let raw = fs::read_to_string(&path).unwrap();
    for secret in [
        "context-secret-value",
        "controller-password-value",
        "provider-key-value",
        "private prompt body",
        "provider-token-value",
        "neutral-auth-value",
        "sk-live-1234567890",
        "session-cookie-value",
        "private-key-value",
    ] {
        assert!(!raw.contains(secret), "recording leaked {secret:?}");
    }
    assert!(raw.contains("[redacted]"));
    assert!(raw.contains("ordinary controller text"));
    assert!(raw.lines().all(|line| line.len() <= 256 * 1024));
    Recording::parse_path(path).unwrap();
    fs::remove_dir_all(artifact_root).ok();
}

#[test]
fn implicit_context_waits_for_ready_and_supports_config_and_late_start() {
    let artifact_root = unique_temp_path("implicit-context-artifacts");
    let mut command = ProcessCommand::new("sh");
    command.args([
        "-c",
        r#"printf '%s\n' '{"type":"ready","version":2,"mode":"rendered","controls":[],"observation_scopes":[]}'
while IFS= read -r line; do
  sequence=$(printf '%s' "$line" | sed -n 's/.*"sequence":\([0-9][0-9]*\).*/\1/p')
  printf '{"sequence":%s,"status":"completed","result":{}}\n' "$sequence"
  case "$line" in *'"type":"shutdown"'*) exit 0 ;; esac
done"#,
    ]);
    let options = SessionOptions::from_config(
        &SessionConfig::default(),
        Some(PathBuf::from("from-config.jsonl")),
        RecentLogs::default(),
        artifact_root.clone(),
    );
    let mut session = Session::spawn_command(command, options).unwrap();
    assert_eq!(session.ready().unwrap().mode, RunMode::Rendered);
    session.shutdown().unwrap();

    let recording = Recording::parse_path(artifact_root.join("from-config.jsonl")).unwrap();
    let Event::SessionStarted { context } = &recording.entries[0].event else {
        panic!("implicit recording must start with context")
    };
    assert_eq!(context.mode, RunMode::Rendered);
    assert_eq!(context.protocol_version, 2);

    let mut command = ProcessCommand::new("sh");
    command.args([
        "-c",
        r#"printf '%s\n' '{"type":"ready","version":2,"mode":"logical","controls":[],"observation_scopes":[]}'
while IFS= read -r line; do
  sequence=$(printf '%s' "$line" | sed -n 's/.*"sequence":\([0-9][0-9]*\).*/\1/p')
  printf '{"sequence":%s,"status":"completed","result":{}}\n' "$sequence"
  case "$line" in *'"type":"shutdown"'*) exit 0 ;; esac
done"#,
    ]);
    let mut session = Session::spawn_command(
        command,
        SessionOptions::new().with_artifact_dir(&artifact_root),
    )
    .unwrap();
    session.ready().unwrap();
    let late_path = session
        .start_recording(Some(PathBuf::from("late.jsonl")))
        .unwrap();
    session.shutdown().unwrap();
    Recording::parse_path(late_path).unwrap();
    fs::remove_dir_all(artifact_root).ok();
}

#[test]
fn failed_implicit_ready_writes_fallback_context_error_and_abort() {
    let artifact_root = unique_temp_path("implicit-ready-error-artifacts");
    let mut command = ProcessCommand::new("sh");
    command.args(["-c", "printf 'not-json\\n'; sleep 30"]);
    let mut options = SessionOptions::new().with_artifact_dir(&artifact_root);
    options.record = Some(PathBuf::from("failed-ready.jsonl"));
    let mut session = Session::spawn_command(command, options).unwrap();
    assert!(session.ready().is_err());
    drop(session);

    let recording = Recording::parse_path(artifact_root.join("failed-ready.jsonl")).unwrap();
    assert!(matches!(
        recording.entries[0].event,
        Event::SessionStarted { .. }
    ));
    assert!(recording.entries.iter().any(|entry| matches!(
        entry.event,
        Event::Error { ref kind, .. } if kind == "ready_receive_failed"
    )));
    assert!(matches!(
        recording.entries.last().unwrap().event,
        Event::SessionEnded {
            outcome: SessionOutcome::Aborted
        }
    ));
    fs::remove_dir_all(artifact_root).ok();
}

#[test]
fn error_observation_response_is_recorded_as_response_then_error() {
    let artifact_root = unique_temp_path("observation-error-artifacts");
    let mut command = ProcessCommand::new("sh");
    command.args([
        "-c",
        r#"printf '%s\n' '{"type":"ready","version":2,"mode":"logical","controls":[],"observation_scopes":["clock"]}'
read line
printf '%s\n' '{"sequence":1,"status":"error","result":{"partial":true},"error":{"code":"observe_failed","message":"safe failure"}}'
sleep 30"#,
    ]);
    let mut session = Session::spawn_command(
        command,
        SessionOptions::new()
            .with_artifact_dir(&artifact_root)
            .with_record(Some(PathBuf::from("observe-error.jsonl"))),
    )
    .unwrap();
    session.ready().unwrap();
    assert!(
        session
            .request(Command::Observe(ObservationRequest::new(
                Selector::Clock,
                Projection::Summary,
            )))
            .is_err()
    );
    drop(session);

    let recording = Recording::parse_path(artifact_root.join("observe-error.jsonl")).unwrap();
    assert!(
        !recording
            .entries
            .iter()
            .any(|entry| matches!(entry.event, Event::Observation { .. }))
    );
    let response_index = recording
        .entries
        .iter()
        .position(|entry| matches!(entry.event, Event::GameResponse { .. }))
        .unwrap();
    let error_index = recording
        .entries
        .iter()
        .position(|entry| {
            matches!(
                entry.event,
                Event::Error { ref kind, .. } if kind == "observe_failed"
            )
        })
        .unwrap();
    assert!(response_index < error_index);
    fs::remove_dir_all(artifact_root).ok();
}

#[test]
fn direct_response_parse_and_shutdown_failures_are_recorded() {
    let artifact_root = unique_temp_path("direct-driver-errors-artifacts");
    let mut command = ProcessCommand::new("sh");
    command.args([
        "-c",
        "printf '%s\\n' '{\"type\":\"ready\",\"version\":2,\"mode\":\"logical\",\"controls\":[],\"observation_scopes\":[]}' ; read line; printf 'not-json\\n'; sleep 30",
    ]);
    let mut session = Session::spawn_command(
        command,
        SessionOptions::new()
            .with_artifact_dir(&artifact_root)
            .with_record(Some(PathBuf::from("parse-error.jsonl"))),
    )
    .unwrap();
    session.ready().unwrap();
    assert!(session.request(Command::Shutdown).is_err());
    drop(session);
    let parsed = Recording::parse_path(artifact_root.join("parse-error.jsonl")).unwrap();
    assert!(parsed.entries.iter().any(|entry| matches!(
        entry.event,
        Event::Error { ref kind, .. } if kind == "response_receive_failed"
    )));

    let mut command = ProcessCommand::new("sh");
    command.args([
        "-c",
        r#"printf '%s\n' '{"type":"ready","version":2,"mode":"logical","controls":[],"observation_scopes":[]}'
read line
printf '%s\n' '{"sequence":1,"status":"completed","result":{}}'
exit 7"#,
    ]);
    let mut session = Session::spawn_command(
        command,
        SessionOptions::new()
            .with_artifact_dir(&artifact_root)
            .with_record(Some(PathBuf::from("shutdown-error.jsonl"))),
    )
    .unwrap();
    session.ready().unwrap();
    assert!(session.shutdown().is_err());
    let shutdown = Recording::parse_path(artifact_root.join("shutdown-error.jsonl")).unwrap();
    assert!(shutdown.entries.iter().any(|entry| matches!(
        entry.event,
        Event::Error { ref kind, .. } if kind == "shutdown_child_failed"
    )));
    fs::remove_dir_all(artifact_root).ok();
}

#[test]
fn ready_mode_mismatch_records_a_parseable_abort() {
    let artifact_root = unique_temp_path("mode-mismatch-artifacts");
    let mut command = ProcessCommand::new("sh");
    command.args([
        "-c",
        "printf '%s\\n' '{\"type\":\"ready\",\"version\":2,\"mode\":\"rendered\",\"controls\":[],\"observation_scopes\":[]}' ; sleep 30",
    ]);
    let mut session = Session::spawn_command(
        command,
        SessionOptions::new()
            .with_artifact_dir(&artifact_root)
            .with_record(Some(PathBuf::from("mismatch.jsonl")))
            .with_recording_context("alpha", RunMode::Logical, json!({})),
    )
    .unwrap();
    assert!(session.ready().unwrap_err().to_string().contains("mode"));
    drop(session);

    let recording = Recording::parse_path(artifact_root.join("mismatch.jsonl")).unwrap();
    assert!(recording.entries.iter().any(|entry| matches!(
        entry.event,
        Event::Error { ref kind, .. } if kind == "ready_mode_mismatch"
    )));
    assert!(matches!(
        recording.entries.last().unwrap().event,
        Event::SessionEnded {
            outcome: SessionOutcome::Aborted
        }
    ));
    fs::remove_dir_all(artifact_root).ok();
}

#[test]
fn dropping_a_live_recorded_session_writes_an_aborted_end() {
    let artifact_root = unique_temp_path("aborted-artifacts");
    let mut command = ProcessCommand::new("sh");
    command.args([
        "-c",
        "printf '%s\\n' '{\"type\":\"ready\",\"version\":2,\"mode\":\"logical\",\"controls\":[],\"observation_scopes\":[]}' ; sleep 30",
    ]);
    let mut session = Session::spawn_command(
        command,
        SessionOptions::new()
            .with_artifact_dir(&artifact_root)
            .with_record(Some(PathBuf::from("aborted.jsonl")))
            .with_recording_context("alpha", RunMode::Logical, json!({})),
    )
    .unwrap();
    session.ready().unwrap();
    drop(session);

    let recording = Recording::parse_path(artifact_root.join("aborted.jsonl")).unwrap();
    assert!(recording.entries.iter().any(|entry| matches!(
        entry.event,
        Event::Error { ref kind, .. } if kind == "session_aborted"
    )));
    assert!(matches!(
        recording.entries.last().unwrap().event,
        Event::SessionEnded {
            outcome: SessionOutcome::Aborted
        }
    ));
    fs::remove_dir_all(artifact_root).ok();
}

fn unique_temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "automation-control-driver-test-{}-{}-{name}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
