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
    assert!(
        !before["context"]
            .to_string()
            .contains(env!("CARGO_MANIFEST_DIR")),
        "metadata leaked an absolute project path"
    );
    let context = before["context"].as_object().unwrap();
    assert_eq!(context.len(), 8);
}
