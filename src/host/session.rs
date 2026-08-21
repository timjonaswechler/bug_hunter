//! Child-process lifecycle and synchronous JSONL protocol client.
//!
//! A Debug Host normally follows `spawn` → [`Session::ready`] → requests/waits →
//! [`Session::shutdown`]. Child stdout is protocol-only; stderr is streamed separately into the
//! rolling diagnostics buffer.

use super::{
    config::SessionConfig,
    diagnostics::{RecentLogs, stream_stderr},
    launch::LaunchSpec,
    recording::{
        self, Controller, Entry as RecordingEntry, Event as RecordingEvent, SessionContext,
        SessionOutcome,
    },
};
use crate::{
    AUTOMATION_CONTROL_ARTIFACT_DIR, Command, PROTOCOL_VERSION, Ready, Request, Response,
    ResponseStatus, observation::Request as ObservationRequest, time::Command as TimeCommand,
};
use serde_json::Value;
use std::{
    fmt,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command as ProcessCommand, Stdio},
    sync::mpsc,
    thread,
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Process, transport, protocol, request, or bounded-wait failure.
#[derive(Debug)]
pub enum DriverError {
    /// Child process could not be launched or piped.
    Launch(String),
    /// Host-side transport or Session Recording I/O failed.
    Io(String),
    /// Child JSONL violated the protocol contract.
    Protocol(String),
    /// Child status or exit was unsuccessful.
    Child(String),
    /// The child returned a protocol-level error response.
    RequestFailed(Response),
    /// An observation predicate remained false after the configured controlled frames.
    WaitLimitReached {
        /// Number of frames advanced before stopping.
        frame_limit: u64,
        /// Final observation result received.
        last_observation: Value,
    },
}

impl fmt::Display for DriverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Launch(message) => write!(formatter, "failed to start child: {message}"),
            Self::Io(message) => formatter.write_str(message),
            Self::Protocol(message) => write!(formatter, "protocol error: {message}"),
            Self::Child(message) => formatter.write_str(message),
            Self::RequestFailed(response) => write!(formatter, "request failed: {response:?}"),
            Self::WaitLimitReached { frame_limit, .. } => write!(
                formatter,
                "observation condition was not met within {frame_limit} controlled frames"
            ),
        }
    }
}

impl std::error::Error for DriverError {}

/// Host and child settings used when spawning a [`Session`].
///
/// `artifact_dir` is the host root for Session Recordings and driver artifacts.
/// `session_artifact_dir` is the child root supplied through
/// [`crate::AUTOMATION_CONTROL_ARTIFACT_DIR`]. It is not automatically nested beneath the host
/// root. If omitted, the child root falls back to `artifact_dir`, then to `artifacts`.
///
/// ```
/// use automation_control::driver::SessionOptions;
/// let options = SessionOptions::new()
///     .with_artifact_dir("artifacts/host")
///     .with_session_artifact_dir("artifacts/sessions/alpha");
/// assert_ne!(options.artifact_dir, options.session_artifact_dir);
/// ```
pub struct SessionOptions {
    /// Optional relative recording path to start during spawn.
    pub record: Option<PathBuf>,
    /// Shared rolling child-stderr diagnostics.
    pub recent_logs: RecentLogs,
    /// Host-side root for recordings and driver artifacts.
    pub artifact_dir: Option<PathBuf>,
    /// Child/session artifact root exported to the child process.
    pub session_artifact_dir: Option<PathBuf>,
    /// Recording context defaults; configure it through the provided builders.
    pub recording: recording::Options,
}

impl SessionOptions {
    /// Creates options with no automatic recording and default diagnostics.
    pub fn new() -> Self {
        Self {
            record: None,
            recent_logs: RecentLogs::default(),
            artifact_dir: None,
            session_artifact_dir: None,
            recording: recording::Options::default(),
        }
    }

    /// Builds options from session configuration and host-side artifact settings.
    ///
    /// This does not set a separate child root; call [`Self::with_session_artifact_dir`] when the
    /// Controlled Session must write artifacts elsewhere.
    pub fn from_config(
        _config: &SessionConfig,
        record: Option<PathBuf>,
        recent_logs: RecentLogs,
        artifact_dir: PathBuf,
    ) -> Self {
        Self::new()
            .with_record(record)
            .with_recent_logs(recent_logs)
            .with_artifact_dir(artifact_dir)
    }

    /// Selects an optional Session Recording path to start during spawn.
    pub fn with_record(mut self, path: Option<PathBuf>) -> Self {
        self.record = path;
        self
    }

    /// Replaces the rolling stderr diagnostics buffer.
    pub fn with_recent_logs(mut self, recent_logs: RecentLogs) -> Self {
        self.recent_logs = recent_logs;
        self
    }

    /// Sets the host root used for Session Recordings and driver artifacts.
    pub fn with_artifact_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.artifact_dir = Some(path.into());
        self
    }

    /// Sets the child artifact root exported through the environment.
    ///
    /// The driver sets the value but does not create this child root.
    pub fn with_session_artifact_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.session_artifact_dir = Some(path.into());
        self
    }

    /// Sets explicit Session Recording identity, expected mode, and opaque host configuration.
    pub fn with_recording_context(
        mut self,
        session_id: impl Into<String>,
        mode: crate::RunMode,
        configuration: Value,
    ) -> Self {
        self.recording.context = SessionContext::new(session_id, mode, configuration);
        self.recording.context_explicit = true;
        self
    }

    /// Sets the source-neutral Controller identity stored with recorded actions.
    pub fn with_controller(mut self, controller: Controller) -> Self {
        self.recording.controller = controller;
        self
    }
}

/// A live child Controlled Session owned by the Debug Host.
///
/// Dropping a live session terminates the child. If recording is active, drop writes an aborted
/// terminal event; [`Self::shutdown`] writes a clean completed terminal event.
pub struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    receiver: mpsc::Receiver<Result<Value, String>>,
    recording: recording::State,
    stderr_thread: Option<thread::JoinHandle<()>>,
    next_sequence: u64,
    clean_shutdown: bool,
}

impl Session {
    /// Builds and spawns the Cargo command described by `spec`.
    pub fn spawn(spec: &LaunchSpec, options: SessionOptions) -> Result<Self, DriverError> {
        Self::spawn_command(spec.command(), options)
    }

    /// Spawns an explicitly constructed child command with piped stdin, stdout, and stderr.
    ///
    /// Stdout must contain only protocol JSONL. Stderr is streamed separately. The configured
    /// child artifact root is exported but not created by the driver.
    pub fn spawn_command(
        mut command: ProcessCommand,
        options: SessionOptions,
    ) -> Result<Self, DriverError> {
        let artifact_root = options
            .artifact_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("artifacts"));
        let session_artifact_root = options
            .session_artifact_dir
            .unwrap_or_else(|| artifact_root.clone());
        command.env(AUTOMATION_CONTROL_ARTIFACT_DIR, &session_artifact_root);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| DriverError::Launch(error.to_string()))?;
        let Some(stdin) = child.stdin.take() else {
            terminate_child(&mut child);
            return Err(DriverError::Launch("child has no stdin".into()));
        };
        let Some(stdout) = child.stdout.take() else {
            terminate_child(&mut child);
            return Err(DriverError::Launch("child has no stdout".into()));
        };
        let Some(stderr) = child.stderr.take() else {
            terminate_child(&mut child);
            return Err(DriverError::Launch("child has no stderr".into()));
        };
        let receiver = json_reader(stdout);
        let recent_logs = options.recent_logs;
        let stderr_thread = thread::spawn(move || {
            if let Err(error) = stream_stderr(BufReader::new(stderr), io::stderr(), recent_logs) {
                eprintln!("automation-control: failed to read child stderr: {error}");
            }
        });
        let mut session = Self {
            child,
            stdin: Some(stdin),
            receiver,
            recording: recording::State {
                writer: None,
                context: options.recording.context,
                context_explicit: options.recording.context_explicit,
                context_written: false,
                ready: false,
                artifact_root,
                controller: options.recording.controller,
                host_sequence: 1,
            },
            stderr_thread: Some(stderr_thread),
            next_sequence: 1,
            clean_shutdown: false,
        };
        if let Some(path) = options.record
            && let Err(error) = session.start_recording(Some(path))
        {
            return Err(error);
        }
        Ok(session)
    }

    /// Consumes and validates the startup [`Ready`] handshake.
    ///
    /// Call this before normal requests. Protocol version is always checked; an explicitly
    /// configured recording context also requires the reported mode to match.
    pub fn ready(&mut self) -> Result<Ready, DriverError> {
        let value = match self.receive() {
            Ok(value) => value,
            Err(error) => return self.fail("ready_receive_failed", error),
        };
        let ready: Ready = match serde_json::from_value(value) {
            Ok(ready) => ready,
            Err(error) => {
                return self.fail(
                    "ready_parse_failed",
                    DriverError::Protocol(format!("invalid ready message: {error}")),
                );
            }
        };
        if ready.kind != "ready" || ready.version != PROTOCOL_VERSION {
            return self.fail(
                "ready_version_unsupported",
                DriverError::Protocol(format!("invalid ready message: {ready:?}")),
            );
        }
        if self.recording.context_explicit && ready.mode != self.recording.context.mode {
            return self.fail(
                "ready_mode_mismatch",
                DriverError::Protocol(format!(
                    "child reported mode {:?}, expected {:?}",
                    ready.mode, self.recording.context.mode
                )),
            );
        }
        if !self.recording.context_explicit {
            self.recording.context.mode = ready.mode;
            self.recording.context.protocol_version = ready.version;
        }
        self.recording.ready = true;
        self.ensure_recording_started()?;
        Ok(ready)
    }

    /// Sends one command with a host-assigned protocol sequence beginning at one.
    ///
    /// Response sequences and result/error shape are validated. Protocol error responses become
    /// [`DriverError::RequestFailed`]. Successful observations are recorded as observation events;
    /// other responses are recorded as game-response events.
    pub fn request(&mut self, command: Command) -> Result<Response, DriverError> {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        let observation = match &command {
            Command::Observe(request) => Some(request.clone()),
            _ => None,
        };
        let action = match recording::command_value(&command) {
            Ok(action) => action,
            Err(error) => {
                return self.fail(
                    "request_action_serialize_failed",
                    map_recording_error(error),
                );
            }
        };
        self.write_recording_event(RecordingEvent::ControllerAction {
            controller: self.recording.controller.clone(),
            action,
        })?;
        let request = Request { sequence, command };
        let serialized = match serde_json::to_string(&request) {
            Ok(serialized) => serialized,
            Err(error) => {
                return self.fail(
                    "request_serialize_failed",
                    DriverError::Protocol(format!(
                        "failed to serialize request {sequence}: {error}"
                    )),
                );
            }
        };
        let send_result = self
            .stdin
            .as_mut()
            .ok_or_else(|| DriverError::Io("child stdin is closed".into()))
            .and_then(|stdin| {
                writeln!(stdin, "{serialized}")
                    .and_then(|_| stdin.flush())
                    .map_err(|error| {
                        DriverError::Io(format!("failed to send sequence {sequence}: {error}"))
                    })
            });
        if let Err(error) = send_result {
            return self.fail("request_send_failed", error);
        }
        let response_value = match self.receive() {
            Ok(value) => value,
            Err(error) => return self.fail("response_receive_failed", error),
        };
        let response: Response = match serde_json::from_value(response_value) {
            Ok(response) => response,
            Err(error) => {
                return self.fail(
                    "response_parse_failed",
                    DriverError::Protocol(format!(
                        "invalid response for sequence {sequence}: {error}"
                    )),
                );
            }
        };
        if response.sequence != sequence {
            return self.fail(
                "response_sequence_mismatch",
                DriverError::Protocol(format!(
                    "expected response sequence {sequence}, got {response:?}"
                )),
            );
        }
        if response.status == ResponseStatus::Completed
            && (response.result.is_none() || response.error.is_some())
        {
            return self.fail(
                "response_payload_invalid",
                DriverError::Protocol(format!(
                    "completed response for sequence {sequence} must contain result and no error"
                )),
            );
        }
        if response.status == ResponseStatus::Error && response.error.is_none() {
            return self.fail(
                "response_payload_invalid",
                DriverError::Protocol(format!(
                    "error response for sequence {sequence} must contain error"
                )),
            );
        }

        if response.status == ResponseStatus::Completed {
            if let (Some(request), Some(result)) = (observation, response.result.clone()) {
                self.write_recording_event(RecordingEvent::Observation {
                    request_sequence: sequence,
                    request,
                    result,
                })?;
            } else {
                self.write_game_response(sequence, &response)?;
            }
        } else {
            self.write_game_response(sequence, &response)?;
            if let Some(error) = &response.error {
                self.write_recording_event(RecordingEvent::Error {
                    kind: error.code.clone(),
                    message: error.message.clone(),
                })?;
            }
            return Err(DriverError::RequestFailed(response));
        }
        Ok(response)
    }

    /// Replaces the context written at the start of the next recording segment.
    pub fn configure_recording(&mut self, configuration: Value) -> Result<(), DriverError> {
        if self.recording.writer.is_some() {
            return Err(DriverError::Io(
                "cannot change recording context while recording is active".into(),
            ));
        }
        self.recording.context.configuration = configuration;
        Ok(())
    }

    /// Starts a recording segment below the configured artifact root.
    ///
    /// A missing path allocates a collision-free `recordings/session-*.jsonl` path.
    pub fn start_recording(&mut self, path: Option<PathBuf>) -> Result<PathBuf, DriverError> {
        if self.recording.writer.is_some() {
            return Err(DriverError::Io(
                "session recording is already active".into(),
            ));
        }
        let writer = recording::Writer::create(&self.recording.artifact_root, path.as_deref())
            .map_err(map_recording_error)?;
        let path = writer.path().to_path_buf();
        self.recording.writer = Some(writer);
        self.recording.context_written = false;
        if self.recording.context_explicit || self.recording.ready {
            if let Err(error) = self.ensure_recording_started() {
                self.recording.writer = None;
                return Err(error);
            }
        }
        Ok(path)
    }

    /// Flushes the active segment with a terminal recording marker.
    pub fn stop_recording(&mut self) -> Result<PathBuf, DriverError> {
        let path = self
            .recording
            .writer
            .as_ref()
            .map(|writer| writer.path().to_path_buf())
            .ok_or_else(|| DriverError::Io("session recording is not active".into()))?;
        self.ensure_recording_started()?;
        self.write_recording_event(RecordingEvent::RecordingStopped)?;
        self.recording.writer = None;
        self.recording.context_written = false;
        Ok(path)
    }

    /// Returns the current host-side recording path, if a segment is active.
    pub fn active_recording_path(&self) -> Option<&Path> {
        self.recording.writer.as_ref().map(recording::Writer::path)
    }

    /// Records a source-neutral host action such as pause or resume.
    pub fn capture_controller_action(&mut self, action: Value) -> Result<(), DriverError> {
        self.write_recording_event(RecordingEvent::ControllerAction {
            controller: self.recording.controller.clone(),
            action,
        })
    }

    /// Records a bounded, sanitized host or session error without attaching child stderr.
    pub fn capture_error(
        &mut self,
        kind: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<(), DriverError> {
        self.write_recording_event(RecordingEvent::Error {
            kind: kind.into(),
            message: message.into(),
        })
    }

    /// Repeats an observation and advances one controlled frame after each miss.
    ///
    /// The predicate runs first against the current state. At most `limit` controlled frames are
    /// then advanced.
    pub fn wait_for_observation<F>(
        &mut self,
        request: ObservationRequest,
        limit: super::wait::FrameLimit,
        mut predicate: F,
    ) -> Result<Response, DriverError>
    where
        F: FnMut(&Value) -> bool,
    {
        let mut response = self.request(Command::Observe(request.clone()))?;
        for advanced_frames in 0..=limit.frames {
            let observation = response.result.as_ref().ok_or_else(|| {
                DriverError::Protocol("completed observation has no result".into())
            })?;
            if predicate(observation) {
                return Ok(response);
            }
            if advanced_frames == limit.frames {
                return Err(DriverError::WaitLimitReached {
                    frame_limit: limit.frames,
                    last_observation: observation.clone(),
                });
            }
            self.request(Command::Time(TimeCommand::advance(
                1,
                limit.step_nanoseconds,
            )))?;
            response = self.request(Command::Observe(request.clone()))?;
        }
        unreachable!("the bounded wait loop always returns")
    }

    /// Checks that the child is still running without exposing its process status to Controllers.
    pub fn ensure_running(&mut self) -> Result<(), DriverError> {
        match self.child.try_wait() {
            Ok(None) => Ok(()),
            Ok(Some(status)) => self.fail(
                "child_aborted",
                DriverError::Child(format!("child exited unexpectedly with {status}")),
            ),
            Err(error) => self.fail("child_status_failed", DriverError::Child(error.to_string())),
        }
    }

    /// Consumes the session, sends the shutdown command, and waits for a successful child exit.
    ///
    /// Do not send [`Command::Shutdown`] separately. The host waits until the child acknowledges
    /// shutdown and exits. A clean exit records `session_ended/completed`.
    pub fn shutdown(mut self) -> Result<(), DriverError> {
        self.request(Command::Shutdown)?;
        self.stdin.take();
        let status = self.child.wait().map_err(|error| {
            DriverError::Child(format!("failed to wait for child shutdown: {error}"))
        })?;
        if !status.success() {
            return self.fail(
                "shutdown_child_failed",
                DriverError::Child(format!("child exited {status}")),
            );
        }
        self.write_recording_event(RecordingEvent::SessionEnded {
            outcome: SessionOutcome::Completed,
        })?;
        self.recording.writer = None;
        self.recording.context_written = false;
        self.clean_shutdown = true;
        Ok(())
    }

    fn receive(&self) -> Result<Value, DriverError> {
        self.receiver
            .recv()
            .map_err(|_| DriverError::Protocol("child stdout closed".into()))?
            .map_err(DriverError::Protocol)
    }

    fn write_game_response(
        &mut self,
        request_sequence: u64,
        response: &Response,
    ) -> Result<(), DriverError> {
        self.write_recording_event(RecordingEvent::GameResponse {
            request_sequence,
            status: response.status,
            result: response.result.clone(),
            error: response.error.clone(),
        })?;
        if let Some(result) = response.result.as_ref()
            && let Some(artifact) = recording::ArtifactReference::from_result(result)
        {
            self.write_recording_event(RecordingEvent::Artifact {
                request_sequence,
                artifact,
            })?;
        }
        Ok(())
    }

    fn fail<T>(&mut self, kind: &str, error: DriverError) -> Result<T, DriverError> {
        self.capture_driver_failure(kind, &error);
        Err(error)
    }

    fn capture_driver_failure(&mut self, kind: &str, error: &DriverError) {
        let _ = self.write_recording_event(RecordingEvent::Error {
            kind: kind.into(),
            message: error.to_string(),
        });
    }

    fn ensure_recording_started(&mut self) -> Result<(), DriverError> {
        if self.recording.writer.is_none() || self.recording.context_written {
            return Ok(());
        }
        let context = self.recording.context.clone();
        self.write_entry(RecordingEvent::SessionStarted { context })?;
        self.recording.context_written = true;
        Ok(())
    }

    fn write_recording_event(&mut self, event: RecordingEvent) -> Result<(), DriverError> {
        if !matches!(event, RecordingEvent::SessionStarted { .. }) {
            self.ensure_recording_started()?;
        }
        self.write_entry(event)
    }

    fn write_entry(&mut self, event: RecordingEvent) -> Result<(), DriverError> {
        let next_sequence = self
            .recording
            .host_sequence
            .checked_add(1)
            .ok_or_else(|| DriverError::Io("recording host sequence is exhausted".into()))?;
        let entry = RecordingEntry {
            version: recording::FORMAT_VERSION,
            sequence: self.recording.host_sequence,
            event: recording::sanitize_event(event),
        };
        self.recording.host_sequence = next_sequence;
        if let Some(writer) = &mut self.recording.writer {
            writer.write(&entry).map_err(map_recording_error)?;
        }
        Ok(())
    }
}

fn map_recording_error(error: recording::Error) -> DriverError {
    DriverError::Io(error.to_string())
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.clean_shutdown && self.recording.writer.is_some() {
            let _ = self.ensure_recording_started();
            let _ = self.write_recording_event(RecordingEvent::Error {
                kind: "session_aborted".into(),
                message: "Controlled Session ended without a clean shutdown".into(),
            });
            let _ = self.write_recording_event(RecordingEvent::SessionEnded {
                outcome: SessionOutcome::Aborted,
            });
            self.recording.writer = None;
            self.recording.context_written = false;
        }
        self.stdin.take();
        if self.child.try_wait().ok().flatten().is_none() {
            terminate_child(&mut self.child);
        }
        if self.clean_shutdown {
            if let Some(thread) = self.stderr_thread.take() {
                let _ = thread.join();
            }
        } else {
            self.stderr_thread.take();
        }
    }
}

fn terminate_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    #[cfg(unix)]
    {
        let process_group = format!("-{}", child.id());
        let _ = ProcessCommand::new("kill")
            .args(["-KILL", &process_group])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn json_reader(stdout: ChildStdout) -> mpsc::Receiver<Result<Value, String>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let value = line.map_err(|error| error.to_string()).and_then(|line| {
                serde_json::from_str(&line)
                    .map_err(|error| format!("non-JSON stdout {line:?}: {error}"))
            });
            if sender.send(value).is_err() {
                return;
            }
        }
        let _ = sender.send(Err("child stdout closed".into()));
    });
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::Command as ProcessCommand,
        time::{Duration, Instant},
    };

    #[test]
    fn reports_a_child_that_exits_while_the_host_is_idle() {
        let mut command = ProcessCommand::new("sh");
        command.args([
            "-c",
            "printf '%s\\n' '{\"type\":\"ready\",\"version\":2,\"mode\":\"logical\",\"controls\":[],\"observation_scopes\":[]}'",
        ]);
        let mut session = Session::spawn_command(command, SessionOptions::new()).unwrap();
        session.ready().unwrap();

        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match session.ensure_running() {
                Err(DriverError::Child(message)) => {
                    assert!(message.contains("exited unexpectedly"));
                    break;
                }
                Ok(()) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                result => panic!("unexpected child health result: {result:?}"),
            }
        }
    }

    #[test]
    fn rejects_non_json_child_stdout_as_a_protocol_error() {
        let mut command = ProcessCommand::new("sh");
        command.args(["-c", "printf 'human log on stdout\\n'"]);
        let mut session = Session::spawn_command(command, SessionOptions::new()).unwrap();
        let error = session.ready().unwrap_err().to_string();
        assert!(
            error.contains("non-JSON stdout"),
            "unexpected error: {error}"
        );
    }
}
