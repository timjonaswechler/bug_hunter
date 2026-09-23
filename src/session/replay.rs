//! Replay owns loading, barriers and stop state, but never allocates request IDs or writes pipes.
use super::{Error, Outcome};
use crate::command::{Command, replay, tick::warp};
use cap_std::fs::Dir;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

mod loader;
#[cfg(test)]
mod tests;

enum Phase {
    Waiting(String),
    Preparing {
        cancel: Arc<AtomicBool>,
        result: mpsc::Receiver<Result<Option<loader::Plan>, Error>>,
    },
    Running(loader::Plan),
}

struct Run {
    start: u64,
    phase: Phase,
    stopping: bool,
    stops: Vec<u64>,
    pending: BTreeMap<u64, bool>,
    warp: Option<u64>,
    stop_sent: bool,
    blocked: Option<(&'static str, Error)>,
}

pub(super) struct Replay {
    root: Arc<Dir>,
    run: Option<Run>,
    notices: Vec<(u64, Outcome)>,
}

impl Replay {
    pub fn new(root: Arc<Dir>) -> Self {
        Self {
            root,
            run: None,
            notices: Vec::new(),
        }
    }

    pub fn busy(&self) -> bool {
        self.run.is_some()
    }

    pub fn control(&mut self, id: u64, command: &Command) {
        match command {
            Command::ReplayStart(start) => {
                if self.busy() {
                    self.notices.push((
                        id,
                        Err(Error::new("replay_already_running", "replay is active")),
                    ));
                    return;
                }
                self.run = Some(Run {
                    start: id,
                    phase: Phase::Waiting(start.path.clone()),
                    stopping: false,
                    stops: Vec::new(),
                    pending: BTreeMap::new(),
                    warp: None,
                    stop_sent: false,
                    blocked: None,
                });
            }
            Command::ReplayStop(_) => {
                if let Some(run) = &mut self.run {
                    run.stopping = true;
                    run.stops.push(id);
                    if let Phase::Preparing { cancel, .. } = &run.phase {
                        cancel.store(true, Ordering::Release);
                    }
                } else {
                    self.notices.push((
                        id,
                        Ok(serde_json::to_value(replay::Stopped { was_running: false }).unwrap()),
                    ));
                }
            }
            _ => unreachable!("replay control"),
        }
    }

    pub fn poll(&mut self, file_barrier: bool) -> Vec<(u64, Outcome)> {
        if let Some(run) = &mut self.run {
            if let Phase::Waiting(path) = &run.phase {
                if run.stopping {
                    run.phase = Phase::Running(Default::default());
                } else if !file_barrier {
                    let root = self.root.clone();
                    let path = path.clone();
                    let cancel = Arc::new(AtomicBool::new(false));
                    let abort = cancel.clone();
                    let (reply, result) = mpsc::channel();
                    std::thread::spawn(move || {
                        let _ = reply.send(loader::load(&root, &path, &abort));
                    });
                    run.phase = Phase::Preparing { cancel, result };
                }
            }
            if let Phase::Preparing { result, .. } = &run.phase {
                match result.try_recv() {
                    Ok(result) => {
                        if run.stopping {
                            // Stop was observed before the loader's reply. Its acknowledgement,
                            // including a racing error, ends cancellation rather than starting work.
                            run.phase = Phase::Running(Default::default());
                        } else {
                            match result {
                                Ok(Some(plan)) => run.phase = Phase::Running(plan),
                                Ok(None) => unreachable!("only Stop cancels a live loader"),
                                Err(error) => {
                                    let id = run.start;
                                    self.run = None;
                                    self.notices.push((id, Err(error)));
                                }
                            }
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.block("session_io_failed", Error::io("replay loader disconnected"));
                        if let Some(run) = &mut self.run {
                            run.phase = Phase::Running(Default::default());
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
        }
        let finished = self.run.as_ref().is_some_and(|run| {
            matches!(&run.phase, Phase::Running(plan) if plan.is_empty() || run.stopping || run.blocked.is_some())
                && run.pending.is_empty()
        });
        if finished {
            self.finish();
        }
        std::mem::take(&mut self.notices)
    }

    // Return at most one command so acceptance and pipe submission stay in one coordinator path.
    // Earlier external work and recording file transitions must drain before entering the plan.
    pub fn next(&mut self, earlier_pending: bool) -> Option<Command> {
        let run = self.run.as_mut()?;
        if run.stopping || run.blocked.is_some() {
            if run.warp.is_some() && !run.stop_sent {
                run.stop_sent = true;
                return Some(warp::Stop {}.into());
            }
            return None;
        }
        let Phase::Running(plan) = &mut run.phase else {
            return None;
        };
        if earlier_pending
            || run.warp.is_some()
            || (matches!(plan.front(), Some(Command::Start(_))) && !run.pending.is_empty())
        {
            return None;
        }
        plan.pop_front()
    }

    pub fn owns(&self, id: u64) -> bool {
        self.run
            .as_ref()
            .is_some_and(|run| run.pending.contains_key(&id))
    }

    pub fn submitted(&mut self, id: u64, command: &Command) {
        let run = self.run.as_mut().unwrap();
        let stopping =
            (run.stopping || run.blocked.is_some()) && matches!(command, Command::Stop(_));
        run.pending.insert(id, stopping);
        if matches!(command, Command::Start(_)) {
            run.warp = Some(id);
        }
    }

    pub fn terminal(&mut self, id: u64, outcome: &Outcome) {
        let Some(run) = &mut self.run else {
            return;
        };
        let Some(stop) = run.pending.remove(&id) else {
            return;
        };
        if run.warp == Some(id) {
            run.warp = None;
        }
        if let Err(error) = outcome {
            let code = match error {
                Error::Rejected { .. } if !stop => return,
                Error::Protocol { .. } if !stop => "command_protocol_failed",
                Error::Io { .. } => "session_io_failed",
                Error::Ended => "session_ended",
                Error::RequestIdExhausted => "request_id_exhausted",
                _ => "stop_failed",
            };
            self.block(code, error.clone());
            if stop {
                // There is no controlled way to drain this Warp after Stop failed. Other
                // already-sent work remains owned by Session, not hidden or cancelled.
                self.finish();
            }
        }
    }

    pub fn block(&mut self, code: &'static str, error: Error) {
        if let Some(run) = &mut self.run {
            run.blocked.get_or_insert((code, error));
        }
    }

    pub fn submission_failed(&mut self, error: Error) {
        let code = if matches!(error, Error::RequestIdExhausted) {
            "request_id_exhausted"
        } else {
            "session_io_failed"
        };
        self.block(code, error);
        // Exhausted IDs can also prevent issuing the internal Warp-Stop.
        if self
            .run
            .as_ref()
            .is_some_and(|run| run.warp.is_some() && run.stop_sent)
        {
            self.finish();
        }
    }

    pub fn end(&mut self, error: Error) -> Vec<(u64, Outcome)> {
        if let Some(Run {
            phase: Phase::Preparing { cancel, .. },
            ..
        }) = &self.run
        {
            cancel.store(true, Ordering::Release);
        }
        let code = if matches!(error, Error::Io { .. }) {
            "session_io_failed"
        } else {
            "session_ended"
        };
        self.block(code, error);
        if self.busy() {
            self.finish();
        }
        std::mem::take(&mut self.notices)
    }

    fn finish(&mut self) {
        let run = self.run.take().unwrap();
        let outcome = match &run.blocked {
            Some((code, error)) => replay::Outcome::Blocked {
                code: (*code).into(),
                message: error.message().into(),
            },
            None if run.stopping => replay::Outcome::Stopped {},
            None => replay::Outcome::Completed {},
        };
        self.notices.push((
            run.start,
            Ok(serde_json::to_value(replay::Completion { outcome }).unwrap()),
        ));
        for id in run.stops {
            let outcome = match &run.blocked {
                Some((_, error)) => Err(error.clone()),
                None => Ok(serde_json::to_value(replay::Stopped { was_running: true }).unwrap()),
            };
            self.notices.push((id, outcome));
        }
    }
}

impl Drop for Replay {
    fn drop(&mut self) {
        if let Some(Run {
            phase: Phase::Preparing { cancel, .. },
            ..
        }) = &self.run
        {
            cancel.store(true, Ordering::Release);
        }
    }
}
