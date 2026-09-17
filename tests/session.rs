use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use woodpecker::{
    command::{recording, tick::warp},
    report,
    session::{self, Session},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
fn config(mode: &str) -> session::Config {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    session::Config {
        launch: session::launch::Config {
            manifest_path: root.join("tests/fixtures/process/Cargo.toml"),
            package: "process_fixture".into(),
            target: session::launch::Target::Binary {
                name: "process_fixture".into(),
            },
            features: vec![],
            arguments: vec![mode.into()],
        },
        artifact_dir: root.join(format!(
            "target/session-tests/{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )),
        tick: Default::default(),
        report: report::Config {
            tracing_errors: false,
            output: "reports".into(),
            provider: report::provider::Config::Local,
        },
    }
}
fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "condition timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn recording_lines(root: &std::path::Path, path: &str) -> Vec<serde_json::Value> {
    std::fs::read_to_string(root.join(path))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn recording_orders_outcomes_and_excludes_controls_and_shutdown() {
    let config = config("normal");
    let root = config.artifact_dir.clone();
    let mut session = Session::start(config).unwrap();
    let start = session
        .send(recording::Start {
            path: "records/session.jsonl".into(),
        })
        .unwrap();
    // Submit before consuming Start: execution must wait for its file barrier.
    let warp = session
        .send(warp::Start {
            ticks: 100,
            pace: None,
        })
        .unwrap();
    assert_eq!(
        session.receive(start).unwrap().path,
        "records/session.jsonl"
    );
    assert_eq!(
        recording_lines(&root, "records/session.jsonl"),
        vec![serde_json::json!({"type":"recording_started","format_version":1})]
    );
    let stop_recording = session.send(recording::Stop {}).unwrap();
    assert_eq!(
        session.receive(stop_recording).unwrap_err().code(),
        "recording_commands_pending"
    );
    assert_eq!(
        session.shutdown().unwrap_err().code(),
        "shutdown_commands_pending"
    );
    // The fixture deliberately answers Stop before the older Warp.
    let stop = session.send(warp::Stop {}).unwrap();
    session.receive(stop).unwrap();
    session.receive(warp).unwrap();
    let history_len = session.history().len();
    assert_eq!(
        session.shutdown().unwrap_err().code(),
        "shutdown_recording_active"
    );
    assert_eq!(session.history().len(), history_len);
    let stop = session.send(recording::Stop {}).unwrap();
    let after = session.send(warp::Stop {}).unwrap();
    let result = session.receive(stop).unwrap();
    assert_eq!(result.recorded_commands, 2);
    session.receive(after).unwrap();
    let lines = recording_lines(&root, &result.path);
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[1]["command"], "tick.warp.start");
    assert_eq!(lines[1]["outcome"]["output"]["executed_ticks"], 0);
    assert_eq!(lines[2]["command"], "tick.warp.stop");
    assert_eq!(
        lines[3],
        serde_json::json!({"type":"recording_ended","outcome":"stopped","recorded_commands":2})
    );
    assert!(lines.iter().all(|line| line.get("request_id").is_none()));
    let duplicate = session
        .send(recording::Start {
            path: result.path.clone(),
        })
        .unwrap();
    assert_eq!(
        session.receive(duplicate).unwrap_err().code(),
        "recording_path_exists"
    );
    let stopped = session.send(recording::Stop {}).unwrap();
    assert_eq!(
        session.receive(stopped).unwrap_err().code(),
        "recording_not_active"
    );
    session.shutdown().unwrap();
    assert_eq!(recording_lines(&root, &result.path), lines);
}

#[test]
fn recording_start_rejects_pending_work_and_abandoned_controls_still_finish() {
    let config = config("normal");
    let root = config.artifact_dir.clone();
    let mut session = Session::start(config).unwrap();
    let warp = session
        .send(warp::Start {
            ticks: 100,
            pace: None,
        })
        .unwrap();
    let start = session
        .send(recording::Start {
            path: "pending.jsonl".into(),
        })
        .unwrap();
    assert_eq!(
        session.receive(start).unwrap_err().code(),
        "recording_commands_pending"
    );
    assert!(!root.join("pending.jsonl").exists());
    let stop = session.send(warp::Stop {}).unwrap();
    session.receive(stop).unwrap();
    session.receive(warp).unwrap();
    drop(
        session
            .send(recording::Start {
                path: "empty.jsonl".into(),
            })
            .unwrap(),
    );
    wait(|| {
        session.history().last().is_some_and(|entry| {
            matches!(entry.outcome, session::history::Outcome::Completed { .. })
        })
    });
    let duplicate = session
        .send(recording::Start {
            path: "other.jsonl".into(),
        })
        .unwrap();
    assert_eq!(
        session.receive(duplicate).unwrap_err().code(),
        "recording_already_active"
    );
    let stop = session.send(recording::Stop {}).unwrap();
    assert_eq!(session.receive(stop).unwrap().recorded_commands, 0);
    assert_eq!(recording_lines(&root, "empty.jsonl").len(), 2);
    session.shutdown().unwrap();
}

#[test]
fn recording_keeps_protocol_failures_and_unanswered_commands_on_process_exit() {
    let settings = config("corrupt");
    let root = settings.artifact_dir.clone();
    let mut session = Session::start(settings).unwrap();
    let start = session
        .send(recording::Start {
            path: "protocol.jsonl".into(),
        })
        .unwrap();
    session.receive(start).unwrap();
    let stop = session.send(warp::Stop {}).unwrap();
    assert!(matches!(
        session.receive(stop),
        Err(session::Error::Protocol { .. })
    ));
    let stop = session.send(recording::Stop {}).unwrap();
    session.receive(stop).unwrap();
    assert_eq!(
        recording_lines(&root, "protocol.jsonl")[1]["outcome"]["status"],
        "protocol_failed"
    );
    session.shutdown().unwrap();

    let settings = config("normal");
    let root = settings.artifact_dir.clone();
    let mut session = Session::start(settings).unwrap();
    let start = session
        .send(recording::Start {
            path: "ended.jsonl".into(),
        })
        .unwrap();
    session.receive(start).unwrap();
    let pending = session
        .send(warp::Start {
            ticks: 100,
            pace: None,
        })
        .unwrap();
    let pid: i32 = std::fs::read_to_string(root.join("pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
    assert!(matches!(
        session.receive_event().unwrap(),
        session::Event::Ended { .. }
    ));
    assert!(session.receive(pending).is_err());
    let lines = recording_lines(&root, "ended.jsonl");
    assert_eq!(
        lines[1]["outcome"],
        serde_json::json!({"status":"unanswered"})
    );
    assert_eq!(
        lines[2],
        serde_json::json!({"type":"recording_ended","outcome":"session_ended","recorded_commands":1})
    );
}

#[test]
fn pending_correlation_history_and_drop() {
    let mut first = Session::start(config("normal")).unwrap();
    let mut second = Session::start(config("normal")).unwrap();
    let mut pending = first
        .send(warp::Start {
            ticks: 100,
            pace: None,
        })
        .unwrap();
    assert_eq!(pending.request_id().as_u64(), 1);
    assert_eq!(
        second.try_receive(&mut pending).unwrap_err().code(),
        "invalid_pending"
    );
    assert_eq!(
        first.shutdown().unwrap_err().code(),
        "shutdown_commands_pending"
    );
    let stop = first.send(warp::Stop {}).unwrap();
    assert_eq!(stop.request_id().as_u64(), 2);
    assert!(first.receive(stop).unwrap().was_running);
    let completion = first.receive(pending).unwrap();
    assert_eq!(completion.outcome, warp::Outcome::Stopped);

    let abandoned = first
        .send(warp::Start {
            ticks: 100,
            pace: None,
        })
        .unwrap();
    drop(abandoned);
    assert_eq!(
        first.shutdown().unwrap_err().code(),
        "shutdown_commands_pending"
    );
    let stop = first.send(warp::Stop {}).unwrap();
    first.receive(stop).unwrap();
    wait(|| {
        first
            .history()
            .iter()
            .all(|e| !matches!(e.outcome, session::history::Outcome::Unanswered))
    });
    for _ in 0..60 {
        let pending = first
            .send(warp::Start {
                ticks: 1,
                pace: None,
            })
            .unwrap();
        first.receive(pending).unwrap();
    }
    assert_eq!(first.history().len(), 50);
    let mut once = first.send(warp::Stop {}).unwrap();
    wait(|| first.try_receive(&mut once).unwrap().is_some());
    assert_eq!(
        first.try_receive(&mut once).unwrap_err().code(),
        "invalid_pending"
    );
    first.shutdown().unwrap();
    second.shutdown().unwrap();
    assert!(first.try_receive_event().is_err());
}

#[test]
fn pipe_drain_and_progress_do_not_need_polling() {
    let mut session = Session::start(config("full_pipes")).unwrap();
    let pending = session
        .send(warp::Start {
            ticks: 1,
            pace: None,
        })
        .unwrap();
    wait(|| {
        matches!(
            session.history()[0].outcome,
            session::history::Outcome::Completed { .. }
        )
    });
    assert_eq!(session.receive(pending).unwrap().executed_ticks, 1);
    session.shutdown().unwrap();
}

#[test]
fn protocol_errors_are_correlated_without_poisoning_other_work() {
    for mode in ["corrupt", "wrong_name", "known_error"] {
        let mut session = Session::start(config(mode)).unwrap();
        let pending = session.send(warp::Stop {}).unwrap();
        assert!(matches!(
            session.receive(pending),
            Err(session::Error::Protocol { .. })
        ));
        session.shutdown().unwrap();
    }
    let mut session = Session::start(config("unsolicited")).unwrap();
    let pending = session.send(warp::Stop {}).unwrap();
    session.receive(pending).unwrap();
    assert!(
        matches!(session.receive_event().unwrap(), session::Event::ProtocolError { code, .. } if code == "fixture")
    );
    assert!(
        matches!(session.receive_event().unwrap(), session::Event::ProtocolError { code, .. } if code == "unexpected_ready")
    );
    assert!(!session.capabilities().screenshot);
    session.shutdown().unwrap();
}

#[test]
fn exit_drains_responses_and_closes_events_once() {
    let config = config("exit");
    let root = config.artifact_dir.clone();
    let mut session = Session::start(config).unwrap();
    let pending = session.send(warp::Stop {}).unwrap();
    assert!(!session.receive(pending).unwrap().was_running);
    assert!(matches!(
        session.receive_event().unwrap(),
        session::Event::Ended { .. }
    ));
    assert!(session.receive_event().is_err());
    let pid: i32 = std::fs::read_to_string(root.join("descendant"))
        .unwrap()
        .parse()
        .unwrap();
    // A reparented descendant can briefly remain a zombie until the OS reaps it.
    wait(|| unsafe { libc::kill(pid, 0) } != 0);
}

#[test]
fn overflow_reserves_exactly_one_end_event() {
    let mut session = Session::start(config("overflow")).unwrap();
    // Allow the private coordinator to fill its bounded event queue, without consuming it.
    std::thread::sleep(Duration::from_millis(300));
    let mut count = 0;
    loop {
        match session.receive_event().unwrap() {
            session::Event::ProtocolError { .. } => count += 1,
            session::Event::Ended {
                reason:
                    session::EndReason::EventQueueOverflow {
                        capacity,
                        dropped_events,
                    },
            } => {
                assert_eq!(count, 256);
                assert_eq!(capacity, 256);
                assert!(dropped_events > 0);
                break;
            }
            event => panic!("unexpected event: {event:?}"),
        }
    }
    assert!(session.receive_event().is_err());
}

#[test]
fn wrong_version_cleans_up_before_returning() {
    let config = config("wrong_version");
    let root = config.artifact_dir.clone();
    let error = Session::start(config).err().expect("unsupported version");
    assert_eq!(error.code(), "unsupported_protocol_version");
    let pid = std::fs::read_to_string(root.join("pid"))
        .unwrap()
        .parse::<i32>()
        .unwrap();
    assert_ne!(unsafe { libc::kill(pid, 0) }, 0);
}
