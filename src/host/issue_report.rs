//! Portable Markdown issue drafts built from Debug Host failure artifacts.

use super::diagnostics::{DiagnosticsError, FailureReport};
use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
};

const FAILURE_JSON_NAME: &str = "failure.json";
const RECENT_LOG_NAME: &str = "recent.log";
const SESSION_RECORD_NAME: &str = "session.jsonl";
const ISSUE_DRAFT_NAME: &str = "github-issue.md";
const SESSION_TAIL_ENTRIES: usize = 12;
const ISSUE_TITLE_PREFIX: &str = "[automation failure] ";

/// A portable Markdown issue draft generated from an automation failure artifact set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueDraft {
    /// Generated issue title.
    pub title: String,
    /// Generated Markdown issue body.
    pub body: String,
}

impl IssueDraft {
    /// Loads required `failure.json` and optional `recent.log` beneath `artifact_dir`.
    ///
    /// If failure metadata supplies `record_path`, that path is read directly and need not be below
    /// `artifact_dir`; otherwise optional `session.jsonl` is read below it.
    pub fn from_artifacts(artifact_dir: impl AsRef<Path>) -> Result<Self, ReportError> {
        let artifact_dir = artifact_dir.as_ref();
        let failure = FailureReport::load(artifact_dir.join(FAILURE_JSON_NAME))?;
        Self::from_failure_report(&failure, artifact_dir)
    }

    /// Builds a draft from validated failure metadata and its artifact directory.
    ///
    /// Lookup order is `recent.log` below the directory, then the supplied `record_path` or the
    /// fallback `session.jsonl` below the directory.
    pub fn from_failure_report(
        failure: &FailureReport,
        artifact_dir: impl AsRef<Path>,
    ) -> Result<Self, ReportError> {
        let artifact_dir = artifact_dir.as_ref();
        failure.validate().map_err(|message| ReportError::Invalid {
            path: artifact_dir.join(FAILURE_JSON_NAME),
            message,
        })?;
        let recent = read_optional(&artifact_dir.join(RECENT_LOG_NAME))?;
        let session_path = failure
            .record_path
            .clone()
            .unwrap_or_else(|| artifact_dir.join(SESSION_RECORD_NAME));
        let session = read_optional(&session_path)?;
        let session_tail = session_tail(&session);
        let title = issue_title(&failure.message);
        let cli_error = failure.cli_error.as_deref().unwrap_or("none");
        let body = format!(
            "# {title}\n\n## Failure\n\n- Kind: `{}`\n- CLI error: `{cli_error}`\n\n## Recent Bevy log\n\n````text\n{recent}\n````\n\n## Last protocol activity\n\n````jsonl\n{session_tail}\n````\n",
            failure.kind,
        );
        Ok(Self { title, body })
    }

    /// Adds consumer-provided Markdown attribution without coupling this crate to a tool name.
    pub fn with_footer(mut self, footer: impl AsRef<str>) -> Self {
        let footer = footer.as_ref().trim();
        if !footer.is_empty() {
            if !self.body.ends_with('\n') {
                self.body.push('\n');
            }
            self.body.push('\n');
            self.body.push_str(footer);
            self.body.push('\n');
        }
        self
    }

    /// Creates `artifact_dir`, overwrites `github-issue.md` beneath it, and returns that path.
    pub fn write_to(&self, artifact_dir: impl AsRef<Path>) -> Result<PathBuf, ReportError> {
        let artifact_dir = artifact_dir.as_ref();
        fs::create_dir_all(artifact_dir)
            .map_err(|error| ReportError::io("create artifact directory", artifact_dir, error))?;
        let path = artifact_dir.join(ISSUE_DRAFT_NAME);
        fs::write(&path, &self.body)
            .map_err(|error| ReportError::io("write issue draft", &path, error))?;
        Ok(path)
    }
}

/// Errors produced while loading diagnostic artifacts or writing a Markdown draft.
#[derive(Debug)]
pub enum ReportError {
    /// Required failure metadata could not be loaded.
    Failure(DiagnosticsError),
    /// Optional artifact reading or draft writing failed.
    Io {
        /// Attempted operation.
        operation: &'static str,
        /// Affected path.
        path: PathBuf,
        /// Underlying I/O error.
        error: io::Error,
    },
    /// Supplied failure metadata was invalid.
    Invalid {
        /// Associated failure-report path.
        path: PathBuf,
        /// Validation diagnostic.
        message: String,
    },
}

impl ReportError {
    fn io(operation: &'static str, path: &Path, error: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.to_path_buf(),
            error,
        }
    }
}

impl From<DiagnosticsError> for ReportError {
    fn from(error: DiagnosticsError) -> Self {
        Self::Failure(error)
    }
}

impl fmt::Display for ReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failure(error) => write!(formatter, "failed to load failure report: {error}"),
            Self::Io {
                operation,
                path,
                error,
            } => write!(
                formatter,
                "failed to {operation} {}: {error}",
                path.display()
            ),
            Self::Invalid { path, message } => {
                write!(
                    formatter,
                    "invalid failure report {}: {message}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ReportError {}

fn issue_title(message: &str) -> String {
    let message = message
        .lines()
        .next()
        .unwrap_or("automation run failed")
        .trim();
    let mut title = format!("{ISSUE_TITLE_PREFIX}{message}");
    if title.chars().count() > 120 {
        title = title.chars().take(117).collect::<String>() + "...";
    }
    title
}

fn session_tail(session: &str) -> String {
    session
        .lines()
        .rev()
        .take(SESSION_TAIL_ENTRIES)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}

fn read_optional(path: &Path) -> Result<String, ReportError> {
    match fs::read_to_string(path) {
        Ok(value) => Ok(value),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(ReportError::io("read", path, error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "automation-control-report-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn write_failure(artifact_dir: &Path, record_path: Option<&Path>) {
        fs::create_dir_all(artifact_dir).unwrap();
        fs::write(
            artifact_dir.join(FAILURE_JSON_NAME),
            serde_json::to_vec_pretty(&json!({
                "version": 1,
                "kind": "panic",
                "message": "stellar catalog invariant violated",
                "cli_error": "child exited with status 101",
                "record_path": record_path,
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn generates_typed_draft_content_from_failure_artifacts() {
        let artifact_dir = temporary_dir("content");
        write_failure(&artifact_dir, None);
        fs::write(
            artifact_dir.join(RECENT_LOG_NAME),
            "ERROR star_sim::automation: stellar catalog invariant violated\n",
        )
        .unwrap();
        fs::write(
            artifact_dir.join(SESSION_RECORD_NAME),
            "first request\nlast request\n",
        )
        .unwrap();

        let draft = IssueDraft::from_artifacts(&artifact_dir).unwrap();

        assert_eq!(
            draft.title,
            "[automation failure] stellar catalog invariant violated"
        );
        assert!(draft.body.contains("- Kind: `panic`"));
        assert!(
            draft
                .body
                .contains("- CLI error: `child exited with status 101`")
        );
        assert!(
            draft
                .body
                .contains("ERROR star_sim::automation: stellar catalog invariant violated")
        );
        assert!(draft.body.contains("first request\nlast request"));
        assert!(!draft.body.contains("star_sim_debug"));
        fs::remove_dir_all(artifact_dir).ok();
    }

    #[test]
    fn truncates_long_titles_by_characters() {
        let artifact_dir = temporary_dir("title");
        let message = format!("{}終", "a".repeat(200));
        fs::create_dir_all(&artifact_dir).unwrap();
        fs::write(
            artifact_dir.join(FAILURE_JSON_NAME),
            serde_json::to_vec(&json!({
                "version": 1,
                "kind": "error",
                "message": message,
            }))
            .unwrap(),
        )
        .unwrap();

        let draft = IssueDraft::from_artifacts(&artifact_dir).unwrap();

        assert_eq!(draft.title.chars().count(), 120);
        assert!(draft.title.ends_with("..."));
        fs::remove_dir_all(artifact_dir).ok();
    }

    #[test]
    fn missing_logs_and_recording_are_rendered_as_empty_sections() {
        let artifact_dir = temporary_dir("optional");
        write_failure(&artifact_dir, None);

        let draft = IssueDraft::from_artifacts(&artifact_dir).unwrap();

        assert!(
            draft
                .body
                .contains("## Recent Bevy log\n\n````text\n\n````")
        );
        assert!(
            draft
                .body
                .contains("## Last protocol activity\n\n````jsonl\n\n````")
        );
        fs::remove_dir_all(artifact_dir).ok();
    }

    #[test]
    fn uses_custom_recording_path_when_present() {
        let artifact_dir = temporary_dir("custom-record");
        let record_path = artifact_dir.join("custom-session.jsonl");
        write_failure(&artifact_dir, Some(&record_path));
        fs::write(&record_path, "custom first\ncustom last\n").unwrap();
        fs::write(artifact_dir.join(SESSION_RECORD_NAME), "wrong default\n").unwrap();

        let draft = IssueDraft::from_artifacts(&artifact_dir).unwrap();

        assert!(draft.body.contains("custom first\ncustom last"));
        assert!(!draft.body.contains("wrong default"));
        fs::remove_dir_all(artifact_dir).ok();
    }

    #[test]
    fn keeps_only_the_last_twelve_session_entries_in_order() {
        let artifact_dir = temporary_dir("tail");
        write_failure(&artifact_dir, None);
        let session = (0..15)
            .map(|sequence| format!("request-{sequence}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(artifact_dir.join(SESSION_RECORD_NAME), session).unwrap();

        let draft = IssueDraft::from_artifacts(&artifact_dir).unwrap();

        assert!(!draft.body.contains("request-2"));
        assert!(draft.body.contains("request-3\nrequest-4"));
        assert!(draft.body.contains("request-14"));
        fs::remove_dir_all(artifact_dir).ok();
    }

    #[test]
    fn writes_draft_and_accepts_consumer_footer() {
        let artifact_dir = temporary_dir("write");
        write_failure(&artifact_dir, None);
        let draft = IssueDraft::from_artifacts(&artifact_dir)
            .unwrap()
            .with_footer("_Generated by `consumer report`._");

        let path = draft.write_to(&artifact_dir).unwrap();

        assert_eq!(path, artifact_dir.join(ISSUE_DRAFT_NAME));
        let written = fs::read_to_string(path).unwrap();
        assert!(written.ends_with("\n\n_Generated by `consumer report`._\n"));
        fs::remove_dir_all(artifact_dir).ok();
    }
}
