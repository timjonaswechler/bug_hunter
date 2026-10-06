use super::*;
use cap_std::fs::File;
use serde_json::json;
use std::{
    io::{self, Write},
    path::PathBuf,
    sync::atomic::AtomicU64,
    time::Instant,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "recording-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn dir(&self) -> Dir {
        Dir::open_ambient_dir(&self.0, cap_std::ambient_authority()).unwrap()
    }
    fn lines(&self, path: &str) -> Vec<serde_json::Value> {
        std::fs::read_to_string(self.0.join(path))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

struct Fault {
    file: File,
    writes: usize,
    fail_at: usize,
    fail_sync: bool,
}
impl Write for Fault {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.writes >= self.fail_at {
            return Err(io::Error::other("injected write failure"));
        }
        self.file.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
impl file::Sink for Fault {
    fn sync_all(&self) -> io::Result<()> {
        if self.fail_sync {
            Err(io::Error::other("injected sync failure"))
        } else {
            self.file.sync_all()
        }
    }
}

fn completion(recorder: &mut Recorder, id: u64) -> Outcome {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let notices = recorder.poll();
        assert!(notices.len() <= 1);
        if let Some(notice) = notices.into_iter().next() {
            match notice {
                Notice::Complete {
                    id: actual,
                    outcome,
                } => {
                    assert_eq!(actual, id);
                    return outcome;
                }
                Notice::Event(event) => panic!("unexpected event: {event:?}"),
            }
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}
fn start(path: &str) -> Command {
    recording::Start { path: path.into() }.into()
}
fn stop() -> Command {
    recording::Stop {}.into()
}

#[test]
fn state_transitions_require_file_acknowledgements() {
    let (operations, input) = mpsc::channel();
    let (output, replies) = mpsc::channel();
    let mut recorder = Recorder {
        state: State::Idle,
        path: String::new(),
        entries: VecDeque::new(),
        operations,
        replies,
    };
    recorder.control(1, &start("a.jsonl"), false).unwrap();
    assert!(matches!(input.recv().unwrap(), worker::Operation::Start(_)));
    assert!(recorder.transitioning());
    assert!(recorder.poll().is_empty());
    assert_eq!(
        recorder.control(2, &stop(), false).unwrap_err().code(),
        "recording_transition_in_progress"
    );
    output.send(worker::Reply::Started(Ok(()))).unwrap();
    assert_eq!(
        completion(&mut recorder, 1).unwrap(),
        json!({"path":"a.jsonl"})
    );
    assert!(recorder.busy() && !recorder.transitioning());
    recorder.control(3, &stop(), false).unwrap();
    assert!(matches!(input.recv().unwrap(), worker::Operation::Stop));
    assert!(recorder.transitioning());
    assert_eq!(
        recorder
            .control(4, &start("b.jsonl"), false)
            .unwrap_err()
            .code(),
        "recording_transition_in_progress"
    );
    output.send(worker::Reply::Stopped(Ok(0))).unwrap();
    assert_eq!(
        completion(&mut recorder, 3).unwrap(),
        json!({"path":"a.jsonl","recorded_commands":0})
    );
    assert!(!recorder.busy());
}

#[test]
fn paths_reject_traversal_symlinks_and_existing_targets() {
    let root = Root::new();
    let dir = root.dir();
    for path in [
        "",
        "/a.jsonl",
        "a//b.jsonl",
        "./a.jsonl",
        "a/./b.jsonl",
        "../a.jsonl",
        "a\\b.jsonl",
        "C:/a.jsonl",
        "a.JSONL",
        "a.png",
        "a\0.jsonl",
    ] {
        assert_eq!(
            file::Recording::start(&dir, path.into())
                .err()
                .unwrap()
                .code(),
            "invalid_recording_path"
        );
    }
    file::Recording::start(&dir, "nested/ä.jsonl".into())
        .unwrap()
        .finish("stopped")
        .unwrap();
    assert_eq!(
        file::Recording::start(&dir, "nested/ä.jsonl".into())
            .err()
            .unwrap()
            .code(),
        "recording_path_exists"
    );
    dir.create_dir("directory.jsonl").unwrap();
    assert_eq!(
        file::Recording::start(&dir, "directory.jsonl".into())
            .err()
            .unwrap()
            .code(),
        "invalid_recording_path"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        for (target, link, path) in [
            ("nested", "inside", "inside/a.jsonl"),
            ("../outside", "escape", "escape/a.jsonl"),
            ("nested/ä.jsonl", "link.jsonl", "link.jsonl"),
            ("missing", "dangling.jsonl", "dangling.jsonl"),
        ] {
            symlink(target, root.0.join(link)).unwrap();
            assert_eq!(
                file::Recording::start(&dir, path.into())
                    .err()
                    .unwrap()
                    .code(),
                "invalid_recording_path",
                "{path}"
            );
        }
    }
}

#[test]
fn header_and_footer_failures_are_io_and_header_failure_removes_the_new_file() {
    let root = Root::new();
    let dir = root.dir();
    let result = file::Recording::start_with(&dir, "header.jsonl".into(), |file| Fault {
        file,
        writes: 0,
        fail_at: 1,
        fail_sync: false,
    });
    assert!(matches!(result, Err(Error::Io { .. })));
    assert!(!dir.exists("header.jsonl"));
    for (path, fail_at, fail_sync) in [("footer.jsonl", 2, false), ("sync.jsonl", usize::MAX, true)]
    {
        let recording = file::Recording::start_with(&dir, path.into(), |file| Fault {
            file,
            writes: 0,
            fail_at,
            fail_sync,
        })
        .unwrap();
        assert!(matches!(recording.finish("stopped"), Err(Error::Io { .. })));
        assert!(dir.exists(path));
    }
}

#[test]
fn active_write_failure_emits_only_event_and_returns_idle() {
    let root = Root::new();
    let dir = root.dir();
    let (operations, input) = mpsc::channel();
    let (output, replies) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        worker::run(dir, input, output, |root, path| {
            file::Recording::start_with(root, path, |file| Fault {
                file,
                writes: 0,
                fail_at: 2,
                fail_sync: false,
            })
        })
    });
    let mut recorder = Recorder {
        state: State::Idle,
        path: String::new(),
        entries: VecDeque::new(),
        operations,
        replies,
    };
    recorder.control(1, &start("failure.jsonl"), false).unwrap();
    completion(&mut recorder, 1).unwrap();
    let command = crate::command::tick::warp::Stop {}.into();
    let outcome = Ok(json!({"was_running": false}));
    recorder.record(2, &command);
    recorder.terminal(2, &outcome);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let notices = recorder.poll();
        if !notices.is_empty() {
            assert!(
                matches!(&notices[..], [Notice::Event(Event::RecordingFailed { path, .. })] if path == "failure.jsonl")
            );
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(!recorder.busy());
    assert_eq!(
        root.lines("failure.jsonl"),
        vec![json!({"type":"recording_started","format_version":1})]
    );
    recorder.control(3, &start("restart.jsonl"), false).unwrap();
    completion(&mut recorder, 3).unwrap();
    recorder.control(4, &stop(), false).unwrap();
    assert!(matches!(
        completion(&mut recorder, 4),
        Err(Error::Io { .. })
    ));
    assert!(!recorder.busy());
    recorder.end(&AtomicBool::new(false));
    worker.join().unwrap();
}

#[test]
fn end_failure_is_reported_before_end_acknowledgement() {
    let root = Root::new();
    let dir = root.dir();
    let (operations, input) = mpsc::channel();
    let (output, replies) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        worker::run(dir, input, output, |root, path| {
            file::Recording::start_with(root, path, |file| Fault {
                file,
                writes: 0,
                fail_at: 2,
                fail_sync: false,
            })
        })
    });
    let mut recorder = Recorder {
        state: State::Idle,
        path: String::new(),
        entries: VecDeque::new(),
        operations,
        replies,
    };
    recorder.control(1, &start("ended.jsonl"), false).unwrap();
    completion(&mut recorder, 1).unwrap();
    let notices = recorder.end(&AtomicBool::new(false));
    assert!(
        matches!(&notices[..], [Notice::Event(Event::RecordingFailed { path, .. })] if path == "ended.jsonl")
    );
    worker.join().unwrap();
}

#[test]
fn persisted_outcome_shapes_keep_unicode_and_distinguish_technical_errors() {
    let command = crate::command::input::text::Input::new("Grüße 🦜").into();
    for (outcome, expected) in [
        (Ok(json!(null)), json!({"status":"completed","output":null})),
        (
            Err(Error::new("text_focus_unavailable", "focus")),
            json!({"status":"rejected","error":{"code":"text_focus_unavailable","message":"focus"}}),
        ),
        (
            Err(Error::new("invalid_response", "response")),
            json!({"status":"protocol_failed","error":{"code":"invalid_response","message":"response"}}),
        ),
        (
            Err(Error::io("disk")),
            json!({"status":"io_failed","error":{"message":"disk"}}),
        ),
        (Err(Error::ended()), json!({"status":"unanswered"})),
    ] {
        assert_eq!(
            file::entry(&command, Some(&outcome)),
            json!({
                "type":"command","command":"input.text.input","arguments":{"text":"Grüße 🦜"},"outcome":expected
            })
        );
    }
}
