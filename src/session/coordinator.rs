use super::protocol::Message;
use super::*;
use std::time::Duration;

fn launch(config: &Config, cancel: &Arc<AtomicBool>) -> Result<process::Process, Error> {
    let (project, mut command) = config.launch.resolve(cancel)?;
    if cancel.load(Ordering::Acquire) {
        return Err(Error::ended());
    }
    let root = if config.artifact_dir.is_absolute() {
        config.artifact_dir.clone()
    } else {
        project.join(&config.artifact_dir)
    };
    if std::fs::symlink_metadata(&root).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::new(
            "invalid_config",
            "artifact root must not be a symlink",
        ));
    }
    std::fs::create_dir_all(&root).map_err(Error::io)?;
    let root = root.canonicalize().map_err(Error::io)?;
    command
        .env("WOODPECKER_ARTIFACT_DIR", root)
        .env("RUST_BACKTRACE", "1")
        .env(
            "WOODPECKER_TICK_PACE",
            serde_json::to_string(&config.tick.pace).unwrap(),
        );
    process::Process::spawn(command)
}

pub(super) fn run(
    config: Config,
    input: mpsc::Receiver<Operation>,
    shared: Arc<Shared>,
    cancel: Arc<AtomicBool>,
    ready: mpsc::Sender<Result<Capabilities, Error>>,
) {
    // A lost coordinator must wake receivers even when unwinding bypasses the normal end path.
    struct EndOnDrop(Arc<Shared>);
    impl Drop for EndOnDrop {
        fn drop(&mut self) {
            self.0.state.lock().unwrap_or_else(|e| e.into_inner()).ended = true;
            self.0.changed.notify_all();
        }
    }
    let _end = EndOnDrop(shared.clone());
    let result = launch(&config, &cancel);
    let mut process = match result {
        Ok(process) => process,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let mut initialized = false;
    let mut active = BTreeMap::<u64, Command>::new();
    let mut next = 1u64;
    let mut shutdown: Option<Outcome> = None;
    let mut stopping = false;
    let mut reason = None;
    let mut closed = 0;
    let mut exit_status = None;
    let mut diagnostics = String::new();
    loop {
        if cancel.load(Ordering::Acquire) {
            break;
        }
        for _ in 0..128 {
            let Ok(event) = process.events.try_recv() else {
                break;
            };
            match event {
                process::Event::Diagnostic(bytes) => {
                    // Keep startup diagnostics bounded. Never write to a potentially blocked terminal
                    // from the coordinator. The reporting/diagnostic observer is a later slice.
                    diagnostics.push_str(&String::from_utf8_lossy(&bytes));
                    if diagnostics.len() > 16384 {
                        diagnostics = diagnostics
                            .chars()
                            .rev()
                            .take(8192)
                            .collect::<String>()
                            .chars()
                            .rev()
                            .collect();
                    }
                }
                process::Event::Closed(channel) => {
                    closed += 1;
                    if !stopping && process.child.try_wait().ok().flatten().is_none() {
                        reason.get_or_insert(EndReason::TransportClosed {
                            channel: channel.into(),
                        });
                    }
                }
                process::Event::Failed(channel, message) => {
                    reason.get_or_insert(EndReason::TransportFailed {
                        channel: channel.into(),
                        message,
                    });
                }
                process::Event::Line(line) => {
                    let decoded = serde_json::from_str::<Message>(&line);
                    if !initialized {
                        match decoded {
                            Ok(Message::Ready {
                                version: protocol::VERSION,
                                capabilities,
                            }) => {
                                initialized = true;
                                let _ = ready.send(Ok(capabilities));
                            }
                            Ok(Message::Ready { .. }) => {
                                let _ = ready.send(Err(Error::new(
                                    "unsupported_protocol_version",
                                    "expected v3",
                                )));
                                return;
                            }
                            _ => {
                                let _ = ready.send(Err(Error::new("invalid_ready", line)));
                                return;
                            }
                        }
                        continue;
                    }
                    let id = decoded.as_ref().ok().and_then(Message::id).or_else(|| {
                        serde_json::from_str::<serde_json::Value>(&line)
                            .ok()
                            .and_then(|v| v["request_id"].as_u64())
                    });
                    if let Some((id, command)) =
                        id.and_then(|id| active.remove(&id).map(|c| (id, c)))
                    {
                        let outcome = match decoded {
                            Ok(Message::Completed {
                                command: name,
                                output,
                                ..
                            }) if name == command.name() && command.validate_output(&output) => {
                                Ok(output)
                            }
                            Ok(Message::Rejected {
                                command: name,
                                error,
                                ..
                            }) if name == command.name() => Err(Error::Rejected {
                                code: error.code,
                                message: error.message,
                            }),
                            Ok(Message::ProtocolError { error, .. }) => Err(Error::Protocol {
                                code: error.code,
                                message: error.message,
                            }),
                            _ => Err(Error::new(
                                "invalid_response",
                                "response does not match the accepted command",
                            )),
                        };
                        if id == u64::MAX {
                            shutdown = Some(outcome);
                        } else {
                            complete(&shared, id, outcome);
                        }
                    } else {
                        let (code, message) = match decoded {
                            Ok(Message::Ready { .. }) => {
                                ("unexpected_ready".into(), "additional Ready ignored".into())
                            }
                            Ok(Message::ProtocolError { error, .. }) => (error.code, error.message),
                            _ => (
                                "invalid_response".into(),
                                "response has no open request".into(),
                            ),
                        };
                        let mut state = shared.state.lock().unwrap();
                        if state.events.len() < 256 {
                            state
                                .events
                                .push_back(Event::ProtocolError { code, message });
                        } else if let Some(EndReason::EventQueueOverflow {
                            dropped_events, ..
                        }) = &mut reason
                        {
                            *dropped_events += 1;
                        } else if reason.is_none() {
                            reason = Some(EndReason::EventQueueOverflow {
                                capacity: 256,
                                dropped_events: 1,
                            });
                        }
                        shared.changed.notify_all();
                    }
                }
            }
        }
        if stopping && shutdown.as_ref().is_some_and(Result::is_err) {
            break;
        }
        if reason.is_some() && exit_status.is_none() {
            process.terminate();
        }
        match exit_status.map_or_else(|| process.child.try_wait(), |status| Ok(Some(status))) {
            Ok(Some(status)) => {
                if exit_status.is_none() {
                    // Descendants may still hold the pipes after the direct child exits.
                    process.terminate();
                    exit_status = Some(status);
                }
                if closed < 2 {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                }
                if stopping && status.success() && shutdown.as_ref().is_some_and(Result::is_ok) {
                    break;
                }
                reason.get_or_insert(EndReason::ProcessExit {
                    status: status.to_string(),
                });
                break;
            }
            Err(e) => {
                reason = Some(EndReason::TransportFailed {
                    channel: "process".into(),
                    message: e.to_string(),
                });
                break;
            }
            _ => {}
        }
        if reason.is_some() {
            break;
        }
        if initialized {
            for _ in 0..64 {
                let Ok(Operation::Send(command, response)) = input.try_recv() else {
                    break;
                };
                let is_shutdown = matches!(command, Command::Shutdown(_));
                let id = if stopping {
                    Err(Error::ended())
                } else if is_shutdown && !active.is_empty() {
                    Err(Error::new(
                        "shutdown_commands_pending",
                        "commands are still pending",
                    ))
                } else if is_shutdown {
                    stopping = true;
                    Ok(u64::MAX)
                } else if next == u64::MAX {
                    Err(Error::new(
                        "request_id_exhausted",
                        "normal request IDs exhausted",
                    ))
                } else {
                    let id = next;
                    next += 1;
                    Ok(id)
                };
                match id {
                    Err(e) => {
                        let _ = response.send(Err(e));
                    }
                    Ok(id) => {
                        let mut state = shared.state.lock().unwrap();
                        if state.history.len() == 50 {
                            state.history.pop_front();
                        }
                        state.history.push_back((
                            id,
                            history::Entry {
                                command: command.clone(),
                                outcome: history::Outcome::Unanswered,
                            },
                        ));
                        drop(state);
                        active.insert(id, command.clone());
                        let _ = response.send(Ok(id));
                        if process
                            .writer
                            .as_ref()
                            .unwrap()
                            .send(protocol::encode(id, &command))
                            .is_err()
                        {
                            reason = Some(EndReason::TransportClosed {
                                channel: "stdin".into(),
                            });
                            break;
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    if !initialized {
        let error = if cancel.load(Ordering::Acquire) {
            Error::ended()
        } else {
            Error::new(
                "launch",
                format!(
                    "game ended before Ready; reason={reason:?}; status={exit_status:?}: {diagnostics}"
                ),
            )
        };
        let _ = ready.send(Err(error));
    }
    drop(process);
    if stopping {
        let result = if reason.is_some() || cancel.load(Ordering::Acquire) {
            Err(Error::ended())
        } else {
            shutdown.unwrap_or_else(|| Err(Error::ended()))
        };
        complete(&shared, u64::MAX, result);
    }
    let mut state = shared.state.lock().unwrap();
    if let Some(reason) = reason {
        state.events.push_back(Event::Ended { reason });
    }
    state.ended = true;
    shared.changed.notify_all();
}

fn complete(shared: &Shared, id: u64, result: Outcome) {
    let mut state = shared.state.lock().unwrap();
    if let Some((_, entry)) = state.history.iter_mut().find(|(key, _)| *key == id) {
        entry.outcome = match &result {
            Ok(output) => history::Outcome::Completed {
                output: output.clone(),
            },
            Err(Error::Ended) => history::Outcome::Unanswered,
            Err(Error::Protocol { code, message }) => history::Outcome::ProtocolFailed {
                code: code.clone(),
                message: message.clone(),
            },
            Err(e) => history::Outcome::Rejected {
                code: e.code().into(),
                message: e.message().into(),
            },
        };
    }
    if !state.abandoned.remove(&id) {
        state.results.insert(id, result);
    }
    shared.changed.notify_all();
}
