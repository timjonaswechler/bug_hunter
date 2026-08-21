use super::{
    Config, RecentLogs,
    controller::{
        Action, Button, ControllerError, ControllerSession, KeyboardAction, Mode, Observation,
        PointerAction,
    },
    recording::Controller,
};
use crate::{screenshot::Command as ScreenshotCommand, time::MAX_FRAMES};
use serde::Deserialize;
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

pub(crate) const INVALID_SCRIPT_EXIT: i32 = 2;
pub(crate) const TIMEOUT_EXIT: i32 = 3;
pub(crate) const ACTION_EXIT: i32 = 4;
pub(crate) const EXPECTATION_EXIT: i32 = 5;
const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Script {
    version: u32,
    session: SessionConfiguration,
    steps: Vec<Step>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionConfiguration {
    mode: ScriptMode,
    #[serde(default)]
    record: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ScriptMode {
    Logical,
    Rendered,
}

impl From<ScriptMode> for Mode {
    fn from(value: ScriptMode) -> Self {
        match value {
            ScriptMode::Logical => Self::Logical,
            ScriptMode::Rendered => Self::Rendered,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Step {
    Click {
        target: String,
    },
    Pointer {
        action: PointerStep,
    },
    Keyboard {
        action: KeyboardStep,
    },
    Text {
        text: String,
    },
    Wait {
        condition: Condition,
        max_frames: u64,
    },
    Expect {
        condition: Condition,
    },
    Screenshot {
        path: String,
        expect: ScreenshotExpectation,
        #[serde(default)]
        rendered_only: bool,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum PointerStep {
    Move { x: f32, y: f32 },
    Press { button: ScriptButton },
    Release { button: ScriptButton },
    Click { button: ScriptButton },
    Scroll { x: f32, y: f32 },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum KeyboardStep {
    Press { key: String },
    Release { key: String },
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ScriptButton {
    Left,
    Right,
    Middle,
}

impl From<ScriptButton> for Button {
    fn from(value: ScriptButton) -> Self {
        match value {
            ScriptButton::Left => Self::Left,
            ScriptButton::Right => Self::Right,
            ScriptButton::Middle => Self::Middle,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Condition {
    Screen { equals: String },
}

impl Condition {
    const fn observation(&self) -> Observation {
        match self {
            Self::Screen { .. } => Observation::ActiveScreen,
        }
    }

    fn matches(&self, result_field: &str, actual: &Value) -> bool {
        match self {
            Self::Screen { equals } => actual[result_field] == *equals,
        }
    }

    fn expected(&self, result_field: &str) -> Value {
        match self {
            Self::Screen { equals } => Value::Object(serde_json::Map::from_iter([(
                result_field.to_owned(),
                Value::String(equals.clone()),
            )])),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScreenshotExpectation {
    #[serde(default)]
    mime_type: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

impl ScreenshotExpectation {
    fn expected(&self, path: &str) -> Value {
        let mut expected = serde_json::Map::from_iter([
            ("type".into(), Value::String("screenshot".into())),
            ("path".into(), Value::String(path.into())),
        ]);
        if let Some(mime_type) = &self.mime_type {
            expected.insert("mime_type".into(), Value::String(mime_type.clone()));
        }
        if let Some(width) = self.width {
            expected.insert("width".into(), Value::from(width));
        }
        if let Some(height) = self.height {
            expected.insert("height".into(), Value::from(height));
        }
        Value::Object(expected)
    }

    fn matches(&self, path: &str, actual: &Value) -> bool {
        actual["type"] == "screenshot"
            && actual["path"] == path
            && self
                .mime_type
                .as_ref()
                .is_none_or(|expected| actual["mime_type"] == *expected)
            && self
                .width
                .is_none_or(|expected| actual["width"] == expected)
            && self
                .height
                .is_none_or(|expected| actual["height"] == expected)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ErrorKind {
    InvalidScript,
    Timeout,
    Action,
    Expectation,
}

#[derive(Debug)]
pub(crate) struct Error {
    kind: ErrorKind,
    script: PathBuf,
    step: Option<usize>,
    message: String,
    expected: Option<Value>,
    actual: Option<Value>,
    last_observation: Option<(String, Value)>,
}

impl Error {
    fn invalid(script: &Path, message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::InvalidScript,
            script: script.to_path_buf(),
            step: None,
            message: message.into(),
            expected: None,
            actual: None,
            last_observation: None,
        }
    }

    fn at_step(kind: ErrorKind, script: &Path, step: usize, message: impl Into<String>) -> Self {
        Self {
            kind,
            script: script.to_path_buf(),
            step: Some(step),
            message: message.into(),
            expected: None,
            actual: None,
            last_observation: None,
        }
    }

    pub(crate) const fn exit_code(&self) -> i32 {
        match self.kind {
            ErrorKind::InvalidScript => INVALID_SCRIPT_EXIT,
            ErrorKind::Timeout => TIMEOUT_EXIT,
            ErrorKind::Action => ACTION_EXIT,
            ErrorKind::Expectation => EXPECTATION_EXIT,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.script.display())?;
        if let Some(step) = self.step {
            write!(formatter, ": step {step} ($.steps[{}])", step - 1)?;
        }
        write!(formatter, ": {}", self.message)?;
        if let Some(expected) = &self.expected {
            write!(formatter, "\nexpected: {expected}")?;
        }
        if let Some(actual) = &self.actual {
            write!(formatter, "\nactual: {actual}")?;
        }
        if let Some((name, observation)) = &self.last_observation {
            write!(
                formatter,
                "\nlast stable observation ({name}): {observation}"
            )?;
        }
        if let Some(artifact) = self
            .actual
            .as_ref()
            .and_then(|actual| actual.get("path").map(|_| actual))
        {
            write!(formatter, "\nartifact: {artifact}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub(crate) struct Summary {
    pub(crate) completed: usize,
    pub(crate) skipped: usize,
    pub(crate) mode: Mode,
}

pub(crate) fn run(
    profile: &Config,
    script_path: &Path,
    mode_override: Option<Mode>,
    artifact_dir: PathBuf,
    record_override: Option<PathBuf>,
    recent_logs: RecentLogs,
) -> Result<Summary, Error> {
    let source = fs::read_to_string(script_path).map_err(|error| {
        Error::invalid(
            script_path,
            format!("could not read Session Script: {error}"),
        )
    });
    let document = source
        .as_ref()
        .ok()
        .and_then(|source| serde_json::from_str::<Value>(source).ok());
    let script = source.and_then(|source| {
        let script: Script = serde_json::from_str(&source).map_err(|error| {
            Error::invalid(
                script_path,
                format!(
                    "invalid Session Script at line {}, column {}: {error}",
                    error.line(),
                    error.column()
                ),
            )
        })?;
        validate(script_path, &script)?;
        Ok(script)
    });

    let configured_mode = script
        .as_ref()
        .ok()
        .map(|script| script.session.mode.into())
        .or_else(|| document.as_ref().and_then(mode_from_document));
    let mode = mode_override.or(configured_mode).unwrap_or(Mode::Logical);
    let configured_record = script
        .as_ref()
        .ok()
        .and_then(|script| script.session.record.clone())
        .or_else(|| document.as_ref().and_then(record_from_document));
    let session = ControllerSession::start(
        profile,
        mode,
        artifact_dir,
        record_override.or(configured_record),
        recent_logs,
        Controller::new("script"),
    );
    let (script, mut session) = match (script, session) {
        (Err(error), Ok(mut session)) => {
            session.capture_script_error(error.kind_name());
            let _ = session.shutdown();
            return Err(error);
        }
        (Err(error), Err(_)) => return Err(error),
        (Ok(_), Err(error)) => {
            return Err(Error::at_step(
                ErrorKind::Action,
                script_path,
                1,
                error.to_string(),
            ));
        }
        (Ok(script), Ok(session)) => (script, session),
    };

    let execution = execute_steps(
        script_path,
        &script.steps,
        mode,
        &profile.screen.result_field,
        &mut session,
    );
    if let Err(error) = &execution {
        session.capture_script_error(error.kind_name());
    }
    let shutdown = session.shutdown();
    match (execution, shutdown) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(Error::at_step(
            ErrorKind::Action,
            script_path,
            script.steps.len().max(1),
            error.to_string(),
        )),
        (Ok((completed, skipped)), Ok(())) => Ok(Summary {
            completed,
            skipped,
            mode,
        }),
    }
}

fn mode_from_document(document: &Value) -> Option<Mode> {
    match document.pointer("/session/mode").and_then(Value::as_str) {
        Some("logical") => Some(Mode::Logical),
        Some("rendered") => Some(Mode::Rendered),
        _ => None,
    }
}

fn record_from_document(document: &Value) -> Option<PathBuf> {
    document
        .pointer("/session/record")
        .and_then(Value::as_str)
        .map(PathBuf::from)
}

impl Error {
    const fn kind_name(&self) -> &'static str {
        match self.kind {
            ErrorKind::InvalidScript => "invalid_session_script",
            ErrorKind::Timeout => "session_script_timeout",
            ErrorKind::Action => "session_script_action_failed",
            ErrorKind::Expectation => "session_script_expectation_failed",
        }
    }
}

fn validate(path: &Path, script: &Script) -> Result<(), Error> {
    if script.version != FORMAT_VERSION {
        return Err(Error::invalid(
            path,
            format!(
                "unsupported format version {} at $.version; expected {FORMAT_VERSION}",
                script.version
            ),
        ));
    }
    if script.steps.is_empty() {
        return Err(Error::invalid(
            path,
            "$.steps must contain at least one step",
        ));
    }
    for (index, step) in script.steps.iter().enumerate() {
        if let Step::Wait { max_frames, .. } = step
            && (*max_frames == 0 || *max_frames > MAX_FRAMES)
        {
            return Err(Error::invalid(
                path,
                format!("$.steps[{index}].max_frames must be between 1 and {MAX_FRAMES}"),
            ));
        }
    }
    Ok(())
}

fn execute_steps(
    path: &Path,
    steps: &[Step],
    mode: Mode,
    screen_result_field: &str,
    session: &mut ControllerSession,
) -> Result<(usize, usize), Error> {
    let initial = session
        .observe(Observation::ActiveScreen)
        .map_err(|error| action_error(path, 1, error, None))?;
    let mut last_observation = Some((Observation::ActiveScreen.as_str().into(), initial));
    let mut completed = 0;
    let mut skipped = 0;

    for (index, step) in steps.iter().enumerate() {
        let position = index + 1;
        match step {
            Step::Click { target } => session
                .activate_target(target)
                .map_err(|error| action_error(path, position, error, last_observation.clone()))?,
            Step::Pointer { action } => {
                let action = match action {
                    PointerStep::Move { x, y } => PointerAction::Move { x: *x, y: *y },
                    PointerStep::Press { button } => PointerAction::Press((*button).into()),
                    PointerStep::Release { button } => PointerAction::Release((*button).into()),
                    PointerStep::Click { button } => PointerAction::Click((*button).into()),
                    PointerStep::Scroll { x, y } => PointerAction::Scroll { x: *x, y: *y },
                };
                perform(
                    session,
                    Action::Pointer(action),
                    path,
                    position,
                    &last_observation,
                )?;
            }
            Step::Keyboard { action } => {
                let action = match action {
                    KeyboardStep::Press { key } => KeyboardAction::Press(key.clone()),
                    KeyboardStep::Release { key } => KeyboardAction::Release(key.clone()),
                };
                perform(
                    session,
                    Action::Keyboard(action),
                    path,
                    position,
                    &last_observation,
                )?;
            }
            Step::Text { text } => perform(
                session,
                Action::Text(text.clone()),
                path,
                position,
                &last_observation,
            )?,
            Step::Wait {
                condition,
                max_frames,
            } => {
                let observation = condition.observation();
                match session.wait_for(observation, *max_frames, |actual| {
                    condition.matches(screen_result_field, actual)
                }) {
                    Ok(actual) => {
                        last_observation = Some((observation.as_str().into(), actual));
                    }
                    Err(ControllerError::WaitLimitReached {
                        last_observation: actual,
                        ..
                    }) => {
                        return Err(Error {
                            kind: ErrorKind::Timeout,
                            script: path.to_path_buf(),
                            step: Some(position),
                            message: format!(
                                "wait condition was not met within {max_frames} controlled frames"
                            ),
                            expected: Some(condition.expected(screen_result_field)),
                            actual: Some(actual.clone()),
                            last_observation: Some((observation.as_str().into(), actual)),
                        });
                    }
                    Err(error) => {
                        return Err(action_error(
                            path,
                            position,
                            error,
                            last_observation.clone(),
                        ));
                    }
                }
            }
            Step::Expect { condition } => {
                let observation = condition.observation();
                let actual = session.observe(observation).map_err(|error| {
                    action_error(path, position, error, last_observation.clone())
                })?;
                last_observation = Some((observation.as_str().into(), actual.clone()));
                if !condition.matches(screen_result_field, &actual) {
                    return Err(Error {
                        kind: ErrorKind::Expectation,
                        script: path.to_path_buf(),
                        step: Some(position),
                        message: "expectation did not match".into(),
                        expected: Some(condition.expected(screen_result_field)),
                        actual: Some(actual),
                        last_observation,
                    });
                }
            }
            Step::Screenshot {
                path: artifact_path,
                expect,
                rendered_only,
            } => {
                if *rendered_only && mode == Mode::Logical {
                    skipped += 1;
                    continue;
                }
                let actual = session
                    .capture_screenshot(ScreenshotCommand::new(artifact_path))
                    .map_err(|error| {
                        action_error(path, position, error, last_observation.clone())
                    })?;
                let artifact = actual.get("artifact").cloned().unwrap_or(Value::Null);
                if !expect.matches(artifact_path, &artifact) {
                    return Err(Error {
                        kind: ErrorKind::Expectation,
                        script: path.to_path_buf(),
                        step: Some(position),
                        message: "screenshot artifact expectation did not match".into(),
                        expected: Some(expect.expected(artifact_path)),
                        actual: Some(artifact),
                        last_observation,
                    });
                }
            }
        }
        completed += 1;
    }
    Ok((completed, skipped))
}

fn perform(
    session: &mut ControllerSession,
    action: Action,
    path: &Path,
    step: usize,
    last_observation: &Option<(String, Value)>,
) -> Result<(), Error> {
    session
        .perform(action)
        .map(|_| ())
        .map_err(|error| action_error(path, step, error, last_observation.clone()))
}

fn action_error(
    path: &Path,
    step: usize,
    error: ControllerError,
    last_observation: Option<(String, Value)>,
) -> Error {
    let kind = if matches!(error, ControllerError::WaitLimitReached { .. }) {
        ErrorKind::Timeout
    } else {
        ErrorKind::Action
    };
    Error {
        kind,
        script: path.to_path_buf(),
        step: Some(step),
        message: error.to_string(),
        expected: None,
        actual: None,
        last_observation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Result<Script, serde_json::Error> {
        serde_json::from_str(source)
    }

    #[test]
    fn rejects_unknown_fields_with_a_source_position() {
        let error = parse(
            r#"{"version":1,"session":{"mode":"logical"},"steps":[{"type":"text","text":"hello","response":{}}]}"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("unknown field `response`"));
        assert!(error.line() > 0);
        assert!(error.column() > 0);
    }

    #[test]
    fn rejects_unknown_actions_with_a_source_position() {
        let error = parse(
            r#"{"version":1,"session":{"mode":"logical"},"steps":[{"type":"shell","command":"false"}]}"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("unknown variant `shell`"));
        assert!(error.line() > 0);
        assert!(error.column() > 0);
    }

    #[test]
    fn wait_limits_are_required_and_bounded() {
        let missing = parse(
            r#"{"version":1,"session":{"mode":"logical"},"steps":[{"type":"wait","condition":{"type":"screen","equals":"museum"}}]}"#,
        )
        .unwrap_err();
        assert!(missing.to_string().contains("max_frames"));

        let script = parse(
            r#"{"version":1,"session":{"mode":"logical"},"steps":[{"type":"wait","condition":{"type":"screen","equals":"museum"},"max_frames":0}]}"#,
        )
        .unwrap();
        let error = validate(Path::new("zero.json"), &script).unwrap_err();
        assert!(error.to_string().contains("$.steps[0].max_frames"));
    }

    #[test]
    fn screenshot_expectations_compare_selected_artifact_properties() {
        let expectation = ScreenshotExpectation {
            mime_type: Some("image/png".into()),
            width: Some(640),
            height: Some(360),
        };
        let artifact = json!({
            "type": "screenshot",
            "path": "screenshots/museum.png",
            "mime_type": "image/png",
            "width": 640,
            "height": 360
        });
        assert!(expectation.matches("screenshots/museum.png", &artifact));
        assert!(!expectation.matches(
            "screenshots/museum.png",
            &json!({"type": "screenshot", "path": "screenshots/museum.png", "mime_type": "image/png", "width": 1, "height": 360})
        ));
    }

    #[test]
    fn raw_virtual_input_actions_are_part_of_the_script_format() {
        let script = parse(
            r#"{"version":1,"session":{"mode":"logical"},"steps":[{"type":"pointer","action":{"type":"move","x":0.5,"y":0.25}},{"type":"pointer","action":{"type":"press","button":"left"}},{"type":"pointer","action":{"type":"release","button":"left"}},{"type":"pointer","action":{"type":"click","button":"right"}},{"type":"pointer","action":{"type":"scroll","x":0.0,"y":-1.0}},{"type":"keyboard","action":{"type":"press","key":"Escape"}},{"type":"keyboard","action":{"type":"release","key":"Escape"}},{"type":"text","text":"museum"}]}"#,
        )
        .unwrap();
        assert_eq!(script.steps.len(), 8);
    }
}
