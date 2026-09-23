//! Owns the bound client. No terminal IO and no automatic resubmission.
use crate::{
    client::{Client, Error, Interrupt, Management},
    command::Command,
    server::protocol::{Cursor, Result as Reply, Snapshot},
};
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Duration,
};

pub(super) enum Request {
    Submit(Command),
    Pending,
}
pub(super) enum Event {
    Connected(Snapshot),
    Pending(Snapshot),
    Reply(Reply),
    Notice(String),
    End(Result<(), String>),
}

struct Output {
    sender: mpsc::SyncSender<Event>,
    stop: Arc<AtomicBool>,
}
impl Output {
    fn send(&self, mut event: Event) -> Result<(), ()> {
        while !self.stop.load(Ordering::Acquire) {
            match self.sender.try_send(event) {
                Ok(()) => return Ok(()),
                Err(mpsc::TrySendError::Disconnected(_)) => return Err(()),
                Err(mpsc::TrySendError::Full(value)) => event = value,
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(())
    }
}

pub(super) struct Network {
    pub requests: mpsc::SyncSender<Request>,
    pub events: mpsc::Receiver<Event>,
    stop: Arc<AtomicBool>,
    interrupt: Arc<Interrupt>,
    worker: Option<JoinHandle<()>>,
}
impl Network {
    pub fn start(address: SocketAddr, selector: String) -> std::io::Result<Self> {
        let (requests, input) = mpsc::sync_channel(16);
        let (sender, events) = mpsc::sync_channel(32);
        let stop = Arc::new(AtomicBool::new(false));
        let output = Output {
            sender,
            stop: stop.clone(),
        };
        let interrupt = Arc::new(Interrupt::default());
        let worker_stop = stop.clone();
        let worker_interrupt = interrupt.clone();
        let worker = std::thread::Builder::new()
            .name("repl-client".into())
            .spawn(move || {
                let result = run(
                    address,
                    selector,
                    &input,
                    &output,
                    &worker_stop,
                    &worker_interrupt,
                );
                let _ = output.send(Event::End(result));
            })?;
        Ok(Self {
            requests,
            events,
            stop,
            interrupt,
            worker: Some(worker),
        })
    }
}
impl Drop for Network {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.interrupt.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(
    address: SocketAddr,
    mut selector: String,
    input: &mpsc::Receiver<Request>,
    output: &Output,
    stop: &AtomicBool,
    interrupt: &Interrupt,
) -> Result<(), String> {
    let management = Management::new(address).map_err(|e| e.to_string())?;
    if !(8..=32).contains(&selector.len()) || !selector.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("expected session selector of 8 to 32 hex digits".into());
    }
    let mut client: Option<Client> = None;
    let mut cursor: Option<Cursor> = None;
    let mut shutdown = false;
    while !stop.load(Ordering::Acquire) {
        if client.is_none() {
            match management
                .bind_interruptible(&selector, interrupt)
                .and_then(|mut c| {
                    let snapshot = c.snapshot()?;
                    Ok((c, snapshot))
                }) {
                Ok((c, snapshot)) => {
                    if cursor
                        .as_ref()
                        .is_some_and(|old| old.session_id != snapshot.cursor.session_id)
                    {
                        return Err("reconnected to a different session".into());
                    }
                    // Pin the full ID after the initial bind, not its possibly ambiguous prefix.
                    selector = snapshot.cursor.session_id.clone();
                    if cursor.is_none() {
                        cursor = Some(snapshot.cursor.clone());
                        if snapshot.state.terminal() {
                            return Err(format!("session already {:?}", snapshot.state));
                        }
                    }
                    let _ = output.send(Event::Connected(snapshot));
                    client = Some(c);
                }
                Err(e) => {
                    if matches!(e, Error::Protocol(_) | Error::Remote { .. }) {
                        return Err(e.to_string());
                    }
                    let _ = output.send(Event::Notice(format!(
                        "disconnected: {e}; reconnecting, no commands retried"
                    )));
                    // Commands entered while disconnected must not execute later unexpectedly.
                    for _ in 0..25 {
                        while input.try_recv().is_ok() {
                            let _ =
                                output.send(Event::Notice("not sent: client disconnected".into()));
                        }
                        if stop.load(Ordering::Acquire) {
                            return Ok(());
                        }
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    continue;
                }
            }
        }
        let c = client.as_mut().unwrap();
        // One input per turn, so queued input cannot starve Activity.
        let action = match input.try_recv() {
            Ok(Request::Pending) => c.snapshot().map(|s| {
                let _ = output.send(Event::Pending(s));
            }),
            Ok(Request::Submit(command)) => {
                // The server remains authoritative in the race with another client's Replay.
                c.snapshot()
                    .inspect_err(|_| {
                        let _ = output.send(Event::Notice(
                            "not sent: pre-submission state unavailable".into(),
                        ));
                    })
                    .and_then(|s| {
                        if s.pending.iter().any(|p| p.command == "replay.start")
                            && command.name() != "replay.stop"
                        {
                            let _ = output.send(Event::Notice(
                                "replay_in_progress: only replay stop is allowed".into(),
                            ));
                            return Ok(());
                        }
                        let reply = c.submit(command)?;
                        if matches!(&reply, Reply::Pending { command, .. } if command == "shutdown")
                        {
                            shutdown = true;
                        }
                        let _ = output.send(Event::Reply(reply));
                        Ok(())
                    })
            }
            Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            Err(mpsc::TryRecvError::Empty) => Ok(()),
        };
        let result = action.and_then(|_| c.poll(cursor.clone(), 50));
        match result {
            Ok(reply) => {
                if let Reply::Error { code, message } = &reply {
                    return Err(format!("Activity unavailable: {code}: {message}"));
                }
                let mut terminal = None;
                match &reply {
                    Reply::Activity {
                        entries,
                        cursor: next,
                    } => {
                        cursor = Some(next.clone());
                        for entry in entries {
                            if entry.event["kind"] == "lifecycle" {
                                match entry.event["state"].as_str() {
                                    Some("Ended") => {
                                        terminal = Some(if shutdown {
                                            Ok(())
                                        } else {
                                            Err("session ended outside this REPL".into())
                                        })
                                    }
                                    Some("Failed") => {
                                        terminal = Some(Err(format!(
                                            "session failed: {}",
                                            entry.event["error"]
                                        )))
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Reply::Gap { cursor: next, .. } => cursor = Some(next.clone()),
                    _ => {}
                }
                let gap = matches!(reply, Reply::Gap { .. });
                let _ = output.send(Event::Reply(reply));
                if gap {
                    // A lifecycle event may itself have been evicted.
                    match c.snapshot() {
                        Ok(snapshot) => {
                            if snapshot.state.terminal() {
                                terminal = Some(Err("session ended across an Activity gap; missing outcomes are unknown".into()));
                            }
                            let _ = output.send(Event::Pending(snapshot));
                        }
                        Err(e) => {
                            let _ =
                                output.send(Event::Notice(format!("snapshot unavailable: {e}")));
                            client = None;
                        }
                    }
                }
                if let Some(result) = terminal {
                    return result;
                }
            }
            Err(e) => {
                // Even a malformed reply may hide an accepted Submit. Never replay it.
                let message = match e {
                    Error::SubmissionUnknown(_) => {
                        format!("submission outcome unknown: {e}; not retried")
                    }
                    _ => format!("client error: {e}"),
                };
                let _ = output.send(Event::Notice(message));
                while input.try_recv().is_ok() {
                    let _ = output.send(Event::Notice("not sent: connection lost".into()));
                }
                client = None;
            }
        }
    }
    Ok(())
}
