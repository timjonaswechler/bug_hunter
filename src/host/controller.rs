use super::{
    Config, DriverError, RecentLogs, Session as DriverSession, SessionOptions,
    recording::Controller,
};
use crate::{
    Command as WireCommand, Handle, RunMode,
    keyboard::{Command as KeyboardCommand, Key},
    observation::{Projection, Request as ObservationRequest, Selector},
    pointer::{Button as WireButton, Command as PointerCommand},
    screenshot::Command as ScreenshotCommand,
    text::Command as TextCommand,
    time::{Command as TimeCommand, MAX_FRAMES},
};
use serde_json::{Value, json};
use std::{fmt, path::PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode {
    Logical,
    Rendered,
}

impl Mode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Logical => "logical",
            Self::Rendered => "rendered",
        }
    }

    const fn wire(self) -> RunMode {
        match self {
            Self::Logical => RunMode::Logical,
            Self::Rendered => RunMode::Rendered,
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SurfaceSize {
    width: f32,
    height: f32,
}

impl SurfaceSize {
    pub(crate) const fn new(width: u32, height: u32) -> Self {
        Self {
            width: width as f32,
            height: height as f32,
        }
    }

    fn position(self, x: f32, y: f32) -> Result<[f32; 2], ControllerError> {
        if !x.is_finite() || !y.is_finite() {
            return Err(ControllerError::Invalid(
                "pointer coordinates must be finite".into(),
            ));
        }
        if !(0.0..1.0).contains(&x) || !(0.0..1.0).contains(&y) {
            return Err(ControllerError::Invalid(
                "pointer coordinates must be normalized values in [0, 1)".into(),
            ));
        }
        Ok([x * self.width, y * self.height])
    }
}

fn session_configuration(
    profile: &Config,
    mode: Mode,
    surface: SurfaceSize,
    paused: bool,
) -> Value {
    json!({
        "profile_id": profile.profile_id,
        "mode": mode.as_str(),
        "surface": {"width": surface.width, "height": surface.height},
        "paused": paused,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Button {
    Left,
    Right,
    Middle,
}

impl Button {
    pub(crate) fn from_name(value: &str) -> Result<Self, String> {
        if value.eq_ignore_ascii_case("left") {
            Ok(Self::Left)
        } else if value.eq_ignore_ascii_case("right") {
            Ok(Self::Right)
        } else if value.eq_ignore_ascii_case("middle") {
            Ok(Self::Middle)
        } else {
            Err(format!(
                "unsupported pointer button {value:?}; expected left, right, or middle"
            ))
        }
    }

    const fn wire(self) -> WireButton {
        match self {
            Self::Left => WireButton::Primary,
            Self::Right => WireButton::Secondary,
            Self::Middle => WireButton::Middle,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PointerAction {
    Move { x: f32, y: f32 },
    Press(Button),
    Release(Button),
    Click(Button),
    Scroll { x: f32, y: f32 },
}

impl PointerAction {
    fn description(&self) -> String {
        match self {
            Self::Move { x, y } => format!("pointer move {x} {y}"),
            Self::Press(button) => format!("pointer press {}", button.as_str()),
            Self::Release(button) => format!("pointer release {}", button.as_str()),
            Self::Click(button) => format!("pointer click {}", button.as_str()),
            Self::Scroll { x, y } => format!("scroll {x} {y}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum KeyboardAction {
    Press(String),
    Release(String),
}

impl KeyboardAction {
    fn description(&self) -> String {
        match self {
            Self::Press(key) => format!("key {key} press"),
            Self::Release(key) => format!("key {key} release"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Action {
    Pointer(PointerAction),
    Keyboard(KeyboardAction),
    Text(String),
}

impl Action {
    fn description(&self) -> String {
        match self {
            Self::Pointer(action) => action.description(),
            Self::Keyboard(action) => action.description(),
            Self::Text(text) => format!("text {text}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Observation {
    Targets,
    Ui,
    Pointers,
    VirtualInput,
    Clock,
    ActiveScreen,
}

impl Observation {
    pub(crate) fn from_name(value: &str) -> Option<Self> {
        match value {
            "targets" => Some(Self::Targets),
            "ui" => Some(Self::Ui),
            "pointers" => Some(Self::Pointers),
            "input" => Some(Self::VirtualInput),
            "clock" => Some(Self::Clock),
            _ => None,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Targets => "targets",
            Self::Ui => "ui",
            Self::Pointers => "pointers",
            Self::VirtualInput => "input",
            Self::Clock => "clock",
            Self::ActiveScreen => "screen",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Status {
    pub(crate) instance: String,
    pub(crate) mode: Mode,
    pub(crate) active_screen: String,
    pub(crate) paused: bool,
    pub(crate) last_action: String,
}

#[derive(Debug)]
pub(crate) enum ControllerError {
    Launch(String),
    Communication(String),
    Child(String),
    Request {
        code: String,
        message: String,
    },
    Invalid(String),
    PausedWait,
    WaitLimitReached {
        frames: u64,
        last_observation: Value,
    },
    Shutdown,
}

impl fmt::Display for ControllerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Launch(message) => {
                write!(formatter, "could not start Controlled Session: {message}")
            }
            Self::Communication(message) => write!(
                formatter,
                "Controlled Session communication failed: {message}"
            ),
            Self::Child(message) => write!(formatter, "Controlled Session ended: {message}"),
            Self::Request { code, message } => write!(formatter, "{code}: {message}"),
            Self::Invalid(message) => formatter.write_str(message),
            Self::PausedWait => formatter.write_str(
                "wait condition is not met and the session is paused; use step or resume",
            ),
            Self::WaitLimitReached { frames, .. } => {
                write!(
                    formatter,
                    "wait condition was not met within {frames} controlled frames"
                )
            }
            Self::Shutdown => formatter.write_str("Controlled Session is already shut down"),
        }
    }
}

impl std::error::Error for ControllerError {}

impl ControllerError {
    pub(crate) const fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::Launch(_) | Self::Communication(_) | Self::Child(_) | Self::Shutdown
        )
    }
}

pub(crate) struct ControllerSession {
    driver: Option<DriverSession>,
    profile: Config,
    mode: Mode,
    surface: SurfaceSize,
    instance: String,
    paused: bool,
    last_action: String,
}

impl ControllerSession {
    pub(crate) fn start(
        profile: &Config,
        mode: Mode,
        artifact_dir: PathBuf,
        record: Option<PathBuf>,
        recent_logs: RecentLogs,
        controller: Controller,
    ) -> Result<Self, ControllerError> {
        let surface = SurfaceSize::new(
            profile.session.surface_width,
            profile.session.surface_height,
        );
        let mut session = Self::start_with_configuration(
            profile,
            mode,
            surface,
            artifact_dir,
            record,
            recent_logs,
            controller,
            session_configuration(profile, mode, surface, false),
            None,
        )?;
        session.advance(profile.session.startup_frames)?;
        Ok(session)
    }

    pub(crate) fn start_replay(
        profile: &Config,
        mode: Mode,
        artifact_dir: PathBuf,
        record: Option<PathBuf>,
        recent_logs: RecentLogs,
        controller: Controller,
        configuration: Value,
        session_artifact_dir: PathBuf,
    ) -> Result<Self, ControllerError> {
        let surface = SurfaceSize::new(
            profile.session.surface_width,
            profile.session.surface_height,
        );
        let mut configuration = configuration;
        if let Some(configuration) = configuration.as_object_mut() {
            configuration
                .entry("profile_id")
                .or_insert_with(|| Value::String(profile.profile_id.clone()));
        }
        Self::start_with_configuration(
            profile,
            mode,
            surface,
            artifact_dir,
            record,
            recent_logs,
            controller,
            configuration,
            Some(session_artifact_dir),
        )
    }

    fn start_with_configuration(
        profile: &Config,
        mode: Mode,
        surface: SurfaceSize,
        artifact_dir: PathBuf,
        record: Option<PathBuf>,
        recent_logs: RecentLogs,
        controller: Controller,
        configuration: Value,
        session_artifact_dir: Option<PathBuf>,
    ) -> Result<Self, ControllerError> {
        let paused = configuration
            .get("paused")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut launch = profile.application.launch();
        launch
            .arguments
            .push(profile.application.mode_argument.clone());
        launch.arguments.push(mode.as_str().into());
        let mut options = SessionOptions::new()
            .with_recent_logs(recent_logs)
            .with_artifact_dir(artifact_dir)
            .with_record(record)
            .with_recording_context(&profile.session.id, mode.wire(), configuration)
            .with_controller(controller);
        if let Some(session_artifact_dir) = session_artifact_dir {
            options = options.with_session_artifact_dir(session_artifact_dir);
        }
        let mut driver = DriverSession::spawn(&launch, options).map_err(map_driver_error)?;
        let ready = driver.ready().map_err(map_driver_error)?;
        if ready.mode != mode.wire() {
            return Err(ControllerError::Communication(
                "child reported a different execution mode".into(),
            ));
        }
        Ok(Self {
            driver: Some(driver),
            profile: profile.clone(),
            mode,
            surface,
            instance: profile.session.id.clone(),
            paused,
            last_action: "none".into(),
        })
    }

    #[cfg(test)]
    fn from_driver(driver: DriverSession, mode: Mode) -> Self {
        let profile = test_profile();
        Self {
            driver: Some(driver),
            surface: SurfaceSize::new(
                profile.session.surface_width,
                profile.session.surface_height,
            ),
            instance: profile.session.id.clone(),
            profile,
            mode,
            paused: false,
            last_action: "none".into(),
        }
    }

    pub(crate) fn perform(&mut self, action: Action) -> Result<Value, ControllerError> {
        let description = action.description();
        let result = match action {
            Action::Pointer(action) => match action {
                PointerAction::Move { x, y } => {
                    let position = self.surface.position(x, y)?;
                    self.transition(PointerCommand::Move {
                        surface: None,
                        position,
                    })?
                }
                PointerAction::Press(button) => self.transition(PointerCommand::Press {
                    button: button.wire(),
                })?,
                PointerAction::Release(button) => self.transition(PointerCommand::Release {
                    button: button.wire(),
                })?,
                PointerAction::Click(button) => self.click(button)?,
                PointerAction::Scroll { x, y } => {
                    if !x.is_finite() || !y.is_finite() {
                        return Err(ControllerError::Invalid(
                            "scroll deltas must be finite".into(),
                        ));
                    }
                    self.transition(PointerCommand::Scroll { delta: [x, y] })?
                }
            },
            Action::Keyboard(action) => match action {
                KeyboardAction::Press(name) => {
                    let key = resolve_key(&name)?;
                    self.settle(WireCommand::Keyboard(KeyboardCommand::Press { key }))?
                }
                KeyboardAction::Release(name) => {
                    let key = resolve_key(&name)?;
                    self.settle(WireCommand::Keyboard(KeyboardCommand::Release { key }))?
                }
            },
            Action::Text(text) => self.settle(WireCommand::Text(TextCommand::new(text)))?,
        };
        self.last_action = description;
        Ok(result)
    }

    pub(crate) fn activate_target(&mut self, target: &str) -> Result<(), ControllerError> {
        let targets = self.observe_raw(Observation::Targets)?;
        let position = TargetView::named(&targets, target)?.center()?;

        self.transition(PointerCommand::Move {
            surface: None,
            position,
        })?;
        self.click(Button::Left)?;
        self.last_action = format!("click {target}");
        Ok(())
    }

    pub(crate) fn observe(&mut self, observation: Observation) -> Result<Value, ControllerError> {
        let value = self.observe_raw(observation)?;
        self.last_action = format!("observe {}", observation.as_str());
        Ok(value)
    }

    pub(crate) fn wait_for<F>(
        &mut self,
        observation: Observation,
        frame_limit: u64,
        mut predicate: F,
    ) -> Result<Value, ControllerError>
    where
        F: FnMut(&Value) -> bool,
    {
        if frame_limit > MAX_FRAMES {
            return Err(ControllerError::Invalid(format!(
                "wait frame limit must be at most {MAX_FRAMES}"
            )));
        }
        let mut value = self.observe_raw(observation)?;
        if predicate(&value) {
            return Ok(value);
        }
        if self.paused {
            return Err(ControllerError::PausedWait);
        }
        for _ in 0..frame_limit {
            self.advance(1)?;
            value = self.observe_raw(observation)?;
            if predicate(&value) {
                return Ok(value);
            }
        }
        Err(ControllerError::WaitLimitReached {
            frames: frame_limit,
            last_observation: value,
        })
    }

    pub(crate) fn pause(&mut self) -> Result<(), ControllerError> {
        self.driver_mut()?
            .capture_controller_action(json!({"type": "pause"}))
            .map_err(map_driver_error)?;
        self.paused = true;
        self.last_action = "pause".into();
        Ok(())
    }

    pub(crate) fn resume(&mut self) -> Result<(), ControllerError> {
        self.driver_mut()?
            .capture_controller_action(json!({"type": "resume"}))
            .map_err(map_driver_error)?;
        self.paused = false;
        self.last_action = "resume".into();
        Ok(())
    }

    pub(crate) fn capture_screenshot(
        &mut self,
        command: ScreenshotCommand,
    ) -> Result<Value, ControllerError> {
        let path = command.path.clone();
        let result = self.request(WireCommand::Screenshot(command))?;
        self.last_action = format!("screenshot {path}");
        Ok(result)
    }

    pub(crate) fn start_recording(
        &mut self,
        path: Option<PathBuf>,
    ) -> Result<PathBuf, ControllerError> {
        let configuration =
            session_configuration(&self.profile, self.mode, self.surface, self.paused);
        let driver = self.driver_mut()?;
        driver
            .configure_recording(configuration)
            .map_err(map_driver_error)?;
        driver.start_recording(path).map_err(map_driver_error)
    }

    pub(crate) fn stop_recording(&mut self) -> Result<PathBuf, ControllerError> {
        self.driver_mut()?
            .stop_recording()
            .map_err(map_driver_error)
    }

    pub(crate) fn capture_invalid_command(&mut self) {
        if let Some(driver) = &mut self.driver {
            let _ = driver.capture_error(
                "invalid_controller_command",
                "Controller command was invalid",
            );
        }
    }

    pub(crate) fn capture_script_error(&mut self, kind: &str) {
        if let Some(driver) = &mut self.driver {
            let _ = driver.capture_error(kind, "Session Script execution failed");
        }
    }

    pub(crate) fn capture_operation_error(&mut self, error: &ControllerError) {
        let (kind, message) = match error {
            ControllerError::Launch(_) => {
                ("session_launch_failed", "Controlled Session launch failed")
            }
            ControllerError::Communication(_) => (
                "session_communication_failed",
                "Controlled Session communication failed",
            ),
            ControllerError::Child(_) => (
                "session_child_ended",
                "Controlled Session child ended unexpectedly",
            ),
            ControllerError::Request { .. } => (
                "session_request_failed",
                "Controlled Session request failed",
            ),
            ControllerError::Invalid(_) => {
                ("invalid_controller_action", "Controller action was invalid")
            }
            ControllerError::PausedWait => (
                "paused_wait",
                "Observation wait was blocked while the session was paused",
            ),
            ControllerError::WaitLimitReached { .. } => (
                "wait_limit_reached",
                "Observation wait reached its frame limit",
            ),
            ControllerError::Shutdown => (
                "session_shutdown",
                "Controlled Session was already shut down",
            ),
        };
        if let Some(driver) = &mut self.driver {
            let _ = driver.capture_error(kind, message);
        }
    }

    pub(crate) fn step(&mut self, frames: u64) -> Result<(), ControllerError> {
        self.advance(frames)?;
        self.last_action = format!("step {frames}");
        Ok(())
    }

    pub(crate) fn status(&mut self) -> Result<Status, ControllerError> {
        let active_screen = self.active_screen()?;
        Ok(Status {
            instance: self.instance.clone(),
            mode: self.mode,
            active_screen,
            paused: self.paused,
            last_action: self.last_action.clone(),
        })
    }

    pub(crate) fn ensure_running(&mut self) -> Result<(), ControllerError> {
        self.driver_mut()?
            .ensure_running()
            .map_err(map_driver_error)
    }

    pub(crate) fn replay_command(
        &mut self,
        command: WireCommand,
    ) -> Result<crate::Response, ControllerError> {
        match self.driver_mut()?.request(command) {
            Ok(response) => Ok(response),
            Err(DriverError::RequestFailed(response)) => Ok(response),
            Err(error) => Err(map_driver_error(error)),
        }
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), ControllerError> {
        let driver = self.driver.take().ok_or(ControllerError::Shutdown)?;
        driver.shutdown().map_err(map_driver_error)
    }

    fn click(&mut self, button: Button) -> Result<Value, ControllerError> {
        self.transition(PointerCommand::Press {
            button: button.wire(),
        })?;
        self.transition(PointerCommand::Release {
            button: button.wire(),
        })
    }

    fn transition(&mut self, command: PointerCommand) -> Result<Value, ControllerError> {
        self.settle(WireCommand::Pointer(command))
    }

    fn settle(&mut self, command: WireCommand) -> Result<Value, ControllerError> {
        let result = self.request(command)?;
        self.advance(1)?;
        Ok(result)
    }

    fn advance(&mut self, frames: u64) -> Result<Value, ControllerError> {
        if frames == 0 || frames > MAX_FRAMES {
            return Err(ControllerError::Invalid(format!(
                "step frames must be between 1 and {MAX_FRAMES}"
            )));
        }
        self.request(WireCommand::Time(TimeCommand::advance(
            frames,
            self.profile.session.frame_nanoseconds,
        )))
    }

    fn observe_raw(&mut self, observation: Observation) -> Result<Value, ControllerError> {
        if observation == Observation::ActiveScreen {
            let screen = self.active_screen()?;
            return Ok(Value::Object(serde_json::Map::from_iter([(
                self.profile.screen.result_field.clone(),
                Value::String(screen),
            )])));
        }
        let selector = match observation {
            Observation::Targets => Selector::Targets,
            Observation::Ui => Selector::Ui,
            Observation::Pointers => Selector::Pointers,
            Observation::VirtualInput => Selector::VirtualInput,
            Observation::Clock => Selector::Clock,
            Observation::ActiveScreen => unreachable!(),
        };
        self.request(WireCommand::Observe(ObservationRequest::new(
            selector,
            Projection::Summary,
        )))
    }

    fn active_screen(&mut self) -> Result<String, ControllerError> {
        let target = self.profile.screen.target.clone();
        let component = self.profile.screen.component.clone();
        let pointer = self.profile.screen.value_pointer.clone();
        let targets = self.observe_raw(Observation::Targets)?;
        let handle = TargetView::named(&targets, &target)?.handle()?;
        let result = self.request(WireCommand::Observe(ObservationRequest::new(
            Selector::Entity(handle),
            Projection::Components {
                type_paths: vec![component.clone()],
            },
        )))?;
        screen_value(&result, &component, &pointer)
    }

    fn request(&mut self, command: WireCommand) -> Result<Value, ControllerError> {
        let response = self
            .driver_mut()?
            .request(command)
            .map_err(map_driver_error)?;
        response
            .result
            .ok_or_else(|| ControllerError::Communication("child returned an empty result".into()))
    }

    fn driver_mut(&mut self) -> Result<&mut DriverSession, ControllerError> {
        self.driver.as_mut().ok_or(ControllerError::Shutdown)
    }
}

struct TargetView<'a> {
    value: &'a Value,
}

impl<'a> TargetView<'a> {
    fn named(observation: &'a Value, name: &str) -> Result<Self, ControllerError> {
        let value = observation
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| item.get("name").and_then(Value::as_str) == Some(name))
            })
            .ok_or_else(|| {
                ControllerError::Invalid(format!("configured target {name:?} is not available"))
            })?;
        Ok(Self { value })
    }

    fn center(&self) -> Result<[f32; 2], ControllerError> {
        let bounds = self
            .value
            .get("bounds")
            .ok_or_else(|| ControllerError::Invalid("configured target has no bounds".into()))?;
        let number = |field: &str| {
            bounds.get(field).and_then(Value::as_f64).ok_or_else(|| {
                ControllerError::Invalid(format!("configured target has no numeric {field} bound"))
            })
        };
        Ok([
            (number("x")? + number("width")? / 2.0) as f32,
            (number("y")? + number("height")? / 2.0) as f32,
        ])
    }

    fn handle(&self) -> Result<Handle, ControllerError> {
        self.value
            .get("entity")
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
            .ok_or_else(|| ControllerError::Invalid("configured target handle is invalid".into()))
    }
}

fn screen_value(
    observation: &Value,
    component_name: &str,
    value_pointer: &str,
) -> Result<String, ControllerError> {
    let component = observation
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("components"))
        .and_then(|components| components.get(component_name))
        .ok_or_else(|| ControllerError::Invalid("screen observation is unavailable".into()))?;
    if component.get("status").and_then(Value::as_str) != Some("available") {
        return Err(ControllerError::Invalid(
            "screen observation is unavailable".into(),
        ));
    }
    component
        .get("value")
        .and_then(|value| value.pointer(value_pointer))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| ControllerError::Invalid("screen observation is invalid".into()))
}

fn resolve_key(name: &str) -> Result<Key, ControllerError> {
    let wire_name = normalize_key_name(name);
    let key: Key = serde_json::from_value(json!(wire_name))
        .map_err(|_| ControllerError::Invalid(format!("unsupported keyboard key {name:?}")))?;
    KeyboardCommand::Press { key: key.clone() }
        .validate()
        .map_err(|error| ControllerError::Invalid(error.to_string()))?;
    Ok(key)
}

fn normalize_key_name(name: &str) -> String {
    let compact = name
        .chars()
        .filter(|character| !matches!(character, '_' | '-' | ' '))
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();

    if compact
        .strip_prefix("digit")
        .is_some_and(|suffix| suffix.len() == 1 && suffix.as_bytes()[0].is_ascii_digit())
    {
        return format!("digit_{}", &compact["digit".len()..]);
    }
    for suffix in ["left", "right", "down", "up", "lock", "menu"] {
        if let Some(prefix) = compact.strip_suffix(suffix)
            && !prefix.is_empty()
        {
            return format!("{prefix}_{suffix}");
        }
    }
    compact
}

#[cfg(test)]
fn test_profile() -> Config {
    Config::parse(include_str!("../../tests/fixtures/host_profile.toml")).unwrap()
}

fn map_driver_error(error: DriverError) -> ControllerError {
    match error {
        DriverError::Launch(message) => ControllerError::Launch(message),
        DriverError::RequestFailed(response) => {
            if let Some(error) = response.error {
                ControllerError::Request {
                    code: error.code,
                    message: error.message,
                }
            } else {
                ControllerError::Communication("child rejected a request".into())
            }
        }
        DriverError::Child(message) => ControllerError::Child(message),
        DriverError::Io(message) => ControllerError::Communication(message),
        DriverError::Protocol(message) => ControllerError::Communication(message),
        DriverError::WaitLimitReached {
            frame_limit,
            last_observation,
        } => ControllerError::WaitLimitReached {
            frames: frame_limit,
            last_observation,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as ProcessCommand;

    fn shell_session(script: &str) -> DriverSession {
        let mut command = ProcessCommand::new("sh");
        command.args(["-c", script]);
        let mut driver = DriverSession::spawn_command(command, SessionOptions::new()).unwrap();
        driver.ready().unwrap();
        driver
    }

    #[test]
    fn normalized_pointer_coordinates_exclude_the_surface_edge() {
        let surface = SurfaceSize::new(640, 360);
        assert_eq!(surface.position(0.5, 0.25).unwrap(), [320.0, 90.0]);
        assert!(surface.position(1.0, 0.5).is_err());
        assert!(surface.position(-0.1, 0.5).is_err());
    }

    #[test]
    fn keyboard_names_are_case_insensitive_and_accept_common_word_forms() {
        for name in ["escape", "Escape", "ESCAPE"] {
            assert_eq!(resolve_key(name).unwrap(), Key::Escape);
        }
        for name in ["ArrowLeft", "arrow_left", "arrow-left", "aRrOwLeFt"] {
            assert_eq!(resolve_key(name).unwrap(), Key::ArrowLeft);
        }
        for (name, expected) in [
            ("A", Key::A),
            ("DIGIT0", Key::Digit0),
            ("Backquote", Key::Backquote),
            ("BracketLeft", Key::BracketLeft),
            ("CONTROL_RIGHT", Key::ControlRight),
            ("CAPS-LOCK", Key::CapsLock),
            ("ContextMenu", Key::ContextMenu),
            ("PAGE DOWN", Key::PageDown),
            ("F12", Key::F12),
        ] {
            assert_eq!(
                resolve_key(name).unwrap(),
                expected,
                "failed to parse {name}"
            );
        }
        assert!(resolve_key("Hyperdrive").is_err());
    }

    #[test]
    fn request_errors_hide_protocol_envelopes_and_sequences() {
        let driver = shell_session(
            r#"printf '%s\n' '{"type":"ready","version":2,"mode":"logical","controls":["pointer","time"],"observation_scopes":[]}'; read line; printf '%s\n' '{"sequence":1,"status":"error","error":{"code":"pointer_failed","message":"no pointer location"}}'; sleep 1"#,
        );
        let mut session = ControllerSession::from_driver(driver, Mode::Logical);
        let message = session
            .perform(Action::Pointer(PointerAction::Press(Button::Left)))
            .unwrap_err()
            .to_string();
        assert_eq!(message, "pointer_failed: no pointer location");
        assert!(!message.contains("sequence"));
        assert!(!message.contains("Response"));
        assert!(!message.contains("version"));
    }

    #[test]
    fn paused_wait_observes_once_without_advancing() {
        let driver = shell_session(
            r#"printf '%s\n' '{"type":"ready","version":2,"mode":"logical","controls":["time"],"observation_scopes":["clock"]}'; read line; printf '%s\n' '{"sequence":1,"status":"completed","result":{"items":[{"ready":false}]}}'; sleep 1"#,
        );
        let mut session = ControllerSession::from_driver(driver, Mode::Logical);
        session.pause().unwrap();
        let error = session
            .wait_for(Observation::Clock, 4, |value| {
                value["items"][0]["ready"] == true
            })
            .unwrap_err();
        assert!(matches!(error, ControllerError::PausedWait));
    }
}
