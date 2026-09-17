//! Recording state and command-order buffering; all filesystem work belongs to the worker.
use super::{Error, Event, Outcome};
use crate::command::{Command, recording};
use cap_std::fs::Dir;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

mod file;
mod worker;

#[cfg(test)]
mod tests;

enum State {
    Idle,
    Starting { id: u64, path: String },
    Active,
    Stopping { id: u64, path: String },
}

struct Entry {
    id: u64,
    command: Command,
    outcome: Option<Outcome>,
}

pub(super) enum Notice {
    Complete { id: u64, outcome: Outcome },
    Event(Event),
}

pub(super) struct Recorder {
    state: State,
    path: String,
    entries: VecDeque<Entry>,
    operations: mpsc::Sender<worker::Operation>,
    replies: mpsc::Receiver<worker::Reply>,
}

impl Recorder {
    pub fn new(root: Dir) -> Self {
        let (operations, replies) = worker::spawn(root);
        Self {
            state: State::Idle,
            path: String::new(),
            entries: VecDeque::new(),
            operations,
            replies,
        }
    }

    pub fn busy(&self) -> bool {
        !matches!(self.state, State::Idle)
    }
    pub fn transitioning(&self) -> bool {
        matches!(self.state, State::Starting { .. } | State::Stopping { .. })
    }

    pub fn control(
        &mut self,
        id: u64,
        command: &Command,
        commands_pending: bool,
    ) -> Result<(), Error> {
        if self.transitioning() {
            return Err(Error::new(
                "recording_transition_in_progress",
                "recording file transition is pending",
            ));
        }
        match command {
            Command::RecordingStart(start) => {
                if self.busy() {
                    return Err(Error::new(
                        "recording_already_active",
                        "stop the current recording first",
                    ));
                }
                if commands_pending {
                    return Err(Error::new(
                        "recording_commands_pending",
                        "earlier commands are pending",
                    ));
                }
                file::validate(&start.path)?;
                self.operations
                    .send(worker::Operation::Start(start.path.clone()))
                    .map_err(Error::io)?;
                self.state = State::Starting {
                    id,
                    path: start.path.clone(),
                };
            }
            Command::RecordingStop(_) => {
                if !self.busy() {
                    return Err(Error::new("recording_not_active", "no recording is active"));
                }
                if commands_pending {
                    return Err(Error::new(
                        "recording_commands_pending",
                        "earlier commands are pending",
                    ));
                }
                self.operations
                    .send(worker::Operation::Stop)
                    .map_err(Error::io)?;
                self.state = State::Stopping {
                    id,
                    path: self.path.clone(),
                };
            }
            _ => unreachable!("only recording controls"),
        }
        Ok(())
    }

    pub fn record(&mut self, id: u64, command: &Command) {
        if matches!(self.state, State::Active) && command.is_recordable() {
            self.entries.push_back(Entry {
                id,
                command: command.clone(),
                outcome: None,
            });
        }
    }

    pub fn terminal(&mut self, id: u64, outcome: &Outcome) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.outcome = Some(outcome.clone());
        }
        let mut ready = Vec::new();
        while self
            .entries
            .front()
            .is_some_and(|entry| entry.outcome.is_some())
        {
            let entry = self.entries.pop_front().unwrap();
            ready.push(file::entry(&entry.command, entry.outcome.as_ref()));
        }
        if !ready.is_empty() {
            // The worker remains alive for the whole recorder lifetime.
            self.operations
                .send(worker::Operation::Append(ready))
                .expect("recording worker");
        }
    }

    fn reply(&mut self, reply: worker::Reply) -> Vec<Notice> {
        match reply {
            worker::Reply::Started(result) => {
                let State::Starting { id, path } = std::mem::replace(&mut self.state, State::Idle)
                else {
                    unreachable!("start reply without transition")
                };
                let outcome = result.map(|()| {
                    self.path = path.clone();
                    self.state = State::Active;
                    serde_json::to_value(recording::Started { path }).unwrap()
                });
                vec![Notice::Complete { id, outcome }]
            }
            worker::Reply::Stopped(result) => {
                let State::Stopping { id, path } = std::mem::replace(&mut self.state, State::Idle)
                else {
                    unreachable!("stop reply without transition")
                };
                vec![Notice::Complete {
                    id,
                    outcome: result.map(|recorded_commands| {
                        serde_json::to_value(recording::Stopped {
                            path,
                            recorded_commands,
                        })
                        .unwrap()
                    }),
                }]
            }
            worker::Reply::Failed { path, message } => {
                if matches!(self.state, State::Active) {
                    self.state = State::Idle;
                }
                self.entries.clear();
                vec![Notice::Event(Event::RecordingFailed { path, message })]
            }
            worker::Reply::Ended => Vec::new(),
        }
    }

    pub fn poll(&mut self) -> Vec<Notice> {
        let mut notices = Vec::new();
        while let Ok(reply) = self.replies.try_recv() {
            notices.extend(self.reply(reply));
        }
        notices
    }

    pub fn end(&mut self, cancel: &AtomicBool) -> Vec<Notice> {
        let entries = self
            .entries
            .drain(..)
            .map(|entry| file::entry(&entry.command, entry.outcome.as_ref()))
            .collect();
        if self
            .operations
            .send(worker::Operation::End(entries))
            .is_err()
        {
            return if self.busy() {
                vec![Notice::Event(Event::RecordingFailed {
                    path: self.path.clone(),
                    message: "recording worker disconnected".into(),
                })]
            } else {
                Vec::new()
            };
        }
        let mut notices = Vec::new();
        loop {
            match self.replies.recv_timeout(Duration::from_millis(20)) {
                Ok(worker::Reply::Ended) => break,
                Ok(reply) => notices.extend(self.reply(reply)),
                // Forced cancellation must not wait indefinitely for a blocked filesystem.
                Err(mpsc::RecvTimeoutError::Timeout) if !cancel.load(Ordering::Acquire) => {}
                Err(_) => break,
            }
        }
        notices
    }
}
