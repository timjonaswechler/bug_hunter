//! Cargo launch-target schema for child Controlled Sessions.

use serde::Deserialize;
use std::{env, process::Command};

/// Kind of Cargo target used for a Controlled Session.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchTargetKind {
    /// A `binary` target selected by `--bin`.
    #[default]
    Binary,
    /// An `example` target selected by `--example`.
    Example,
}

/// Declarative Cargo command for launching one child process.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LaunchSpec {
    /// Cargo package name passed to `-p`.
    pub package: String,
    /// Binary or example target kind; defaults to binary.
    #[serde(default)]
    pub kind: LaunchTargetKind,
    /// Cargo target name.
    pub target: String,
    /// Features passed to the launched target, distinct from this crate's host-side `driver` feature.
    #[serde(default)]
    pub features: Vec<String>,
    /// Child arguments placed after Cargo's `--` separator.
    #[serde(default)]
    pub arguments: Vec<String>,
}

impl LaunchSpec {
    /// Builds, but does not run or validate, the Cargo command for this target.
    ///
    /// Arguments in the specification are passed to the launched process after Cargo's `--`
    /// separator. Target and feature existence are checked only when Cargo executes.
    pub fn command(&self) -> Command {
        let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Command::new(cargo);
        command.args(["run", "-q", "-p", &self.package]);
        match self.kind {
            LaunchTargetKind::Binary => {
                command.args(["--bin", &self.target]);
            }
            LaunchTargetKind::Example => {
                command.args(["--example", &self.target]);
            }
        }
        if !self.features.is_empty() {
            command.arg("--features").arg(self.features.join(","));
        }
        command.arg("--").args(&self.arguments);
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_controlled_binary_command_from_a_launch_spec() {
        let spec = LaunchSpec {
            package: "bevy_example".into(),
            kind: LaunchTargetKind::Binary,
            target: "bevy_example".into(),
            features: vec!["automation".into()],
            arguments: vec![],
        };
        let command = spec.command();
        let args: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "run",
                "-q",
                "-p",
                "bevy_example",
                "--bin",
                "bevy_example",
                "--features",
                "automation",
                "--"
            ]
        );
    }

    #[test]
    fn omitted_kind_deserializes_to_a_binary_launch() {
        let spec: LaunchSpec = toml::from_str(
            r#"
            package = "app"
            target = "app"
            "#,
        )
        .unwrap();
        assert_eq!(spec.kind, LaunchTargetKind::Binary);

        let args: Vec<_> = spec
            .command()
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|values| values == ["--bin", "app"]));
        assert!(!args.iter().any(|value| value == "--example"));
    }

    #[test]
    fn explicit_example_is_a_supported_launch_target() {
        let spec = LaunchSpec {
            package: "automation_control".into(),
            kind: LaunchTargetKind::Example,
            target: "bevy_controller".into(),
            features: vec!["driver".into()],
            arguments: vec![],
        };
        let args: Vec<_> = spec
            .command()
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert!(
            args.windows(2)
                .any(|values| values == ["--example", "bevy_controller"])
        );
    }
}
