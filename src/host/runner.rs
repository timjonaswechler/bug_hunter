use super::{
    Config, RecentLogs,
    controller::{ControllerSession, Mode},
    github, recording, repl, replay, report, script,
};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Runs the generic Debug Host with a profile embedded by the thin application binary.
pub fn run_embedded(profile_source: &'static str) -> ! {
    let profile = match Config::parse(profile_source) {
        Ok(profile) => profile,
        Err(error) => {
            eprintln!("automation host: {error}");
            std::process::exit(1);
        }
    };
    let cli = match parse_cli(&profile, std::env::args_os()) {
        Ok(cli) => cli,
        Err(error) => error.exit(),
    };
    if let Err(error) = execute(&profile, cli) {
        eprintln!("{}: {error}", profile.tool.name);
        std::process::exit(error.exit_code());
    }
    std::process::exit(0)
}

#[derive(Debug, Parser)]
struct Cli {
    /// Controlled Session execution mode. Overrides a Session Script's configured mode.
    #[arg(long, value_enum)]
    mode: Option<ModeArgument>,

    /// Root for session diagnostics and artifacts.
    #[arg(long)]
    artifact_dir: Option<PathBuf>,

    /// Start Session Recording at this artifact-root-relative JSONL path.
    #[arg(long, global = true)]
    record: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ModeArgument {
    Logical,
    Rendered,
}

impl From<ModeArgument> for Mode {
    fn from(value: ModeArgument) -> Self {
        match value {
            ModeArgument::Logical => Self::Logical,
            ModeArgument::Rendered => Self::Rendered,
        }
    }
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Replay recorded Controller actions in a fresh Controlled Session.
    Replay { recording: PathBuf },

    /// Run a human-authored Session Script in a fresh Controlled Session.
    Run { script: PathBuf },

    /// Draft or publish a report from an existing failure artifact directory.
    Report {
        artifact_dir: PathBuf,
        #[arg(long)]
        create: bool,
        /// Print the artifact summary as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug)]
enum ExecuteError {
    General(String),
    Replay(replay::Error),
    Script(script::Error),
}

impl ExecuteError {
    const fn exit_code(&self) -> i32 {
        match self {
            Self::General(_) => 1,
            Self::Replay(error) => error.exit_code(),
            Self::Script(error) => error.exit_code(),
        }
    }
}

impl std::fmt::Display for ExecuteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::General(message) => formatter.write_str(message),
            Self::Replay(error) => error.fmt(formatter),
            Self::Script(error) => error.fmt(formatter),
        }
    }
}

fn parse_cli(
    profile: &Config,
    args: impl IntoIterator<Item = OsString>,
) -> Result<Cli, clap::Error> {
    let command = Cli::command()
        .name(profile.tool.name.clone())
        .about(profile.tool.about.clone());
    let matches = command.try_get_matches_from(args)?;
    Cli::from_arg_matches(&matches)
}

fn execute(profile: &Config, cli: Cli) -> Result<(), ExecuteError> {
    let artifact_dir = cli
        .artifact_dir
        .unwrap_or_else(|| profile.tool.default_artifact_dir.clone());
    match cli.command {
        Some(CliCommand::Replay { recording }) => {
            replay_recording(profile, recording, artifact_dir, cli.record)
        }
        Some(CliCommand::Run { script }) => session_script(
            profile,
            script,
            cli.mode.map(Into::into),
            artifact_dir,
            cli.record,
        ),
        Some(CliCommand::Report {
            artifact_dir,
            create,
            json,
        }) => report(profile, artifact_dir, create, json).map_err(ExecuteError::General),
        None => controlled_repl(
            profile,
            cli.mode
                .map(Into::into)
                .unwrap_or_else(|| match profile.session.default_mode {
                    super::DefaultMode::Logical => Mode::Logical,
                    super::DefaultMode::Rendered => Mode::Rendered,
                }),
            artifact_dir,
            cli.record,
        )
        .map_err(ExecuteError::General),
    }
}

fn controlled_repl(
    profile: &Config,
    mode: Mode,
    artifact_dir: PathBuf,
    record: Option<PathBuf>,
) -> Result<(), String> {
    let recent_logs = RecentLogs::default();
    let interrupted = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&interrupted);
    ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst))
        .map_err(|error| format!("could not install Ctrl-C handler: {error}"))?;

    let diagnostic_record_path = record
        .as_ref()
        .and_then(|path| recording::path_below_artifact_root(&artifact_dir, path).ok());
    let result = ControllerSession::start(
        profile,
        mode,
        artifact_dir.clone(),
        record,
        recent_logs.clone(),
        recording::Controller::new("repl"),
    )
    .and_then(|session| repl::run(session, interrupted))
    .map_err(|error| error.to_string());

    let cli_error = result.as_ref().err().map(String::as_str);
    persist_failure(
        &recent_logs,
        &artifact_dir,
        cli_error,
        diagnostic_record_path.as_deref(),
    );
    result
}

fn replay_recording(
    profile: &Config,
    recording: PathBuf,
    artifact_dir: PathBuf,
    record: Option<PathBuf>,
) -> Result<(), ExecuteError> {
    let recent_logs = RecentLogs::default();
    let summary = replay::run(profile, &recording, artifact_dir, record, recent_logs)
        .map_err(ExecuteError::Replay)?;
    println!(
        "Session Replay passed: {} actions, mode={}",
        summary.actions, summary.mode
    );
    Ok(())
}

fn session_script(
    profile: &Config,
    path: PathBuf,
    mode: Option<Mode>,
    artifact_dir: PathBuf,
    record: Option<PathBuf>,
) -> Result<(), ExecuteError> {
    let recent_logs = RecentLogs::default();
    let diagnostic_record_path = record
        .as_ref()
        .and_then(|path| recording::path_below_artifact_root(&artifact_dir, path).ok());
    let result = script::run(
        profile,
        &path,
        mode,
        artifact_dir.clone(),
        record,
        recent_logs.clone(),
    );
    let cli_error = result.as_ref().err().map(ToString::to_string);
    persist_failure(
        &recent_logs,
        &artifact_dir,
        cli_error.as_deref(),
        diagnostic_record_path.as_deref(),
    );
    let summary = result.map_err(ExecuteError::Script)?;
    println!(
        "Session Script passed: {} completed, {} skipped, mode={}",
        summary.completed, summary.skipped, summary.mode
    );
    Ok(())
}

fn persist_failure(
    recent_logs: &RecentLogs,
    artifact_dir: &std::path::Path,
    cli_error: Option<&str>,
    record_path: Option<&std::path::Path>,
) {
    if (cli_error.is_some() || recent_logs.failure().is_some())
        && let Err(error) =
            recent_logs.persist_failure_artifacts(artifact_dir, cli_error, record_path)
    {
        eprintln!("warning: could not save failure artifacts: {error}");
    }
}

fn report(profile: &Config, artifact_dir: PathBuf, create: bool, json: bool) -> Result<(), String> {
    if create {
        let report = github::Report::prepare(&artifact_dir, &profile.report)
            .map_err(|error| error.to_string())?;
        let outcome = report.publish().map_err(|error| error.to_string())?;
        println!(
            "{}",
            serde_json::to_string(&outcome).map_err(|error| error.to_string())?
        );
        return Ok(());
    }

    let report = report::Report::load(&artifact_dir)?;
    if json {
        println!("{}", report.json()?);
    } else {
        print!("{report}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Config {
        Config::parse(include_str!("../../tests/fixtures/host_profile.toml")).unwrap()
    }

    fn parse(args: &[&str]) -> Cli {
        parse_cli(&profile(), args.iter().map(OsString::from)).unwrap()
    }

    #[test]
    fn preserves_the_debug_host_command_line() {
        let cli = parse(&["star_sim_debug"]);
        assert_eq!(cli.mode, None);
        assert!(cli.artifact_dir.is_none());
        assert!(cli.command.is_none());

        let cli = parse(&["star_sim_debug", "--mode", "logical"]);
        assert_eq!(cli.mode, Some(ModeArgument::Logical));
        assert!(cli.command.is_none());
        assert!(
            parse_cli(
                &profile(),
                ["star_sim_debug", "logical"].map(OsString::from)
            )
            .is_err()
        );
    }

    #[test]
    fn preserves_run_and_report_subcommands() {
        let cli = parse(&["star_sim_debug", "run", "sessions/museum.json"]);
        assert!(matches!(
            cli.command,
            Some(CliCommand::Run { script })
                if script == std::path::Path::new("sessions/museum.json")
        ));

        let cli = parse(&["star_sim_debug", "report", "artifacts/failure", "--create"]);
        assert!(matches!(
            cli.command,
            Some(CliCommand::Report {
                artifact_dir,
                create: true,
                json: false,
            }) if artifact_dir == std::path::Path::new("artifacts/failure")
        ));
    }

    #[test]
    fn config_option_is_not_exposed() {
        assert!(
            parse_cli(
                &profile(),
                ["star_sim_debug", "--config", "other.toml"].map(OsString::from)
            )
            .is_err()
        );
    }
}
