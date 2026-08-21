use automation_control::driver::{FAILURE_REPORT_VERSION, FailureReport, IssueDraft};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn failure_artifacts_generate_a_typed_issue_draft() {
    let artifact_dir = unique_temp_dir();
    fs::create_dir_all(&artifact_dir).expect("artifact directory should be created");
    let record_path = artifact_dir.join("custom-session.jsonl");
    let failure = FailureReport {
        version: FAILURE_REPORT_VERSION,
        kind: "panic".into(),
        message: "stellar catalog invariant violated".into(),
        cli_error: Some("child exited with status 101".into()),
        record_path: Some(record_path.clone()),
    };
    failure
        .write_to(artifact_dir.join("failure.json"))
        .expect("typed failure report should be written");
    fs::write(
        artifact_dir.join("recent.log"),
        "ERROR star_sim::automation: stellar catalog invariant violated\nstack frame\n",
    )
    .expect("recent log should be written");
    let session = (0..15)
        .map(|sequence| {
            json!({
                "sequence": sequence,
                "direction": "to_app",
                "message": {"id": format!("request-{sequence}")}
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&record_path, format!("{session}\n")).expect("session recording should be written");

    let draft = IssueDraft::from_artifacts(&artifact_dir)
        .expect("typed failure artifacts should produce an issue draft");
    assert_eq!(
        draft.title,
        "[automation failure] stellar catalog invariant violated"
    );
    assert!(draft.body.contains("- Kind: `panic`"));
    assert!(
        draft
            .body
            .contains("- CLI error: `child exited with status 101`")
    );
    assert!(draft.body.contains("ERROR star_sim::automation"));
    assert!(!draft.body.contains("request-2\""));
    assert!(draft.body.contains("request-3\""));
    assert!(draft.body.contains("request-14\""));

    let draft_path = draft
        .write_to(&artifact_dir)
        .expect("issue draft should be written");
    assert_eq!(draft_path, artifact_dir.join("github-issue.md"));
    assert_eq!(
        fs::read_to_string(draft_path).expect("written issue draft should be readable"),
        draft.body
    );

    fs::remove_dir_all(artifact_dir).ok();
}

fn unique_temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "automation-control-issue-report-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos()
    ))
}
