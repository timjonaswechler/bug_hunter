//! Synchronous typed sessions backed by a private, continuously progressing coordinator.
mod coordinator;
mod error;
pub mod history;
mod input;
mod inspect;
pub mod launch;
mod plugin;
mod process;
pub mod protocol;
mod recording;
mod replay;
mod screenshot;
mod window;

use crate::command::{self, Command};
pub use error::Error;
pub use plugin::Plugin;
pub use protocol::Capabilities;
use serde::Serialize;
use std::{
    collections::{BTreeMap, VecDeque},
    marker::PhantomData,
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

#[derive(Clone, Debug)]
pub struct Config {
    pub launch: launch::Config,
    pub artifact_dir: PathBuf,
    pub tick: command::tick::Config,
    pub report: crate::report::Config,
}
impl Config {
    pub fn validate(&self) -> Result<(), Error> {
        self.launch.validate()?;
        self.report.validate()?;
        if self.artifact_dir.as_os_str().is_empty() || !self.tick.pace.is_valid() {
            return Err(Error::new(
                "invalid_config",
                "artifact directory and valid tick pace required",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RequestId(u64);
impl RequestId {
    pub fn as_u64(self) -> u64 {
        self.0
    }
}
impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub struct Pending<T> {
    owner: Arc<Shared>,
    id: RequestId,
    command: Command,
    consumed: bool,
    marker: PhantomData<T>,
}
impl<T> Pending<T> {
    pub fn request_id(&self) -> RequestId {
        self.id
    }
    pub fn command(&self) -> &Command {
        &self.command
    }
}
impl<T> Drop for Pending<T> {
    fn drop(&mut self) {
        if !self.consumed {
            let mut state = self.owner.state.lock().unwrap();
            if state.results.remove(&self.id.0).is_none() && !state.ended {
                state.abandoned.insert(self.id.0);
            }
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    ProtocolError { code: String, message: String },
    RecordingFailed { path: String, message: String },
    Ended { reason: EndReason },
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EndReason {
    ProcessExit { status: String },
    TransportClosed { channel: String },
    TransportFailed { channel: String, message: String },
    EventQueueOverflow { capacity: u32, dropped_events: u64 },
}

type Outcome = Result<serde_json::Value, Error>;
#[derive(Default)]
struct State {
    results: BTreeMap<u64, Outcome>,
    abandoned: std::collections::BTreeSet<u64>,
    events: VecDeque<Event>,
    history: VecDeque<(u64, history::Entry)>,
    ended: bool,
}
#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}
enum Operation {
    Send(Command, mpsc::Sender<Result<u64, Error>>),
}

pub struct Session {
    shared: Arc<Shared>,
    operations: mpsc::Sender<Operation>,
    cancel: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    capabilities: Capabilities,
}

impl Session {
    pub fn start(config: Config) -> Result<Self, Error> {
        Self::start_cancellable(config, Arc::new(AtomicBool::new(false)))
    }

    pub(crate) fn start_cancellable(
        config: Config,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self, Error> {
        config.validate()?;
        let shared = Arc::new(Shared::default());
        let (operations, input) = mpsc::channel();
        let (ready, initialized) = mpsc::channel();
        let state = shared.clone();
        let abort = cancel.clone();
        let worker =
            std::thread::spawn(move || coordinator::run(config, input, state, abort, ready));
        match initialized.recv().unwrap_or_else(|_| Err(Error::ended())) {
            Ok(capabilities) => Ok(Self {
                shared,
                operations,
                cancel,
                worker: Some(worker),
                capabilities,
            }),
            Err(e) => {
                cancel.store(true, Ordering::Release);
                let _ = worker.join();
                Err(e)
            }
        }
    }

    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }
    pub fn history(&self) -> Vec<history::Entry> {
        self.shared
            .state
            .lock()
            .unwrap()
            .history
            .iter()
            .map(|(_, e)| e.clone())
            .collect()
    }

    pub fn send<C: command::Request>(&mut self, command: C) -> Result<Pending<C::Output>, Error> {
        self.submit(command.into())
    }
    pub(crate) fn submit<T>(&mut self, command: Command) -> Result<Pending<T>, Error> {
        let (tx, rx) = mpsc::channel();
        self.operations
            .send(Operation::Send(command.clone(), tx))
            .map_err(|_| Error::ended())?;
        let id = rx.recv().map_err(|_| Error::ended())??;
        Ok(Pending {
            owner: self.shared.clone(),
            id: RequestId(id),
            command,
            consumed: false,
            marker: PhantomData,
        })
    }
    pub fn try_receive<T: serde::de::DeserializeOwned>(
        &mut self,
        pending: &mut Pending<T>,
    ) -> Result<Option<T>, Error> {
        if !Arc::ptr_eq(&self.shared, &pending.owner) || pending.consumed {
            return Err(Error::new(
                "invalid_pending",
                "pending belongs to another session or was consumed",
            ));
        }
        let mut state = self.shared.state.lock().unwrap();
        if let Some(result) = state.results.remove(&pending.id.0) {
            pending.consumed = true;
            return result.and_then(|v| {
                serde_json::from_value(v)
                    .map(Some)
                    .map_err(|e| Error::new("invalid_response", e))
            });
        }
        if state.ended {
            pending.consumed = true;
            return Err(Error::ended());
        }
        Ok(None)
    }
    pub fn receive<T: serde::de::DeserializeOwned>(
        &mut self,
        mut pending: Pending<T>,
    ) -> Result<T, Error> {
        loop {
            if let Some(output) = self.try_receive(&mut pending)? {
                return Ok(output);
            }
            let state = self.shared.state.lock().unwrap();
            if !state.ended && !state.results.contains_key(&pending.id.0) {
                drop(self.shared.changed.wait(state).unwrap());
            }
        }
    }
    pub fn try_receive_event(&mut self) -> Result<Option<Event>, Error> {
        let mut state = self.shared.state.lock().unwrap();
        if let Some(event) = state.events.pop_front() {
            return Ok(Some(event));
        }
        if state.ended {
            return Err(Error::ended());
        }
        Ok(None)
    }
    pub fn receive_event(&mut self) -> Result<Event, Error> {
        loop {
            if let Some(event) = self.try_receive_event()? {
                return Ok(event);
            }
            let state = self.shared.state.lock().unwrap();
            if !state.ended && state.events.is_empty() {
                drop(self.shared.changed.wait(state).unwrap());
            }
        }
    }
    pub fn shutdown(&mut self) -> Result<(), Error> {
        let pending = self.submit::<()>(Command::Shutdown(command::Empty {}))?;
        self.receive(pending)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
