use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use woodpecker::{
    command::tick::warp,
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
