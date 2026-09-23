use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use woodpecker::{
    command::tick::warp,
    report::{self, Origin},
    session::{self, Event, Session},
};

fn config(mode: &str, tracing: bool) -> session::Config {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    session::Config {
        launch: session::launch::Config {
            manifest_path: root.join("Cargo.toml"),
            package: "woodpecker".into(),
            target: session::launch::Target::Example {
                name: "observation_fixture".into(),
            },
            features: vec![],
            arguments: vec![mode.into()],
        },
        artifact_dir: root.join(format!(
            "target/observation-tests/{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )),
        tick: Default::default(),
        report: report::Config {
            tracing_errors: tracing,
            output: "reports".into(),
            provider: report::provider::Config::Local,
        },
    }
}

fn event(session: &mut Session) -> Event {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(event) = session.try_receive_event().unwrap() {
            return event;
        }
        assert!(Instant::now() < deadline, "event timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn caught_worker_and_unknown_panics_are_observed_without_ending_the_session() {
    for (mode, message) in [
        ("caught", Some("caught panic")),
        ("worker", Some("worker panic")),
        ("unknown", None),
        ("task", Some("task panic")),
    ] {
        let config = config(mode, false);
        let root = config.artifact_dir.clone();
        let mut session = Session::start(config).unwrap();
        let tick = session
            .send(warp::Start {
                ticks: 1,
                pace: None,
            })
            .unwrap();
        session.receive(tick).unwrap();
        let Event::Failure { failure } = event(&mut session) else {
            panic!("expected panic");
        };
        assert_eq!(failure.message(), message);
        assert!(
            matches!(failure.origin(), Origin::Panic { location: Some(_), backtrace: Some(text),
            backtrace_status: report::BacktraceStatus::Captured } if !text.is_empty())
        );
        assert!(
            root.join("prior-hook").is_file(),
            "previous hook was not called"
        );
        session.shutdown().unwrap();
    }
}

#[test]
fn panic_and_abort_are_drained_before_ended_without_a_second_process_exit_failure() {
    for mode in [
        "panic",
        "abort",
        "exit_zero",
        "corrupt_marker",
        "incomplete_marker",
    ] {
        let mut session = Session::start(config(mode, false)).unwrap();
        let _pending = session
            .send(warp::Start {
                ticks: 1,
                pace: None,
            })
            .unwrap();
        let mut failures = Vec::new();
        let mut observation_errors = 0;
        loop {
            match event(&mut session) {
                Event::Failure { failure } => failures.push(failure),
                Event::ObservationError { code, .. } => {
                    assert!(
                        failures.is_empty(),
                        "marker errors must precede the exit fallback"
                    );
                    assert_eq!(code, "invalid_report_marker");
                    observation_errors += 1;
                }
                Event::Ended { .. } => break,
                other => panic!("{other:?}"),
            }
        }
        assert!(!failures.is_empty());
        if matches!(mode, "exit_zero" | "corrupt_marker" | "incomplete_marker") {
            assert_eq!(failures.len(), 1);
            assert!(matches!(failures[0].origin(), Origin::ProcessExit { .. }));
            assert_eq!(failures[0].message(), Some("process exited unexpectedly"));
        } else {
            assert!(
                failures
                    .iter()
                    .all(|f| matches!(f.origin(), Origin::Panic { .. }))
            );
        }
        assert_eq!(
            observation_errors,
            usize::from(matches!(mode, "corrupt_marker" | "incomplete_marker"))
        );
        assert!(session.try_receive_event().is_err());
    }
}

#[test]
fn tracing_is_opt_in_obeys_filters_and_requires_a_registered_layer() {
    for (mode, enabled, message) in [
        ("trace", true, Some("observed trace")),
        ("fields", true, Some("count=3, valid=true, text=\"Grüße\"")),
        ("trace", false, None),
        ("filtered", true, None),
        ("handled", true, None),
    ] {
        let mut session = Session::start(config(mode, enabled)).unwrap();
        let tick = session
            .send(warp::Start {
                ticks: 1,
                pace: None,
            })
            .unwrap();
        session.receive(tick).unwrap();
        if let Some(message) = message {
            let Event::Failure { failure } = event(&mut session) else {
                panic!("expected tracing");
            };
            assert_eq!(failure.message(), Some(message));
            assert!(
                matches!(failure.origin(), Origin::TracingError { target: Some(target), .. } if target == "fixture")
            );
        }
        session.shutdown().unwrap();
        assert!(
            session.try_receive_event().is_err(),
            "unexpected event after intentional shutdown"
        );
    }
    let error = Session::start(config("missing_layer", true)).err().unwrap();
    assert_eq!(error.code(), "launch");
    assert!(error.message().contains("layer"));
    let mut session = Session::start(config("missing_layer", false)).unwrap();
    session.shutdown().unwrap();
}

#[test]
fn pre_ready_panic_remains_a_launch_diagnostic() {
    let error = Session::start(config("pre_ready", false)).err().unwrap();
    assert_eq!(error.code(), "launch");
    assert!(error.message().contains("panic before Ready"), "{error}");
}

#[test]
fn report_context_is_an_immutable_copy_of_start_metadata_and_correlated_history() {
    let mut session = Session::start(config("caught", false)).unwrap();
    let warp = session
        .send(warp::Start {
            ticks: 100,
            pace: Some(warp::Pace::TicksPerSecond { target: 1.0 }),
        })
        .unwrap();
    let Event::Failure { failure } = event(&mut session) else {
        panic!("expected failure");
    };
    let report = report::Report::create(failure, &session);
    let before = serde_json::to_value(&report).unwrap();
    let markdown = report.to_markdown();
    assert_eq!(report.title(), "caught panic");
    assert!(markdown.contains(&format!(
        "<!-- bug_hunter-signature: {} -->",
        report.signature().as_str()
    )));
    // Signature normalization uses the launch-resolved private project directory,
    // not a path added to the public Context.
    let project = std::fs::canonicalize(env!("CARGO_MANIFEST_DIR")).unwrap();
    let original = format!("error at {}/src.rs", project.display());
    let normalized = report::Report::create(
        report::Failure::panic(Some("error at <project>/src.rs".into()), None, None),
        &session,
    );
    let with_path = report::Report::create(
        report::Failure::panic(Some(original.clone()), None, None),
        &session,
    );
    assert_eq!(with_path.signature(), normalized.signature());
    assert_eq!(with_path.failure().message(), Some(original.as_str()));
    assert_eq!(report.context().application.package, "woodpecker");
    assert_eq!(
        report.context().application.version,
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(report.context().application.arguments, ["caught"]);
    assert_eq!(report.context().protocol_version, 3);
    assert_eq!(report.context().platform.os, std::env::consts::OS);
    assert!(report.context().application.source.is_some());
    assert!(
        report
            .context()
            .toolchain
            .cargo
            .as_ref()
            .unwrap()
            .starts_with("cargo ")
    );
    assert!(matches!(
        report.context().commands[0].outcome,
        session::history::Outcome::Unanswered
    ));
    let stop = session.send(warp::Stop {}).unwrap();
    session.receive(stop).unwrap();
    session.receive(warp).unwrap();
    session.shutdown().unwrap();
    assert_eq!(serde_json::to_value(&report).unwrap(), before);
    assert_eq!(report.to_markdown(), markdown);
    assert_eq!(report.clone().to_markdown(), markdown);
    assert!(
        !before["context"]
            .to_string()
            .contains(env!("CARGO_MANIFEST_DIR")),
        "metadata leaked an absolute project path"
    );
    let context = before["context"].as_object().unwrap();
    assert_eq!(context.len(), 8);
}

#[test]
fn local_reports_use_start_configuration_without_ending_or_mutating_the_session() {
    use report::provider::{FileReference, Outcome, Reference};
    let mut config = config("caught", false);
    let root = config.artifact_dir.clone();
    // Resolve relative roots against the launch directory, not the submitter's cwd.
    config.artifact_dir = root
        .strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
        .into();
    config.report.output = "diagnostics/reports".into();
    let mut session = Session::start(config.clone()).unwrap();
    config.report.output = "ignored-later-change".into();
    let warp = session
        .send(warp::Start {
            ticks: 1,
            pace: None,
        })
        .unwrap();
    session.receive(warp).unwrap();
    let Event::Failure { failure } = event(&mut session) else {
        panic!("expected caught panic");
    };
    let report = report::Report::create(failure, &session);
    assert!(
        !root.join("diagnostics/reports").exists(),
        "no automatic persistence"
    );
    let before = serde_json::to_value(session.history()).unwrap();
    let Outcome::Created {
        reference: Reference::File(FileReference { path }),
    } = report::submit(&report, &session).unwrap()
    else {
        panic!("expected newly created local report");
    };
    assert!(path.starts_with("diagnostics/reports"));
    assert!(!path.is_absolute());
    let expected = report.to_markdown();
    assert_eq!(std::fs::read_to_string(root.join(&path)).unwrap(), expected);
    assert_eq!(serde_json::to_value(session.history()).unwrap(), before);
    assert!(!root.join("ignored-later-change").exists());
    assert!(session.try_receive_event().unwrap().is_none());
    let warp = session
        .send(warp::Start {
            ticks: 1,
            pace: None,
        })
        .unwrap();
    session.receive(warp).unwrap();
    session.shutdown().unwrap();
    assert!(
        matches!(report::submit(&report, &session).unwrap(), Outcome::Existing {
        reference: Reference::File(FileReference { path: ref p })
    } if p == &path)
    );
    // A different report can also be created after session end.
    let ended = report::Report::create(
        report::Failure::process_exit("test diagnosis".into()),
        &session,
    );
    assert!(matches!(
        report::submit(&ended, &session).unwrap(),
        Outcome::Created { .. }
    ));
    assert_eq!(std::fs::read_to_string(root.join(path)).unwrap(), expected);
}

#[cfg(unix)]
#[test]
fn github_reports_use_the_public_session_api_with_an_isolated_cli_fixture() {
    use report::provider::{Config, Outcome, Reference};
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    const CHILD_ROOT: &str = "WOODPECKER_GITHUB_TEST_ROOT";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let root = PathBuf::from(root);
        let mut config = config("caught", false);
        config.artifact_dir = root.join("artifacts");
        config.report.provider = Config::Github;
        let mut session = Session::start(config.clone()).unwrap();
        config.report.provider = Config::Local;
        let warp = session
            .send(warp::Start {
                ticks: 1,
                pace: None,
            })
            .unwrap();
        session.receive(warp).unwrap();
        let Event::Failure { failure } = event(&mut session) else {
            panic!("expected panic");
        };
        let report = report::Report::create(failure, &session);
        assert!(
            matches!(report::submit(&report, &session).unwrap(), Outcome::Created {
            reference: Reference::Issue { ref identifier, .. }
        } if identifier == "42")
        );
        assert!(!root.join("artifacts/reports").exists());
        let request: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("request.json")).unwrap()).unwrap();
        assert_eq!(
            request,
            serde_json::json!({"title": report.title(), "body": report.to_markdown()})
        );
        assert_eq!(
            PathBuf::from(fs::read_to_string(root.join("cwd")).unwrap()),
            fs::canonicalize(env!("CARGO_MANIFEST_DIR")).unwrap()
        );
        fs::write(
            root.join("search.json"),
            serde_json::json!([{
                "number":42, "html_url":"https://example.test/org/repo/issues/42",
                "body":report.to_markdown(), "state":"closed"
            }])
            .to_string(),
        )
        .unwrap();
        session.shutdown().unwrap();
        assert!(matches!(
            report::submit(&report, &session).unwrap(),
            Outcome::Existing {
                reference: Reference::Issue { .. }
            }
        ));
        assert_eq!(
            fs::read_to_string(root.join("calls")).unwrap(),
            "GET\nPOST\nGET\n"
        );
        fs::write(root.join("fail"), "fixture search failed").unwrap();
        let Outcome::Fallback {
            reference,
            provider_error,
        } = report::submit(&report, &session).unwrap()
        else {
            panic!("expected local fallback");
        };
        assert!(matches!(
            provider_error,
            report::provider::Error::Github(report::provider::github::Error::CommandFailed {
                operation: report::provider::github::Operation::Search,
                ..
            })
        ));
        assert_eq!(
            fs::read_to_string(root.join("artifacts").join(reference.path)).unwrap(),
            report.to_markdown()
        );
        return;
    }

    // PATH is changed only for this subprocess, never in the multithreaded test runner.
    let root = config("caught", false).artifact_dir.join("github-fixture");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(root.join("search.json"), "[]").unwrap();
    fs::write(
        bin.join("gh"),
        r#"#!/bin/sh
set -eu
root="$WOODPECKER_GITHUB_TEST_ROOT"
test "$GH_PROMPT_DISABLED" = 1
printf '%s' "$PWD" > "$root/cwd"
printf '%s\n' "$4" >> "$root/calls"
if [ -f "$root/fail" ]; then
  /bin/cat "$root/fail" >&2
  exit 1
fi
case "$4" in
  GET) /bin/cat "$root/search.json";;
  POST)
    /bin/cat > "$root/request.json"
    printf '%s\n' '{"number":42,"html_url":"https://example.test/org/repo/issues/42"}';;
  *) exit 99;;
esac
"#,
    )
    .unwrap();
    fs::set_permissions(bin.join("gh"), fs::Permissions::from_mode(0o700)).unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "github_reports_use_the_public_session_api_with_an_isolated_cli_fixture",
            "--nocapture",
        ])
        .env(CHILD_ROOT, &root)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
