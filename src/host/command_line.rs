//! Shared command-line parser for running named Controlled Sessions or reporting artifacts.
//!
//! The literal first argument `report` is reserved for report generation; all other run names
//! remain opaque to this crate.

use std::{
    ffi::{OsStr, OsString},
    fmt,
    path::PathBuf,
};

/// Parses the Controller commands shared by automation tools.
///
/// Run names other than the reserved `report` command remain opaque strings so a consuming tool
/// can define them without making them part of this crate's interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandLine {
    /// Runs a consumer-defined Controlled Session.
    Run(RunOptions),
    /// Generates or publishes a report from an artifact directory.
    Report(ReportOptions),
}

/// Options for a consumer-defined Controlled Session run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOptions {
    /// Optional Debug Host configuration path.
    pub config_path: Option<PathBuf>,
    /// Opaque consumer-defined run name; `report` is reserved.
    pub scenario: String,
    /// Optional host artifact root, not a distinct child-session artifact root.
    pub artifact_dir: Option<PathBuf>,
    /// Optional relative Session Recording path.
    pub record: Option<PathBuf>,
}

/// Options for generating or publishing a failure report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportOptions {
    /// Retained for consistent global option parsing; report generation does not load it.
    pub config_path: Option<PathBuf>,
    /// Host artifact directory containing `failure.json`.
    pub artifact_dir: PathBuf,
    /// Whether to publish through GitHub rather than only writing a local draft.
    pub create: bool,
}

/// The usage text shared by command-line errors and consumers that need to print help.
pub const USAGE: &str = "usage: TOOL [--config PATH] SCENARIO [--artifact-dir PATH] [--record PATH] | report ARTIFACT_DIR [--create]";

/// Invalid shared command-line syntax.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandLineError(String);

impl fmt::Display for CommandLineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CommandLineError {}

impl CommandLine {
    /// Parses arguments after the executable name.
    ///
    /// `--config` is accepted before or after the run name. Other run options are parsed after the
    /// opaque run name, while report arguments follow the reserved `report ARTIFACT_DIR` form.
    pub fn parse<I, T>(args: I) -> Result<Self, CommandLineError>
    where
        I: IntoIterator<Item = T>,
        T: AsRef<OsStr>,
    {
        let mut config_path = None;
        let mut remaining = Vec::new();
        let mut args = args
            .into_iter()
            .map(|argument| argument.as_ref().to_os_string());
        while let Some(argument) = args.next() {
            if argument == OsStr::new("--config") {
                if config_path.is_some() {
                    return Err(error("--config may only be supplied once"));
                }
                let path = args
                    .next()
                    .ok_or_else(|| error("--config requires a path"))?;
                config_path = Some(PathBuf::from(path));
            } else {
                remaining.push(argument);
            }
        }

        let scenario = remaining
            .first()
            .ok_or_else(|| error(USAGE.to_string()))?
            .to_str()
            .ok_or_else(|| error("scenario name must be valid UTF-8"))?
            .to_owned();
        let arguments = remaining.into_iter().skip(1);
        if scenario == "report" {
            parse_report(config_path, arguments)
        } else {
            parse_run(config_path, scenario, arguments)
        }
    }
}

fn parse_run(
    config_path: Option<PathBuf>,
    scenario: String,
    arguments: impl Iterator<Item = OsString>,
) -> Result<CommandLine, CommandLineError> {
    let mut artifact_dir = None;
    let mut record = None;
    let mut arguments = arguments;
    while let Some(option) = arguments.next() {
        if option == OsStr::new("--artifact-dir") {
            if artifact_dir.is_some() {
                return Err(error("--artifact-dir may only be supplied once"));
            }
            artifact_dir = Some(PathBuf::from(required_value(
                &mut arguments,
                "--artifact-dir",
            )?));
        } else if option == OsStr::new("--record") {
            if record.is_some() {
                return Err(error("--record may only be supplied once"));
            }
            record = Some(PathBuf::from(required_value(&mut arguments, "--record")?));
        } else {
            return Err(error(format!(
                "unknown option {:?}",
                option.to_string_lossy()
            )));
        }
    }
    Ok(CommandLine::Run(RunOptions {
        config_path,
        scenario,
        artifact_dir,
        record,
    }))
}

fn parse_report(
    config_path: Option<PathBuf>,
    arguments: impl Iterator<Item = OsString>,
) -> Result<CommandLine, CommandLineError> {
    let mut arguments = arguments;
    let artifact_dir = arguments
        .next()
        .ok_or_else(|| error("usage: TOOL report ARTIFACT_DIR [--create]"))?;
    if artifact_dir == OsStr::new("--create") {
        return Err(error("report requires ARTIFACT_DIR before --create"));
    }
    let create = match arguments.next() {
        None => false,
        Some(option) if option == OsStr::new("--create") => true,
        Some(option) => {
            return Err(error(format!(
                "unknown report option {:?}",
                option.to_string_lossy()
            )));
        }
    };
    if let Some(option) = arguments.next() {
        if option == OsStr::new("--create") && create {
            return Err(error("--create may only be supplied once"));
        }
        return Err(error(format!(
            "unexpected report argument {:?}",
            option.to_string_lossy()
        )));
    }
    Ok(CommandLine::Report(ReportOptions {
        config_path,
        artifact_dir: PathBuf::from(artifact_dir),
        create,
    }))
}

fn required_value(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<OsString, CommandLineError> {
    arguments
        .next()
        .ok_or_else(|| error(format!("{option} requires a path")))
}

fn error(message: impl Into<String>) -> CommandLineError {
    CommandLineError(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> CommandLine {
        CommandLine::parse(arguments.iter().copied()).unwrap()
    }

    #[test]
    fn parses_run_options_with_config_before_or_after_scenario() {
        assert_eq!(
            parse(&[
                "--config",
                "before.toml",
                "logical",
                "--artifact-dir",
                "artifacts",
                "--record",
                "session.jsonl",
            ]),
            CommandLine::Run(RunOptions {
                config_path: Some(PathBuf::from("before.toml")),
                scenario: "logical".into(),
                artifact_dir: Some(PathBuf::from("artifacts")),
                record: Some(PathBuf::from("session.jsonl")),
            })
        );
        assert_eq!(
            parse(&[
                "logical",
                "--record",
                "session.jsonl",
                "--config",
                "after.toml",
            ]),
            CommandLine::Run(RunOptions {
                config_path: Some(PathBuf::from("after.toml")),
                scenario: "logical".into(),
                artifact_dir: None,
                record: Some(PathBuf::from("session.jsonl")),
            })
        );
    }

    #[test]
    fn leaves_scenario_names_opaque() {
        assert_eq!(
            parse(&["catalog-regression"]),
            CommandLine::Run(RunOptions {
                config_path: None,
                scenario: "catalog-regression".into(),
                artifact_dir: None,
                record: None,
            })
        );
    }

    #[test]
    fn parses_report_options_and_create_flag() {
        assert_eq!(
            parse(&["report", "artifacts/failure", "--create"]),
            CommandLine::Report(ReportOptions {
                config_path: None,
                artifact_dir: PathBuf::from("artifacts/failure"),
                create: true,
            })
        );
    }

    #[test]
    fn rejects_duplicate_and_unknown_options() {
        for arguments in [
            vec!["--config", "one.toml", "--config", "two.toml", "logical"],
            vec!["logical", "--artifact-dir", "one", "--artifact-dir", "two"],
            vec!["logical", "--record", "one", "--record", "two"],
            vec!["logical", "--unknown"],
            vec!["report", "artifacts", "--create", "--create"],
        ] {
            assert!(
                CommandLine::parse(arguments.clone()).is_err(),
                "{arguments:?}"
            );
        }
    }

    #[test]
    fn reports_missing_values_with_option_names() {
        assert_eq!(
            CommandLine::parse(["--config"]).unwrap_err().to_string(),
            "--config requires a path"
        );
        assert_eq!(
            CommandLine::parse(["logical", "--record"])
                .unwrap_err()
                .to_string(),
            "--record requires a path"
        );
        assert_eq!(
            CommandLine::parse(["report"]).unwrap_err().to_string(),
            "usage: TOOL report ARTIFACT_DIR [--create]"
        );
    }
}
