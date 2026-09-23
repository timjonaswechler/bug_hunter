use super::*;
use crate::{
    client::Management,
    command::{Empty, inspect, recording, tick::warp},
    server::protocol::{
        Activity, Cursor, Lifecycle, Operation, Request, Response, Result as Reply, Snapshot,
    },
};
use serde_json::json;
use std::{
    net::{TcpListener, TcpStream},
    time::Duration,
};
use tungstenite::{Message, WebSocket};

fn inspect() -> Command {
    inspect::Command::Resources {
        selector: inspect::Selector::All {},
        projection: inspect::Projection::Metadata {},
    }
    .into()
}
fn warp(ticks: u64) -> Command {
    warp::Start { ticks, pace: None }.into()
}

#[test]
fn whole_document_and_programmatic_lists_have_the_same_validation() {
    for source in [
        r#"{"version":2,"commands":[]}"#,
        r#"{"version":1,"commands":[],"session":"aaaaaaaa"}"#,
        r#"{"version":1,"version":1,"commands":[]}"#,
        r#"{"version":1,"commands":[],"commands":[]}"#,
        r#"{"version":1,"commands":null}"#,
        r#"{"commands":[]}"#,
        r#"{"version":1,"commands":[]} trailing"#,
    ] {
        assert!(
            matches!(
                Script::parse(source),
                Err(Error::InvalidScript {
                    command_index: None,
                    ..
                })
            ),
            "{source}"
        );
    }
    for invalid in [
        json!({"command":"unknown","arguments":{}}).to_string(),
        r#"{"command":"tick.warp.start","arguments":{"ticks":1,"ticks":2}}"#.into(),
        json!({"command":"tick.warp.start","arguments":{"ticks":-1}}).to_string(),
        json!({"command":"tick.warp.start","arguments":{"ticks":1,"extra":0}}).to_string(),
        json!({"command":"shutdown","arguments":{"extra":0}}).to_string(),
        r#"{"command":"input.pointer.move_by","arguments":{"delta":[1e39,0]}}"#.into(),
    ] {
        let source = format!(
            r#"{{"version":1,"commands":[{},{}]}}"#,
            serde_json::to_string(&inspect()).unwrap(),
            invalid
        );
        assert!(
            matches!(
                Script::parse(&source),
                Err(Error::InvalidScript {
                    command_index: Some(1),
                    ..
                })
            ),
            "{source}"
        );
    }
    let invalid = vec![Command::Shutdown(Empty {}), inspect()];
    assert!(matches!(
        Script::new(invalid.clone()),
        Err(Error::InvalidScript {
            command_index: Some(0),
            ..
        })
    ));
    assert!(Script::parse(&json!({"version":1,"commands":invalid}).to_string()).is_err());
    assert!(
        Script::new(vec![Command::SetPace(warp::SetPace {
            pace: warp::Pace::TicksPerSecond { target: f32::NAN },
        })])
        .is_err()
    );
    assert!(
        Script::parse(r#"{"version":1,"commands":[]}"#)
            .unwrap()
            .commands()
            .is_empty()
    );
    // Semantic errors belong to the game, not the document parser.
    let commands = vec![
        Command::SetPace(warp::SetPace {
            pace: warp::Pace::TicksPerSecond { target: -1.0 },
        }),
        Command::Shutdown(Empty {}),
    ];
    assert_eq!(Script::new(commands.clone()).unwrap().commands(), commands);
}

struct Peer {
    socket: WebSocket<TcpStream>,
    position: u64,
}
impl Peer {
    fn read(&mut self) -> Request {
        let Message::Text(text) = self.socket.read().unwrap() else {
            panic!("expected text")
        };
        serde_json::from_str(&text).unwrap()
    }
    fn send(&mut self, call: u64, result: Reply) {
        self.socket
            .send(Message::Text(
                serde_json::to_string(&Response {
                    version: 1,
                    call,
                    result,
                })
                .unwrap()
                .into(),
            ))
            .unwrap();
    }
    fn cursor(&self) -> Cursor {
        Cursor {
            session_id: "a".repeat(32),
            position: self.position,
        }
    }
    fn snapshot(&mut self) {
        let request = self.read();
        assert!(matches!(request.operation, Operation::Snapshot {}));
        self.send(
            request.call,
            Reply::Snapshot {
                snapshot: Snapshot {
                    cursor: self.cursor(),
                    state: Lifecycle::Ready,
                    pending: vec![],
                },
            },
        );
    }
    fn submit(&mut self, name: &str, id: u64) {
        let request = self.read();
        assert!(
            matches!(&request.operation, Operation::Submit { command } if command.name() == name)
        );
        self.send(
            request.call,
            Reply::Pending {
                request_id: id,
                command: name.into(),
            },
        );
    }
    fn poll(&mut self, wait: bool, events: Vec<Value>) {
        let request = self.read();
        let Operation::Poll { cursor, wait_ms } = request.operation else {
            panic!("expected poll, got {:?}", request.operation)
        };
        assert_eq!(cursor.unwrap(), self.cursor());
        assert_eq!(wait_ms > 0, wait);
        let entries = events
            .into_iter()
            .map(|event| {
                self.position += 1;
                Activity {
                    cursor: self.cursor(),
                    event,
                }
            })
            .collect();
        self.send(
            request.call,
            Reply::Activity {
                entries,
                cursor: self.cursor(),
            },
        );
    }
}
fn fixture(script: Script, serve: impl FnOnce(&mut Peer) + Send + 'static) -> Outcome {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut peer = Peer {
            socket: tungstenite::accept(stream).unwrap(),
            position: 100,
        };
        peer.snapshot();
        serve(&mut peer);
        // No implicit stop/shutdown and no retry after failure.
        assert!(peer.socket.read().is_err());
    });
    let mut client = Management::new(address).unwrap().bind("aaaaaaaa").unwrap();
    let outcome = run(&mut client, &script).unwrap();
    drop(client);
    worker.join().unwrap();
    outcome
}
fn completed(id: u64, name: &str, output: Value) -> Value {
    json!({"kind":"completed","request_id":id,"command":name,"output":output})
}
fn warp_output(ticks: u64) -> Value {
    json!({"requested_ticks":ticks,"executed_ticks":ticks,"outcome":"completed"})
}

#[test]
fn ordinary_commands_pipeline_and_all_three_boundaries_wait_for_prior_results() {
    let script = Script::new(vec![
        warp(100),
        inspect(),
        recording::Start {
            path: "run.jsonl".into(),
        }
        .into(),
        warp(3),
        recording::Stop {}.into(),
        Command::Shutdown(Empty {}),
    ])
    .unwrap();
    let outcome = fixture(script, |p| {
        p.submit("tick.warp.start", 11);
        p.poll(false, vec![]);
        // Inspect is submitted while Warp is still pending.
        p.submit("inspect.query", 12);
        p.poll(
            false,
            vec![
                completed(999, "inspect.query", json!({"items":[]})),
                completed(12, "inspect.query", json!({"items":[]})),
            ],
        );
        // Recording-start must not be submitted before the older Warp completes.
        p.poll(
            true,
            vec![completed(11, "tick.warp.start", warp_output(100))],
        );
        p.submit("recording.start", 13);
        p.poll(false, vec![]);
        p.submit("tick.warp.start", 14);
        p.poll(
            false,
            vec![completed(14, "tick.warp.start", warp_output(3))],
        );
        p.poll(
            true,
            vec![completed(
                13,
                "recording.start",
                json!({"path":"run.jsonl"}),
            )],
        );
        p.submit("recording.stop", 15);
        p.poll(false, vec![]);
        p.poll(
            true,
            vec![completed(
                15,
                "recording.stop",
                json!({"path":"run.jsonl","recorded_commands":1}),
            )],
        );
        p.submit("shutdown", u64::MAX);
        p.poll(false, vec![completed(u64::MAX, "shutdown", Value::Null)]);
    });
    let Outcome::Passed { completed } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(
        completed
            .iter()
            .map(|c| c.command_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4, 5]
    );
    assert_eq!(
        completed.iter().map(|c| c.request_id).collect::<Vec<_>>(),
        vec![11, 12, 13, 14, 15, u64::MAX]
    );
}

#[test]
fn rejection_and_failure_are_collected_without_skipping_later_commands() {
    let outcome = fixture(
        Script::new(vec![inspect(), warp(3), inspect()]).unwrap(),
        |p| {
            p.submit("inspect.query", 31);
            p.poll(
                false,
                vec![
                    json!({"kind":"rejected","request_id":31,"command":"inspect.query",
            "error":{"code":"test_rejection","message":"rejected"}}),
                ],
            );
            p.submit("tick.warp.start", 33);
            p.poll(
                false,
                vec![
                    json!({"kind":"failed","request_id":33,"command":"tick.warp.start",
            "error":{"code":"test_failure","message":"failed"}}),
                ],
            );
            p.submit("inspect.query", 37);
            p.poll(
                false,
                vec![completed(37, "inspect.query", json!({"items":[]}))],
            );
        },
    );
    let Outcome::Failed {
        completed,
        failures,
    } = outcome
    else {
        panic!("{outcome:?}")
    };
    assert_eq!(completed[0].command_index, 2);
    assert_eq!(failures.len(), 2);
    assert_eq!(failures[0].request_id, Some(31));
    assert!(matches!(&failures[1].reason, Reason::Failed { code, .. } if code == "test_failure"));
}

#[test]
fn activity_gap_preserves_completed_results_and_marks_remaining_positions() {
    let outcome = fixture(
        Script::new(vec![inspect(), warp(300), inspect()]).unwrap(),
        |p| {
            p.submit("inspect.query", 1);
            p.poll(
                false,
                vec![completed(1, "inspect.query", json!({"items":[]}))],
            );
            p.submit("tick.warp.start", 2);
            let request = p.read();
            assert!(matches!(request.operation, Operation::Poll { .. }));
            p.position += 10;
            p.send(
                request.call,
                Reply::Gap {
                    cursor: p.cursor(),
                    message: "evicted".into(),
                },
            );
        },
    );
    let Outcome::Failed {
        completed,
        failures,
    } = outcome
    else {
        panic!("{outcome:?}")
    };
    assert_eq!(completed.len(), 1);
    assert_eq!(failures.len(), 2);
    assert_eq!(failures[0].command_index, 1);
    assert_eq!(failures[0].request_id, Some(2));
    assert!(matches!(failures[0].reason, Reason::Unknown { .. }));
    assert_eq!(failures[1].command_index, 2);
    assert!(matches!(failures[1].reason, Reason::NotSubmitted { .. }));
}

#[test]
fn missing_submit_confirmation_is_unknown_and_never_retried() {
    let outcome = fixture(Script::new(vec![inspect(), inspect()]).unwrap(), |p| {
        let request = p.read();
        assert!(matches!(request.operation, Operation::Submit { .. }));
        p.socket.close(None).unwrap();
    });
    let Outcome::Failed { failures, .. } = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(failures.len(), 2);
    assert!(failures[0].request_id.is_none());
    assert!(matches!(failures[0].reason, Reason::Unknown { .. }));
    assert!(matches!(failures[1].reason, Reason::NotSubmitted { .. }));
}

#[test]
fn malformed_correlated_output_is_not_a_success() {
    let outcome = fixture(Script::new(vec![warp(3)]).unwrap(), |p| {
        p.submit("tick.warp.start", 1);
        p.poll(false, vec![completed(1, "tick.warp.start", warp_output(4))]);
    });
    let Outcome::Failed { failures, .. } = outcome else {
        panic!("{outcome:?}")
    };
    assert!(
        matches!(&failures[0].reason, Reason::Unknown { message } if message.contains("invalid output"))
    );
}

#[test]
fn pre_acceptance_rejection_has_no_fictitious_id_and_later_work_still_runs() {
    let outcome = fixture(Script::new(vec![inspect(), inspect()]).unwrap(), |p| {
        let request = p.read();
        assert!(matches!(request.operation, Operation::Submit { .. }));
        p.send(
            request.call,
            Reply::Error {
                code: "session_not_ready".into(),
                message: "starting".into(),
            },
        );
        p.poll(false, vec![]);
        p.submit("inspect.query", 8);
        p.poll(
            false,
            vec![completed(8, "inspect.query", json!({"items":[]}))],
        );
    });
    let Outcome::Failed {
        completed,
        failures,
    } = outcome
    else {
        panic!("{outcome:?}")
    };
    assert_eq!(completed[0].command_index, 1);
    assert_eq!(failures[0].request_id, None);
    assert!(
        matches!(&failures[0].reason, Reason::Rejected { code, .. } if code == "session_not_ready")
    );
}

#[test]
fn session_end_does_not_wait_forever_or_invent_a_command_completion() {
    let outcome = fixture(Script::new(vec![warp(3), inspect()]).unwrap(), |p| {
        p.submit("tick.warp.start", 1);
        p.poll(false, vec![json!({"kind":"lifecycle","state":"Failed","error":{"code":"ended","message":"ended"}})]);
    });
    let Outcome::Failed { failures, .. } = outcome else {
        panic!("{outcome:?}")
    };
    assert!(matches!(failures[0].reason, Reason::Unknown { .. }));
    assert!(matches!(failures[1].reason, Reason::NotSubmitted { .. }));
}
