//! Separate Cargo profile, real Bevy panic, public Session and local report.
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use woodpecker::{
    command::tick::warp,
    report::{self, Origin},
    session::{self, Event, Session},
};

#[test]
fn separately_built_abort_panic_is_reported_before_end_without_duplicate_exit() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = repo.join(format!(
        "target/panic-abort-acceptance-{}",
        std::process::id()
    ));
    assert!(!root.exists(), "preserve prior evidence");
    let mut session = Session::start(session::Config {
        launch: session::launch::Config {
            manifest_path: repo.join("tests/fixtures/panic_abort/Cargo.toml"),
            package: "panic_abort_fixture".into(),
            target: session::launch::Target::Binary {
                name: "panic_abort_fixture".into(),
            },
            features: vec![],
            arguments: vec!["panic".into()],
        },
        artifact_dir: root.clone(),
        tick: Default::default(),
        report: report::Config {
            tracing_errors: false,
            output: "reports".into(),
            provider: report::provider::Config::Local,
        },
    })
    .unwrap();
    let pid: i32 = std::fs::read_to_string(root.join("fixture-pid"))
        .unwrap()
        .parse()
        .unwrap();
    let tick = session
        .send(warp::Start {
            ticks: 1,
            pace: None,
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut events = Vec::new();
    let mut reports = Vec::new();
    loop {
        assert!(Instant::now() < deadline, "event deadline");
        if let Some(event) = session.try_receive_event().unwrap() {
            events.push(serde_json::to_value(&event).unwrap());
            match event {
                Event::Failure { failure } => {
                    assert_eq!(failure.message(), Some("fixture panic\nsecond line"));
                    assert!(
                        matches!(failure.origin(),Origin::Panic { location:Some(_),backtrace:Some(text),backtrace_status:report::BacktraceStatus::Captured } if !text.is_empty())
                    );
                    reports.push(report::Report::create(failure, &session));
                }
                Event::Ended { .. } => break,
                other => panic!("unexpected event: {other:?}"),
            }
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    assert_eq!(reports.len(), 1, "no duplicate ProcessExit failure");
    assert_eq!(events.len(), 2, "exactly Failure then Ended");
    assert_eq!(events[0]["kind"], "failure");
    assert_eq!(events[1]["kind"], "ended");
    #[cfg(unix)]
    assert!(
        events[1]["reason"]["status"]
            .as_str()
            .unwrap()
            .contains("SIGABRT")
    );
    assert!(
        matches!(session.receive(tick), Err(session::Error::Ended)),
        "aborted tick must resolve as session ended, not success"
    );
    assert!(session.try_receive_event().is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("prior-hook")).unwrap(),
        "called\n"
    );
    let snapshot = serde_json::to_value(&reports[0]).unwrap();
    let markdown = reports[0].to_markdown();
    let report::provider::Outcome::Created {
        reference: report::provider::Reference::File(report::provider::FileReference { path }),
    } = report::submit(&reports[0], &session).unwrap()
    else {
        panic!("local report not created")
    };
    assert_eq!(std::fs::read_to_string(root.join(&path)).unwrap(), markdown);
    assert_eq!(serde_json::to_value(&reports[0]).unwrap(), snapshot);
    assert_eq!(snapshot["context"]["commands"].as_array().unwrap().len(), 1);
    assert_eq!(
        snapshot["context"]["commands"][0]["outcome"]["status"],
        "unanswered"
    );
    drop(session);
    #[cfg(unix)]
    {
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
    std::fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "acceptance":"passed","panic_strategy":"abort","mode":"panic",
        "events":events,"report":snapshot,"markdown_path":path,"process_id":pid,"process_gone":true
    })).unwrap()).unwrap();
    eprintln!("panic=abort evidence: {}", root.display());
}
