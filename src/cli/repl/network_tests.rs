use super::network::{Event, Network, Request};
use crate::{
    command::tick::warp,
    server::protocol::{self, Cursor, Lifecycle, Operation, Response, Result as Reply, Snapshot},
};
use std::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::{Duration, Instant},
};
use tungstenite::{Message, WebSocket};

fn request(socket: &mut WebSocket<TcpStream>) -> protocol::Request {
    loop {
        if let Message::Text(text) = socket.read().unwrap() {
            return serde_json::from_str(&text).unwrap();
        }
    }
}
fn reply(socket: &mut WebSocket<TcpStream>, call: u64, result: Reply) {
    socket
        .send(Message::Text(
            serde_json::to_string(&Response {
                version: protocol::VERSION,
                call,
                result,
            })
            .unwrap()
            .into(),
        ))
        .unwrap();
}
fn snapshot(socket: &mut WebSocket<TcpStream>, position: u64) {
    let req = request(socket);
    assert!(matches!(req.operation, Operation::Snapshot {}));
    reply(
        socket,
        req.call,
        Reply::Snapshot {
            snapshot: Snapshot {
                cursor: Cursor {
                    session_id: "a".repeat(32),
                    position,
                },
                state: Lifecycle::Ready,
                pending: vec![],
            },
        },
    );
}
fn until(network: &Network, mut test: impl FnMut(Event) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let event = network
            .events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        if test(event) {
            return;
        }
    }
}

#[test]
fn reconnect_keeps_cursor_reports_gap_and_never_repeats_uncertain_submit() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (done_tx, done) = mpsc::channel();
    let peer = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        snapshot(&mut socket, 10);
        loop {
            let req = request(&mut socket);
            match req.operation {
                Operation::Poll { cursor, .. } => {
                    assert_eq!(cursor.as_ref().unwrap().position, 10);
                    reply(
                        &mut socket,
                        req.call,
                        Reply::Activity {
                            entries: vec![],
                            cursor: cursor.unwrap(),
                        },
                    );
                }
                Operation::Snapshot {} => reply(
                    &mut socket,
                    req.call,
                    Reply::Snapshot {
                        snapshot: Snapshot {
                            cursor: Cursor {
                                session_id: "a".repeat(32),
                                position: 10,
                            },
                            state: Lifecycle::Ready,
                            pending: vec![],
                        },
                    },
                ),
                Operation::Submit { command } => {
                    assert_eq!(command.name(), "tick.warp.start");
                    // Accepted remotely, confirmation lost.
                    break;
                }
            }
        }
        drop(socket);
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        snapshot(&mut socket, 20);
        let req = request(&mut socket);
        assert!(matches!(
            req.operation,
            Operation::Poll {
                cursor: Some(Cursor { position: 10, .. }),
                ..
            }
        ));
        reply(
            &mut socket,
            req.call,
            Reply::Gap {
                cursor: Cursor {
                    session_id: "a".repeat(32),
                    position: 15,
                },
                message: "evicted".into(),
            },
        );
        snapshot(&mut socket, 20);
        let req = request(&mut socket);
        assert!(matches!(
            req.operation,
            Operation::Poll {
                cursor: Some(Cursor { position: 15, .. }),
                ..
            }
        ));
        reply(
            &mut socket,
            req.call,
            Reply::Activity {
                entries: vec![],
                cursor: Cursor {
                    session_id: "a".repeat(32),
                    position: 20,
                },
            },
        );
        done_tx.send(()).unwrap();
        // Worker Drop must interrupt this read without sending stop/shutdown.
        while let Ok(Message::Text(text)) = socket.read() {
            let req: protocol::Request = serde_json::from_str(&text).unwrap();
            assert!(matches!(req.operation, Operation::Poll { .. }));
        }
    });
    let network = Network::start(address, "aaaaaaaa".into()).unwrap();
    until(&network, |event| matches!(event, Event::Connected(_)));
    network
        .requests
        .send(Request::Submit(
            warp::Start {
                ticks: 1000,
                pace: None,
            }
            .into(),
        ))
        .unwrap();
    until(
        &network,
        |event| matches!(event, Event::Notice(s) if s.contains("outcome unknown")),
    );
    until(&network, |event| {
        matches!(event, Event::Reply(Reply::Gap { .. }))
    });
    done.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(network);
    peer.join().unwrap();
}

#[test]
fn closing_client_interrupts_a_blocked_handshake() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (connected_tx, connected) = mpsc::channel();
    let peer = std::thread::spawn(move || {
        use std::io::Read;
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        connected_tx.send(()).unwrap();
        let mut bytes = vec![];
        stream.read_to_end(&mut bytes).unwrap();
    });
    let network = Network::start(address, "a".repeat(32)).unwrap();
    connected.recv_timeout(Duration::from_secs(5)).unwrap();
    let start = Instant::now();
    drop(network);
    assert!(start.elapsed() < Duration::from_secs(1));
    peer.join().unwrap();
}

#[test]
fn closing_client_interrupts_a_blocked_activity_response() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (blocked_tx, blocked) = mpsc::channel();
    let peer = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        snapshot(&mut socket, 0);
        assert!(matches!(
            request(&mut socket).operation,
            Operation::Poll { .. }
        ));
        blocked_tx.send(()).unwrap();
        assert!(socket.read().is_err());
    });
    let network = Network::start(address, "a".repeat(32)).unwrap();
    blocked.recv_timeout(Duration::from_secs(5)).unwrap();
    let start = Instant::now();
    drop(network);
    assert!(start.elapsed() < Duration::from_secs(1));
    peer.join().unwrap();
}
