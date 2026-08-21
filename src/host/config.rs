//! Versioned TOML configuration for a profile-driven Debug Host.

use super::launch::{LaunchSpec, LaunchTargetKind};
use crate::time::{MAX_FRAMES, MAX_STEP_NANOSECONDS};
use serde::Deserialize;
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

pub const CONFIG_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub profile_id: String,
    pub tool: ToolConfig,
    pub application: ApplicationConfig,
    pub session: SessionConfig,
    pub report: ReportConfig,
    pub screen: ScreenConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolConfig {
    pub name: String,
    pub about: String,
    pub default_artifact_dir: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationConfig {
    pub package: String,
    #[serde(default)]
    pub kind: LaunchTargetKind,
    pub target: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub mode_argument: String,
}

impl ApplicationConfig {
    pub fn launch(&self) -> LaunchSpec {
        LaunchSpec {
            package: self.package.clone(),
            kind: self.kind.clone(),
            target: self.target.clone(),
            features: self.features.clone(),
            arguments: self.arguments.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DefaultMode {
    Logical,
    Rendered,
}

impl Default for DefaultMode {
    fn default() -> Self {
        Self::Rendered
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionConfig {
    pub id: String,
    pub default_mode: DefaultMode,
    pub surface_width: u32,
    pub surface_height: u32,
    pub frame_nanoseconds: u64,
    pub startup_frames: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            id: "session".into(),
            default_mode: DefaultMode::Rendered,
            surface_width: 640,
            surface_height: 360,
            frame_nanoseconds: 16_666_667,
            startup_frames: 1,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportConfig {
    pub generated_by: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenConfig {
    pub target: String,
    pub component: String,
    pub value_pointer: String,
    pub result_field: String,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|error| ConfigError::Read {
            path: path.to_path_buf(),
            error,
        })?;
        Self::parse(&contents).map_err(|error| match error {
            ConfigError::Parse { error, .. } => ConfigError::Parse {
                path: path.to_path_buf(),
                error,
            },
            other => other,
        })
    }

    pub fn parse(contents: &str) -> Result<Self, ConfigError> {
        let config = toml::from_str::<Self>(contents).map_err(|error| ConfigError::Parse {
            path: PathBuf::from("<embedded>"),
            error: error.to_string(),
        })?;
        config.validate().map_err(ConfigError::Invalid)?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != CONFIG_VERSION {
            return Err(format!(
                "unsupported configuration version {}; expected {CONFIG_VERSION}",
                self.version
            ));
        }
        for (name, value) in [
            ("profile_id", self.profile_id.as_str()),
            ("tool.name", self.tool.name.as_str()),
            ("tool.about", self.tool.about.as_str()),
            ("application.package", self.application.package.as_str()),
            ("application.target", self.application.target.as_str()),
            (
                "application.mode_argument",
                self.application.mode_argument.as_str(),
            ),
            ("session.id", self.session.id.as_str()),
            ("screen.target", self.screen.target.as_str()),
            ("screen.component", self.screen.component.as_str()),
            ("screen.result_field", self.screen.result_field.as_str()),
        ] {
            require_value(name, value)?;
        }
        if self.tool.default_artifact_dir.as_os_str().is_empty() {
            return Err("tool.default_artifact_dir must not be empty".into());
        }
        if self.session.surface_width == 0 || self.session.surface_height == 0 {
            return Err("session surface dimensions must be positive".into());
        }
        if self.session.frame_nanoseconds == 0
            || self.session.frame_nanoseconds > MAX_STEP_NANOSECONDS
        {
            return Err(format!(
                "session.frame_nanoseconds must be between 1 and {MAX_STEP_NANOSECONDS}"
            ));
        }
        if self.session.startup_frames == 0 || self.session.startup_frames > MAX_FRAMES {
            return Err(format!(
                "session.startup_frames must be between 1 and {MAX_FRAMES}"
            ));
        }
        if !self.screen.value_pointer.starts_with('/') {
            return Err("screen.value_pointer must be a JSON pointer beginning with '/'".into());
        }
        if let Some(generated_by) = &self.report.generated_by {
            require_value("report.generated_by", generated_by)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Read {
        path: PathBuf,
        error: std::io::Error,
    },
    Parse {
        path: PathBuf,
        error: String,
    },
    Invalid(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, error } => {
                write!(
                    formatter,
                    "failed to read configuration {}: {error}",
                    path.display()
                )
            }
            Self::Parse { path, error } => {
                write!(formatter, "invalid TOML in {}: {error}", path.display())
            }
            Self::Invalid(message) => {
                write!(formatter, "invalid automation configuration: {message}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

fn require_value(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{name} must not be empty"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILE: &str = r#"
version = 1
profile_id = "test-v1"

[tool]
name = "test_debug"
about = "Control a test application"
default_artifact_dir = "artifacts/test"

[application]
package = "app"
target = "app"
features = ["automation-control"]
arguments = []
mode_argument = "--controlled-mode"

[session]
id = "alpha"
default_mode = "rendered"
surface_width = 640
surface_height = 360
frame_nanoseconds = 16666667
startup_frames = 1

[report]
generated_by = "test_debug report"

[screen]
target = "session.status"
component = "app::SessionObservation"
value_pointer = "/active_screen"
result_field = "active_screen"
"#;

    #[test]
    fn loads_and_validates_a_complete_profile() {
        let config = Config::parse(PROFILE).unwrap();
        assert_eq!(config.application.package, "app");
        assert_eq!(config.session.frame_nanoseconds, 16_666_667);
    }

    #[test]
    fn rejects_unknown_and_invalid_profile_fields() {
        assert!(Config::parse(&PROFILE.replace("version = 1", "version = 2")).is_err());
        assert!(
            Config::parse(&PROFILE.replace(
                "value_pointer = \"/active_screen\"",
                "value_pointer = \"active_screen\""
            ))
            .is_err()
        );
        assert!(
            Config::parse(&PROFILE.replace("profile_id = \"test-v1\"", "profile_id = \"\""))
                .is_err()
        );
        assert!(Config::parse(&format!("{PROFILE}\nunexpected = true\n")).is_err());
    }

    #[test]
    fn rejects_controlled_time_values_above_protocol_limits() {
        assert!(
            Config::parse(&PROFILE.replace(
                "frame_nanoseconds = 16666667",
                &format!("frame_nanoseconds = {}", MAX_STEP_NANOSECONDS + 1)
            ))
            .is_err()
        );
        assert!(
            Config::parse(&PROFILE.replace(
                "startup_frames = 1",
                &format!("startup_frames = {}", MAX_FRAMES + 1)
            ))
            .is_err()
        );
    }
}
