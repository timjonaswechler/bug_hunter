use super::*;
use serde_json::{Value, json};
use std::io::Cursor;

const HEADER: &str = r#"{"type":"recording_started","format_version":1}"#;
const FOOTER: &str = r#"{"type":"recording_ended","outcome":"stopped","recorded_commands":0}"#;

fn parse(text: &str) -> Result<loader::Plan, Error> {
    loader::parse(
        Cursor::new(text),
        "records/test.jsonl",
        &AtomicBool::new(false),
    )
    .map(Option::unwrap)
}

fn source(entries: &[Value], ended: &str) -> String {
    let mut lines = vec![HEADER.into()];
    lines.extend(entries.iter().map(Value::to_string));
    lines.push(
        json!({"type":"recording_ended","outcome":ended,"recorded_commands":entries.len()})
            .to_string(),
    );
    lines.join("\n")
}

fn entry(command: Command, outcome: Value) -> Value {
    let mut entry = serde_json::to_value(command).unwrap();
    entry["type"] = json!("command");
    entry["outcome"] = outcome;
    entry
}

fn warp(ticks: u64) -> Command {
    warp::Start { ticks, pace: None }.into()
}
fn stop() -> Command {
    warp::Stop {}.into()
}
fn successful(output: Value) -> Value {
    json!({"status":"completed","output":output})
}

#[test]
fn header_is_validated_before_any_version_specific_content() {
    assert!(parse(&format!("{HEADER}\n{FOOTER}")).unwrap().is_empty());
    assert!(parse(&format!("{HEADER}\n{FOOTER}\n")).unwrap().is_empty());
    for version in ["2", "0", "18446744073709551615"] {
        let error = parse(&format!(
            "{{\"type\":\"recording_started\",\"format_version\":{version}}}\nnot json"
        ))
        .unwrap_err();
        assert_eq!(error.code(), "unsupported_recording_version");
        assert!(error.message().contains("records/test.jsonl: line 1"));
    }
    for header in [
        "",
        "null",
        "[]",
        "{}",
        " ",
        r#"{"type":"recording_started","format_version":1.0}"#,
        r#"{"type":"recording_started","format_version":-1}"#,
        r#"{"type":"recording_started","format_version":"1"}"#,
        r#"{"type":"recording_started","format_version":18446744073709551616}"#,
        r#"{"type":"wrong","format_version":2}"#,
        r#"{"type":"recording_started","format_version":2,"extra":1}"#,
        r#"{"type":"recording_started","format_version":1,"format_version":2}"#,
    ] {
        assert_eq!(
            parse(&format!("{header}\n{FOOTER}")).unwrap_err().code(),
            "invalid_recording",
            "{header}"
        );
    }
}

#[test]
fn file_structure_fields_arguments_and_outputs_are_strict() {
    let valid = entry(stop(), successful(json!({"was_running":false})));
    let good = source(std::slice::from_ref(&valid), "stopped");
    assert_eq!(parse(&good).unwrap(), [stop()]);
    for text in [
        HEADER.into(),
        format!("{HEADER}\n"),
        format!("{HEADER}\n\n{FOOTER}"),
        format!("{HEADER}\n{FOOTER}\n\n"),
        format!("{HEADER}\n{FOOTER}\n{FOOTER}"),
        format!("{HEADER}\n{FOOTER} {{}}"),
        good.replace("\"recorded_commands\":1", "\"recorded_commands\":0"),
        good.replace("\"type\":\"command\"", "\"type\":\"other\""),
        good.replace("\"arguments\":{}", "\"arguments\":{\"extra\":0}"),
        good.replace("\"arguments\":{}", "\"arguments\":{},\"request_id\":3"),
        good.replace(
            "\"was_running\":false",
            "\"was_running\":false,\"was_running\":true",
        ),
        good.replace("\"was_running\":false", "\"was_running\":\"no\""),
        good.replace(
            "\"status\":\"completed\"",
            "\"status\":\"completed\",\"extra\":0",
        ),
        good.replace("\"output\":{\"was_running\":false},", ""),
        good.replace("\"command\":\"tick.warp.stop\"", "\"command\":\"unknown\""),
        good.replace("\"command\":\"tick.warp.stop\"", "\"command\":\"shutdown\""),
        good.replace(
            "\"command\":\"tick.warp.stop\"",
            "\"command\":\"replay.stop\"",
        ),
        good.replace(
            "\"command\":\"tick.warp.stop\"",
            "\"command\":\"recording.stop\"",
        ),
        good.replace("\"arguments\":{}", "\"arguments\":{},\"arguments\":{}"),
    ] {
        assert_eq!(
            parse(&text).unwrap_err().code(),
            "invalid_recording",
            "{text}"
        );
    }
    for outcome in [
        json!({"status":"rejected","error":{"code":"x","message":"y","extra":0}}),
        json!({"status":"io_failed","error":{"code":"x","message":"y"}}),
        json!({"status":"protocol_failed","error":{"code":"x"}}),
        json!({"status":"unanswered","output":null}),
        json!({"status":"completed"}),
    ] {
        assert_eq!(
            parse(&source(&[entry(stop(), outcome)], "session_ended"))
                .unwrap_err()
                .code(),
            "invalid_recording"
        );
    }
}

#[test]
fn outcomes_are_validated_but_not_expectations_and_warps_use_effective_ticks() {
    let entries = vec![
        entry(
            warp(100),
            successful(json!({"requested_ticks":100,"executed_ticks":3,"outcome":"stopped"})),
        ),
        entry(stop(), successful(json!({"was_running":true}))),
        entry(
            warp(10),
            successful(json!({"requested_ticks":10,"executed_ticks":0,"outcome":"stopped"})),
        ),
        entry(stop(), successful(json!({"was_running":true}))),
        entry(
            warp(1),
            successful(json!({"requested_ticks":1,"executed_ticks":1,"outcome":"completed"})),
        ),
        entry(stop(), successful(json!({"was_running":false}))),
        entry(
            warp(50),
            json!({"status":"rejected","error":{"code":"old","message":"old"}}),
        ),
        entry(
            stop(),
            json!({"status":"protocol_failed","error":{"code":"old","message":"old"}}),
        ),
        entry(
            stop(),
            json!({"status":"io_failed","error":{"message":"old"}}),
        ),
        entry(warp(80), json!({"status":"unanswered"})),
    ];
    assert_eq!(
        parse(&source(&entries, "session_ended")).unwrap(),
        [warp(3), warp(1), stop(), warp(50), stop(), stop(), warp(80)]
    );
    assert_eq!(
        parse(&source(&entries, "stopped")).unwrap_err().code(),
        "invalid_recording"
    );
    for completion in [
        json!({"requested_ticks":99,"executed_ticks":3,"outcome":"stopped"}),
        json!({"requested_ticks":100,"executed_ticks":101,"outcome":"stopped"}),
        json!({"requested_ticks":100,"executed_ticks":99,"outcome":"completed"}),
        json!({"requested_ticks":100,"executed_ticks":-1,"outcome":"stopped"}),
    ] {
        assert!(
            parse(&source(
                &[entry(warp(100), successful(completion))],
                "stopped"
            ))
            .is_err()
        );
    }
}

#[test]
fn effective_warps_also_remove_a_stop_with_a_lost_response_without_losing_footer_checks() {
    for outcome in [
        json!({"status":"protocol_failed","error":{"code":"bad","message":"bad"}}),
        json!({"status":"io_failed","error":{"message":"lost"}}),
        json!({"status":"unanswered"}),
    ] {
        let entries = [
            entry(
                warp(100),
                successful(json!({"requested_ticks":100,"executed_ticks":1,"outcome":"stopped"})),
            ),
            entry(stop(), outcome.clone()),
        ];
        assert_eq!(
            parse(&source(&entries, "session_ended")).unwrap(),
            [warp(1)]
        );
        if outcome["status"] == "unanswered" {
            assert!(parse(&source(&entries, "stopped")).is_err());
        }
    }
}

#[test]
fn arbitrary_inspect_values_reject_nested_duplicate_keys_and_preserve_unicode() {
    let text = format!(
        "{HEADER}\n{}\n{}",
        r#"{"type":"command","command":"inspect.query","arguments":{"source":"resources","selector":{"kind":"all"},"projection":{"kind":"value"}},"outcome":{"status":"completed","output":{"items":[{"type_path":"test::Resource","result":{"kind":"value","value":{"status":"readable","value":{"nested":[{"text":"Grüße 🦜","text":"東京"}]}}}}]}}}"#,
        r#"{"type":"recording_ended","outcome":"stopped","recorded_commands":1}"#
    );
    let error = parse(&text).unwrap_err();
    assert_eq!(error.code(), "invalid_recording");
    assert!(error.message().contains("duplicate field text"));
    let command = crate::command::input::text::Input {
        text: "Grüße 🦜\n東京".into(),
    }
    .into();
    let entries = [entry(command, successful(Value::Null))];
    let plan = parse(&source(&entries, "stopped")).unwrap();
    assert_eq!(
        serde_json::to_value(&plan[0]).unwrap()["arguments"]["text"],
        "Grüße 🦜\n東京"
    );
}

fn directory() -> (std::path::PathBuf, Dir) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "target/replay-tests/{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    let root = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
    (path, root)
}

#[test]
fn loader_opens_only_regular_files_without_following_any_symlink() {
    let (_path, root) = directory();
    root.write("ok.jsonl", format!("{HEADER}\n{FOOTER}"))
        .unwrap();
    let cancel = AtomicBool::new(false);
    assert!(
        loader::load(&root, "ok.jsonl", &cancel)
            .unwrap()
            .unwrap()
            .is_empty()
    );
    for path in [
        "",
        "/ok.jsonl",
        "./ok.jsonl",
        "../ok.jsonl",
        "a//ok.jsonl",
        "a/../ok.jsonl",
        "a\\b.jsonl",
        "C:a.jsonl",
        "a.png",
        "a\0.jsonl",
    ] {
        assert_eq!(
            loader::load(&root, path, &cancel).unwrap_err().code(),
            "invalid_recording_path",
            "{path}"
        );
    }
    root.create_dir("directory.jsonl").unwrap();
    assert_eq!(
        loader::load(&root, "directory.jsonl", &cancel)
            .unwrap_err()
            .code(),
        "invalid_recording_path"
    );
    assert_eq!(
        loader::load(&root, "missing/file.jsonl", &cancel)
            .unwrap_err()
            .code(),
        "io"
    );
    assert!(!root.exists("missing"));
    #[cfg(unix)]
    {
        root.symlink("ok.jsonl", "link.jsonl").unwrap();
        root.symlink("absent.jsonl", "dangling.jsonl").unwrap();
        root.symlink(".", "parent").unwrap();
        std::os::unix::fs::symlink("/tmp", _path.join("outside")).unwrap();
        for path in [
            "link.jsonl",
            "dangling.jsonl",
            "parent/ok.jsonl",
            "outside/ok.jsonl",
        ] {
            assert_eq!(
                loader::load(&root, path, &cancel).unwrap_err().code(),
                "invalid_recording_path",
                "{path}"
            );
        }
        let fifo = std::ffi::CString::new(_path.join("pipe.jsonl").as_os_str().as_encoded_bytes())
            .unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert_eq!(
            loader::load(&root, "pipe.jsonl", &cancel)
                .unwrap_err()
                .code(),
            "invalid_recording_path"
        );
        let file = super::super::recording::file::open_read(&root, "ok.jsonl").unwrap();
        root.rename("ok.jsonl", &root, "original.jsonl").unwrap();
        root.symlink("original.jsonl", "ok.jsonl").unwrap();
        assert!(
            loader::parse(std::io::BufReader::new(file), "ok.jsonl", &cancel)
                .unwrap()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            loader::load(&root, "ok.jsonl", &cancel).unwrap_err().code(),
            "invalid_recording_path"
        );
    }
}

#[test]
fn read_failures_include_path_and_line_and_cancellation_is_acknowledged() {
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected"))
        }
    }
    let error = loader::parse(
        std::io::BufReader::new(Broken),
        "a.jsonl",
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert_eq!(error.code(), "io");
    assert!(error.message().contains("a.jsonl: line 1"));
    assert!(
        loader::parse(Cursor::new("invalid"), "a.jsonl", &AtomicBool::new(true))
            .unwrap()
            .is_none()
    );
}

fn running(plan: Vec<Command>) -> Replay {
    let mut replay = Replay::new(Arc::new(directory().1));
    replay.run = Some(Run {
        start: 1,
        phase: Phase::Running(plan.into()),
        stopping: false,
        stops: vec![],
        pending: BTreeMap::new(),
        warp: None,
        stop_sent: false,
        blocked: None,
    });
    replay
}

fn submit(replay: &mut Replay, id: u64) -> Command {
    let command = replay.next(false).unwrap();
    replay.submitted(id, &command);
    command
}

fn completion(notices: &[(u64, Outcome)]) -> replay::Outcome {
    serde_json::from_value::<replay::Completion>(
        notices
            .iter()
            .find(|(id, _)| *id == 1)
            .unwrap()
            .1
            .clone()
            .unwrap(),
    )
    .unwrap()
    .outcome
}

#[test]
fn execution_barriers_preserve_order_and_business_rejections_do_not_block() {
    let mut replay = running(vec![stop(), stop(), warp(1), stop()]);
    assert!(replay.next(true).is_none());
    assert_eq!(submit(&mut replay, 2), stop());
    assert_eq!(submit(&mut replay, 3), stop());
    assert!(replay.next(false).is_none());
    replay.terminal(3, &Err(Error::new("business", "rejected")));
    assert!(replay.next(false).is_none());
    replay.terminal(2, &Ok(json!({"was_running":false})));
    assert_eq!(submit(&mut replay, 4), warp(1));
    assert!(replay.next(false).is_none());
    replay.terminal(4, &Ok(json!({})));
    assert_eq!(submit(&mut replay, 5), stop());
    assert!(replay.poll(false).is_empty());
    replay.terminal(5, &Ok(json!({})));
    assert_eq!(
        completion(&replay.poll(false)),
        replay::Outcome::Completed {}
    );
    assert!(!replay.busy());
}

#[test]
fn stop_waits_for_warp_and_all_stop_responses_in_either_order() {
    for warp_first in [false, true] {
        let mut replay = running(vec![warp(100), stop()]);
        submit(&mut replay, 2);
        replay.control(3, &replay::Stop {}.into());
        replay.control(4, &replay::Stop {}.into());
        assert_eq!(submit(&mut replay, 5), stop());
        assert!(replay.next(false).is_none());
        let ids = if warp_first { [2, 5] } else { [5, 2] };
        replay.terminal(ids[0], &Ok(json!({})));
        assert!(replay.poll(false).is_empty());
        replay.terminal(ids[1], &Ok(json!({})));
        let notices = replay.poll(false);
        assert_eq!(completion(&notices), replay::Outcome::Stopped {});
        assert_eq!(notices.len(), 3);
        for (_, result) in &notices[1..] {
            assert_eq!(result.as_ref().unwrap()["was_running"], true);
        }
        replay.control(6, &replay::Stop {}.into());
        assert_eq!(
            replay.poll(false)[0].1.as_ref().unwrap()["was_running"],
            false
        );
    }
}

#[test]
fn preparing_stop_waits_for_loader_ack_and_prior_load_errors_win() {
    for stop_first in [false, true] {
        let mut replay = running(vec![]);
        let (reply, result) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        replay.run.as_mut().unwrap().phase = Phase::Preparing {
            cancel: cancel.clone(),
            result,
        };
        if stop_first {
            replay.control(2, &replay::Stop {}.into());
            assert!(cancel.load(Ordering::Acquire));
            assert!(replay.poll(false).is_empty());
            assert!(replay.busy());
        }
        reply
            .send(Err(Error::new("invalid_recording", "bad file")))
            .unwrap();
        let notices = replay.poll(false);
        if stop_first {
            assert_eq!(completion(&notices), replay::Outcome::Stopped {});
        } else {
            assert_eq!(
                notices[0].1.as_ref().unwrap_err().code(),
                "invalid_recording"
            );
            replay.control(2, &replay::Stop {}.into());
            assert_eq!(
                replay.poll(false)[0].1.as_ref().unwrap()["was_running"],
                false
            );
        }
        assert!(!replay.busy());
    }
}

#[test]
fn file_barriers_hold_preparing_and_all_active_phases_reject_another_start() {
    let mut replay = Replay::new(Arc::new(directory().1));
    let start = Command::from(replay::Start {
        path: "missing.jsonl".into(),
    });
    replay.control(1, &start);
    assert!(replay.poll(true).is_empty());
    assert!(replay.next(false).is_none());
    replay.control(2, &start);
    assert_eq!(
        replay.poll(true)[0].1.as_ref().unwrap_err().code(),
        "replay_already_running"
    );
    replay.control(3, &replay::Stop {}.into());
    replay.control(4, &start);
    let notices = replay.poll(true);
    assert_eq!(
        notices[0].1.as_ref().unwrap_err().code(),
        "replay_already_running"
    );
    assert_eq!(completion(&notices), replay::Outcome::Stopped {});
    assert!(!replay.busy());
    let mut replay = running(vec![stop()]);
    replay.control(2, &start);
    assert_eq!(
        replay.poll(false)[0].1.as_ref().unwrap_err().code(),
        "replay_already_running"
    );
}

#[test]
fn stop_drains_non_tick_work_without_issuing_a_warp_stop() {
    let mut replay = running(vec![stop(), stop(), warp(1)]);
    submit(&mut replay, 2);
    submit(&mut replay, 3);
    replay.control(4, &replay::Stop {}.into());
    assert!(replay.next(false).is_none());
    replay.terminal(3, &Ok(json!({})));
    assert!(replay.poll(false).is_empty());
    replay.terminal(2, &Err(Error::new("business", "rejected")));
    assert_eq!(completion(&replay.poll(false)), replay::Outcome::Stopped {});
}

#[test]
fn technical_errors_block_and_waiting_stops_get_the_cause() {
    for (error, code) in [
        (
            Error::Protocol {
                code: "bad".into(),
                message: "bad".into(),
            },
            "command_protocol_failed",
        ),
        (Error::io("disk"), "session_io_failed"),
        (Error::Ended, "session_ended"),
    ] {
        let mut replay = running(vec![stop(), stop(), warp(1)]);
        submit(&mut replay, 2);
        submit(&mut replay, 3);
        replay.control(4, &replay::Stop {}.into());
        replay.terminal(2, &Err(error.clone()));
        assert!(replay.next(false).is_none());
        assert!(replay.poll(false).is_empty());
        replay.terminal(3, &Ok(json!({})));
        let notices = replay.poll(false);
        assert!(
            matches!(completion(&notices), replay::Outcome::Blocked { code: actual, .. } if actual == code)
        );
        assert_eq!(notices[1].1.as_ref().unwrap_err().code(), error.code());
    }
    let mut replay = running(vec![warp(1)]);
    replay.submission_failed(Error::RequestIdExhausted);
    assert!(
        matches!(completion(&replay.poll(false)), replay::Outcome::Blocked { code, .. } if code == "request_id_exhausted")
    );
    let mut replay = running(vec![warp(100)]);
    submit(&mut replay, 2);
    replay.control(3, &replay::Stop {}.into());
    submit(&mut replay, 4);
    replay.terminal(4, &Err(Error::new("not_stoppable", "fixture")));
    let notices = replay.poll(false);
    assert!(
        matches!(completion(&notices), replay::Outcome::Blocked { code, .. } if code == "stop_failed")
    );
    assert_eq!(notices[1].1.as_ref().unwrap_err().code(), "not_stoppable");
}
