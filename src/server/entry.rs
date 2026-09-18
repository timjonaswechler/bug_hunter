use super::{activity, protocol::*};
use crate::{
    command::{Command, Empty, recording, replay, tick::warp},
    session::{self, Error, Session},
};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(super) struct Entry {
    data: Mutex<Data>,
    admission: Mutex<()>,
    commands: mpsc::Sender<Submission>,
    pub cancel: Arc<AtomicBool>,
    forced: AtomicBool,
    pub done: AtomicBool,
    pub changed: tokio::sync::Notify,
}
struct Data {
    detail: Detail,
    activity: activity::Store,
    input: Option<mpsc::Receiver<Submission>>,
}
struct Submission {
    command: Command,
    answer: tokio::sync::oneshot::Sender<Result>,
}

impl Entry {
    pub fn new(id: String, artifact_dir: std::path::PathBuf, limit: usize) -> Arc<Self> {
        let (commands, input) = mpsc::channel();
        Arc::new(Self {
            data: Mutex::new(Data {
                detail: Detail {
                    id: id.clone(),
                    state: Lifecycle::Starting,
                    created_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis(),
                    artifact_dir,
                    error: None,
                },
                activity: activity::Store::new(id, limit),
                input: Some(input),
            }),
            admission: Mutex::new(()),
            commands,
            cancel: Arc::new(AtomicBool::new(false)),
            forced: AtomicBool::new(false),
            done: AtomicBool::new(false),
            changed: tokio::sync::Notify::new(),
        })
    }
    pub fn detail(&self) -> Detail {
        self.data.lock().unwrap().detail.clone()
    }
    fn transition(&self, state: Lifecycle, error: Option<Error>) {
        let mut data = self.data.lock().unwrap();
        if data.detail.state.terminal() {
            return;
        }
        data.detail.state = state;
        data.detail.error = error.clone();
        data.activity
            .push(json!({"kind":"lifecycle","state":state,"error":error}));
        self.changed.notify_waiters();
    }
    pub fn push(&self, event: Value) {
        self.data.lock().unwrap().activity.push(event);
        self.changed.notify_waiters();
    }
    pub fn poll(&self, cursor: Option<&Cursor>) -> Result {
        self.data.lock().unwrap().activity.poll(cursor)
    }
    pub fn stop(&self) {
        let _admission = self.admission.lock().unwrap();
        let mut data = self.data.lock().unwrap();
        if data.detail.state.terminal() || data.detail.state == Lifecycle::Stopping {
            return;
        }
        if data.detail.state == Lifecycle::Starting {
            self.cancel.store(true, Ordering::Release);
        }
        data.detail.state = Lifecycle::Stopping;
        data.activity
            .push(json!({"kind":"lifecycle","state":"Stopping","error":null}));
        self.changed.notify_waiters();
    }
    pub fn abort(&self) {
        if !self.done.load(Ordering::Acquire) {
            self.forced.store(true, Ordering::Release);
            self.cancel.store(true, Ordering::Release);
        }
    }
    pub async fn submit(&self, command: Command) -> Result {
        if self.detail().state != Lifecycle::Ready {
            return Error::new("session_not_ready", "commands require Ready").into();
        }
        let (answer, result) = tokio::sync::oneshot::channel();
        if self.commands.send(Submission { command, answer }).is_err() {
            return Error::ended().into();
        }
        result.await.unwrap_or_else(|_| Error::ended().into())
    }
}

pub(super) fn start(entry: Arc<Entry>, create: Create) {
    std::thread::spawn(move || {
        struct Completion<'a>(&'a Entry);
        impl Drop for Completion<'_> {
            fn drop(&mut self) {
                if !self.0.done.load(Ordering::Acquire) {
                    self.0.transition(
                        Lifecycle::Failed,
                        Some(Error::new(
                            "coordinator_lost",
                            "session owner terminated unexpectedly",
                        )),
                    );
                    self.0.done.store(true, Ordering::Release);
                    self.0.changed.notify_waiters();
                }
            }
        }
        let _completion = Completion(&entry);
        let input = entry.data.lock().unwrap().input.take().unwrap();
        let root = entry.detail().artifact_dir;
        // Exclusive final-component creation: an externally created directory or symlink is never adopted.
        let result = prepare(&root).and_then(|_| {
            if entry.cancel.load(Ordering::Acquire) {
                return Err(Error::ended());
            }
            Session::start_cancellable(
                session::Config {
                    launch: create.launch,
                    tick: create.tick,
                    report: create.report,
                    artifact_dir: root,
                },
                entry.cancel.clone(),
            )
        });
        match result {
            Ok(mut session) => {
                {
                    let mut data = entry.data.lock().unwrap();
                    if data.detail.state == Lifecycle::Starting {
                        data.detail.state = Lifecycle::Ready;
                        data.activity
                            .push(json!({"kind":"lifecycle","state":"Ready","error":null}));
                        entry.changed.notify_waiters();
                    }
                }
                run(&entry, &mut session, input);
                drop(session);
            }
            Err(e) => {
                if entry.detail().state == Lifecycle::Stopping
                    && matches!(e, Error::Ended)
                    && !entry.forced.load(Ordering::Acquire)
                {
                    entry.transition(Lifecycle::Ended, None);
                } else {
                    entry.transition(Lifecycle::Failed, Some(e));
                }
            }
        }
        entry.done.store(true, Ordering::Release);
        entry.changed.notify_waiters();
    });
}

fn prepare(root: &std::path::Path) -> std::result::Result<(), Error> {
    std::fs::create_dir(root).map_err(Error::io)
}

fn accept(
    entry: &Entry,
    session: &mut Session,
    command: Command,
    pending: &mut Vec<session::Pending<Value>>,
) -> Result {
    match session.submit::<Value>(command) {
        Ok(token) => {
            let request_id = token.request_id().as_u64();
            let command = token.command().name().to_string();
            entry.push(json!({"kind":"pending","request_id":request_id,"command":command}));
            if request_id == u64::MAX {
                entry.transition(Lifecycle::Stopping, None);
            }
            pending.push(token);
            Result::Pending {
                request_id,
                command,
            }
        }
        Err(e) => e.into(),
    }
}

fn run(entry: &Entry, session: &mut Session, input: mpsc::Receiver<Submission>) {
    let mut pending = Vec::new();
    let mut replay_stop = None;
    let mut replay_stopped = false;
    let mut stop_sent = false;
    let mut shutdown_sent = false;
    let mut recording_stop = None;
    loop {
        if entry.cancel.load(Ordering::Acquire) {
            entry.transition(
                Lifecycle::Failed,
                Some(Error::new("shutdown_incomplete", "forced session cleanup")),
            );
            break;
        }
        for _ in 0..64 {
            let Ok(submission) = input.try_recv() else {
                break;
            };
            let _admission = entry.admission.lock().unwrap();
            let result = if entry.detail().state != Lifecycle::Ready {
                Error::new("session_not_ready", "commands require Ready").into()
            } else {
                accept(entry, session, submission.command, &mut pending)
            };
            let _ = submission.answer.send(result);
        }
        let mut i = 0;
        while i < pending.len() {
            let result = session.try_receive(&mut pending[i]);
            if matches!(result, Ok(None)) {
                i += 1;
                continue;
            }
            let token = pending.remove(i);
            let id = token.request_id().as_u64();
            let name = token.command().name();
            match result {
                Ok(Some(output)) => {
                    if replay_stop == Some(id) {
                        replay_stopped = true;
                    }
                    entry.push(
                        json!({"kind":"completed","request_id":id,"command":name,"output":output}),
                    );
                    if id == u64::MAX {
                        entry.transition(Lifecycle::Ended, None);
                        return;
                    }
                }
                Err(error) => {
                    let technical = !matches!(error, Error::Rejected { .. });
                    entry.push(json!({"kind":if technical {"failed"} else {"rejected"},"request_id":id,"command":name,"error":error}));
                    if id == u64::MAX || recording_stop == Some(id) || replay_stop == Some(id) {
                        entry.transition(Lifecycle::Failed, Some(error));
                        return;
                    }
                }
                _ => unreachable!(),
            }
        }
        loop {
            match session.try_receive_event() {
                Ok(Some(event)) => entry.push(json!({"kind":"event","event":event})),
                Ok(None) => break,
                Err(e) => {
                    entry.transition(Lifecycle::Failed, Some(e));
                    return;
                }
            }
        }
        shutdown_sent |= pending.iter().any(|p| p.request_id().as_u64() == u64::MAX);
        if entry.detail().state == Lifecycle::Stopping && !shutdown_sent {
            if replay_stop.is_none() {
                match accept(entry, session, replay::Stop {}.into(), &mut pending) {
                    Result::Pending { request_id, .. } => replay_stop = Some(request_id),
                    Result::Error { code, message } => {
                        entry.transition(Lifecycle::Failed, Some(Error::new(&code, message)));
                        return;
                    }
                    _ => unreachable!("submission result"),
                }
            } else if !replay_stopped {
                // Replay owns its internal Warp-Stop and drains already-submitted work first.
            } else if !stop_sent {
                let result = accept(entry, session, warp::Stop {}.into(), &mut pending);
                if let Result::Error { code, message } = result {
                    entry.transition(Lifecycle::Failed, Some(Error::new(&code, message)));
                    return;
                }
                stop_sent = true;
            } else if pending.is_empty() {
                let result = accept(entry, session, Command::Shutdown(Empty {}), &mut pending);
                if matches!(&result, Result::Error { code, .. } if code == "shutdown_recording_active")
                {
                    // Direct shutdown stays strict. Management explicitly closes the recording
                    // and waits for the durable footer before retrying shutdown.
                    match accept(entry, session, recording::Stop {}.into(), &mut pending) {
                        Result::Pending { request_id, .. } => recording_stop = Some(request_id),
                        Result::Error { code, message } => {
                            entry.transition(Lifecycle::Failed, Some(Error::new(&code, message)));
                            return;
                        }
                        _ => unreachable!("submission result"),
                    }
                    continue;
                }
                if let Result::Error { code, message } = result {
                    entry.transition(Lifecycle::Failed, Some(Error::new(&code, message)));
                    return;
                }
                shutdown_sent = true;
            }
        }
        // A client-issued shutdown already went through the same session acceptance checks.
        shutdown_sent |= pending.iter().any(|p| p.request_id().as_u64() == u64::MAX);
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusive_creation_does_not_adopt_an_external_directory_or_symlink() {
        let parent = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/server-tests")
            .join(super::super::random_id().unwrap());
        std::fs::create_dir_all(&parent).unwrap();
        let root = parent.join("assigned");
        prepare(&root).unwrap();
        std::fs::write(root.join("external"), "preserve").unwrap();
        assert!(prepare(&root).is_err());
        assert_eq!(
            std::fs::read_to_string(root.join("external")).unwrap(),
            "preserve"
        );
        #[cfg(unix)]
        {
            let link = parent.join("link");
            std::os::unix::fs::symlink(&root, &link).unwrap();
            assert!(prepare(&link).is_err());
        }
    }
}
