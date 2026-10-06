#![cfg(feature = "client")]
//! Real reflection, process, server and WebSocket clients at the 4 MiB retention limit.
use serde_json::{Value, json};
use std::{
    net::TcpStream,
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use woodpecker::{
    client::{Client, Management},
    server::{
        self,
        protocol::{Cursor, Lifecycle, Result as Reply},
    },
};

const MIB: usize = 1024 * 1024;

struct Host {
    management: Management,
    address: std::net::SocketAddr,
    finished: Option<mpsc::Receiver<Result<(), woodpecker::session::Error>>>,
}
impl Host {
    fn start(root: PathBuf) -> Self {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let result = runtime.block_on(async {
                let server = server::Server::bind(server::Config {
                    address: "127.0.0.1:0".parse().unwrap(),
                    artifact_dir: root,
                    shutdown_timeout: Duration::from_secs(10),
                    activity_bytes: 4 * MIB,
                })
                .await
                .unwrap();
                ready_tx.send(server.address().unwrap()).unwrap();
                server.run().await
            });
            let _ = done_tx.send(result);
        });
        let address = ready_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        Self {
            management: Management::new(address).unwrap(),
            address,
            finished: Some(done_rx),
        }
    }

    fn stop(&mut self) -> Result<(), String> {
        let Some(finished) = self.finished.take() else {
            return Ok(());
        };
        // Wait for the server's terminal result even if the HTTP response was lost.
        let _ = self.management.request("POST", "/stop", None);
        finished
            .recv_timeout(Duration::from_secs(20))
            .map_err(|error| format!("server cleanup: {error}"))?
            .map_err(|error| format!("server shutdown: {error}"))
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        let result = self.stop();
        if !thread::panicking() {
            result.expect("server cleanup failed");
        }
    }
}
fn wait(mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !f() {
        assert!(Instant::now() < deadline, "condition timed out");
        thread::sleep(Duration::from_millis(5));
    }
}
fn submit(client: &mut Client, command: &str, arguments: Value) -> u64 {
    let command = serde_json::from_value(json!({"command":command,"arguments":arguments})).unwrap();
    let Reply::Pending { request_id, .. } = client.submit(command).unwrap() else {
        panic!("not accepted")
    };
    request_id
}
fn query(name: &str) -> Value {
    json!({"source":"resources","selector":{"kind":"type","type_path":format!("activity_fixture::{name}")},"projection":{"kind":"value"}})
}
fn settled(client: &mut Client) {
    wait(|| client.snapshot().unwrap().pending.is_empty());
}
fn activity(client: &mut Client, cursor: Cursor) -> (Vec<Value>, Cursor) {
    let Reply::Activity { entries, cursor } = client.poll(Some(cursor), 0).unwrap() else {
        panic!("unexpected gap")
    };
    (
        entries.into_iter().map(|entry| entry.event).collect(),
        cursor,
    )
}
fn completed(events: &[Value], id: u64) -> &Value {
    let event = events
        .iter()
        .find(|e| e["request_id"] == id && e["kind"] == "completed")
        .expect("missing outcome");
    &event["output"]
}
fn gap(client: &mut Client, cursor: Cursor) -> Cursor {
    let Reply::Gap { cursor, message } = client.poll(Some(cursor), 0).unwrap() else {
        panic!("expected visible gap, not a truncated or invented result")
    };
    assert!(message.contains("unknown"), "{message}");
    cursor
}

#[test]
fn large_reflected_values_reports_and_nonreading_clients_preserve_unknown_outcomes() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = repo.join(format!("target/activity-load-{}", std::process::id()));
    assert!(!root.exists(), "preserve prior evidence: {root:?}");
    let mut host = Host::start(root.clone());
    let detail = host.management.request("POST", "/sessions", Some(&json!({
        "launch":{"manifest_path":repo.join("Cargo.toml"),"package":"woodpecker",
            "target":{"kind":"example","name":"activity_fixture"},"features":[],"arguments":[]},
        "tick":{"pace":{"kind":"as_fast_as_possible"}},
        "report":{"tracing_errors":true,"output":"reports","provider":{"kind":"local"}}
    }))).unwrap();
    let id = detail["id"].as_str().unwrap();
    wait(|| {
        host.management
            .request("GET", &format!("/sessions/{id}"), None)
            .unwrap()["state"]
            == "Ready"
    });
    let artifact = root.join(id);
    let mut fast = host.management.bind(id).unwrap();
    let mut slow = host.management.bind(id).unwrap();
    let cursor = fast.snapshot().unwrap().cursor;
    let counter = submit(&mut fast, "inspect.query", query("Counter"));
    settled(&mut fast);
    let (events, _) = activity(&mut fast, cursor);
    let pid = completed(&events, counter)["items"][0]["result"]["value"]["value"]["process_id"]
        .as_i64()
        .unwrap() as i32;

    // A single real reflected result exceeds retention. Snapshot confirms only
    // that work is no longer pending, never that its lost outcome succeeded.
    submit(&mut fast, "recording.start", json!({"path":"large.jsonl"}));
    settled(&mut fast);
    let old = slow.snapshot().unwrap().cursor;
    let oversized = submit(&mut fast, "inspect.query", query("Large"));
    settled(&mut fast);
    let recovered = gap(&mut slow, old);
    let (remaining, _) = activity(&mut slow, recovered);
    assert!(!remaining.iter().any(|e| e["request_id"] == oversized));
    let stop = submit(&mut fast, "recording.stop", json!({}));
    settled(&mut fast);
    let lines: Vec<Value> = std::fs::read_to_string(artifact.join("large.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let text = lines[1]["outcome"]["output"]["items"][0]["result"]["value"]["value"]["text"]
        .as_str()
        .unwrap();
    assert_eq!(text.len(), 5 * MIB);
    assert!(text.bytes().all(|b| b == b'x'));
    assert_eq!(lines.last().unwrap()["recorded_commands"], 1);
    assert!(stop > oversized);

    // The slow client remains connected but deliberately does not poll while
    // another client obtains full, individually retainable responses.
    let lagged = slow.snapshot().unwrap().cursor;
    let mut fast_cursor = fast.snapshot().unwrap().cursor;
    let mut medium_ids = Vec::new();
    for _ in 0..3 {
        let request = submit(&mut fast, "inspect.query", query("Medium"));
        settled(&mut fast);
        let (events, next) = activity(&mut fast, fast_cursor);
        fast_cursor = next;
        let value = completed(&events, request)["items"][0]["result"]["value"]["value"]["text"]
            .as_str()
            .unwrap();
        assert_eq!(value.len(), 2 * MIB);
        assert!(value.bytes().all(|b| b == b'm'));
        medium_ids.push(request);
    }
    let recovered = gap(&mut slow, lagged);
    let (retained, _) = activity(&mut slow, recovered.clone());
    assert!(!retained.iter().any(|e| e["request_id"] == medium_ids[0]));
    assert_eq!(
        completed(&retained, medium_ids[2])["items"][0]["result"]["value"]["value"]["text"]
            .as_str()
            .unwrap()
            .len(),
        2 * MIB
    );

    // Request the retained multi-megabyte response, then do not read that socket.
    // A separate client must still be able to inspect the live session.
    let stream = TcpStream::connect(host.address).unwrap();
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let bytes: libc::c_int = 4096;
        assert_eq!(
            unsafe {
                libc::setsockopt(
                    stream.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_RCVBUF,
                    (&bytes as *const libc::c_int).cast(),
                    std::mem::size_of_val(&bytes) as libc::socklen_t,
                )
            },
            0
        );
    }
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let (mut blocked, _) = tungstenite::client(
        format!("ws://{}/v1/sessions/{id}/connect", host.address),
        stream,
    )
    .unwrap();
    blocked
        .send(tungstenite::Message::Text(
            json!({"version":1,"call":1,
        "operation":{"kind":"poll","cursor":recovered,"wait_ms":0}})
            .to_string()
            .into(),
        ))
        .unwrap();
    let mut first_byte = [0];
    assert!(
        blocked.get_ref().peek(&mut first_byte).unwrap() > 0,
        "server must begin its response before testing the other client"
    );
    let cursor = fast.snapshot().unwrap().cursor;
    let counter = submit(&mut fast, "inspect.query", query("Counter"));
    settled(&mut fast);
    let (events, _) = activity(&mut fast, cursor);
    assert_eq!(
        completed(&events, counter)["items"][0]["result"]["value"]["value"]["ticks"],
        0
    );
    drop(blocked);

    // The report incorporates the real large Inspect history, exceeds Activity
    // retention, but must still be saved completely by the local provider.
    let before_report = fast.snapshot().unwrap().cursor;
    submit(&mut fast, "tick.warp.start", json!({"ticks":1}));
    settled(&mut fast);
    let mut report = None;
    wait(|| {
        report = std::fs::read_dir(artifact.join("reports"))
            .ok()
            .and_then(|paths| {
                paths
                    .flatten()
                    .map(|e| e.path())
                    .find(|p| p.extension().is_some_and(|x| x == "md"))
            });
        report.is_some()
    });
    let markdown = std::fs::read(report.unwrap()).unwrap();
    assert!(markdown.len() > 4 * MIB);
    assert!(String::from_utf8_lossy(&markdown).contains(&"x".repeat(5 * MIB)));
    // Wait for asynchronous report activity, not merely the earlier warp reply.
    wait(|| {
        matches!(
            fast.poll(Some(before_report.clone()), 0).unwrap(),
            Reply::Gap { .. }
        )
    });
    let recovered = gap(&mut fast, before_report);
    let (events, _) = activity(&mut fast, recovered);
    assert!(!events.iter().any(|e| e["kind"] == "report"));
    assert_eq!(fast.snapshot().unwrap().state, Lifecycle::Ready);
    let cursor = fast.snapshot().unwrap().cursor;
    let counter = submit(&mut fast, "inspect.query", query("Counter"));
    settled(&mut fast);
    let (events, _) = activity(&mut fast, cursor);
    assert_eq!(
        completed(&events, counter)["items"][0]["result"]["value"]["value"]["ticks"],
        1
    );
    host.management
        .request("POST", &format!("/sessions/{id}/stop"), None)
        .unwrap();
    wait(|| fast.snapshot().unwrap().state == Lifecycle::Ended);
    #[cfg(unix)]
    {
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
    host.stop().unwrap();
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(&json!({
            "acceptance":"passed","activity_limit":4*MIB,"inspect_bytes":5*MIB,
            "report_bytes":markdown.len(),"delayed_poll_gap":true,"nonreading_socket":true,
            "lost_activity_outcomes":"unknown","session":id
        }))
        .unwrap(),
    )
    .unwrap();
    eprintln!("activity evidence: {}", root.display());
}
