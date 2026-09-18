//! GitHub CLI owns repository/host/auth resolution. No provider credentials or prompts.
use super::{Outcome, Reference};
use crate::{
    report::{Report, markdown},
    session::process,
};
use serde::{Deserialize, Serialize};
use std::{path::Path, process::Command, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Search,
    Publish,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Error {
    Unavailable {
        operation: Operation,
        message: String,
    },
    CommandFailed {
        operation: Operation,
        message: String,
    },
    InvalidResponse {
        operation: Operation,
        message: String,
    },
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, operation, message) = match self {
            Self::Unavailable { operation, message } => ("unavailable", operation, message),
            Self::CommandFailed { operation, message } => ("command failed", operation, message),
            Self::InvalidResponse { operation, message } => {
                ("invalid response", operation, message)
            }
        };
        let operation = match operation {
            Operation::Search => "search",
            Operation::Publish => "publish",
        };
        write!(f, "{operation} {kind}: {message}")
    }
}
impl std::error::Error for Error {}

pub(crate) fn submit_cancellable(
    project: &Path,
    report: &Report,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Outcome, Error> {
    submit_with(report, |operation, input| {
        execute_cancellable(command(project, "gh", operation), operation, input, cancel)
    })
}

fn command(project: &Path, program: impl AsRef<std::ffi::OsStr>, operation: Operation) -> Command {
    let mut command = Command::new(program);
    command.current_dir(project).env("GH_PROMPT_DISABLED", "1");
    command.arg("api");
    match operation {
        Operation::Search => command.args([
            "repos/{owner}/{repo}/issues?state=all&per_page=100",
            "--method",
            "GET",
            "--paginate",
        ]),
        Operation::Publish => command.args([
            "repos/{owner}/{repo}/issues",
            "--method",
            "POST",
            "--input",
            "-",
        ]),
    };
    command
}

/// Reuse the session's process-group ownership and concurrent pipe draining.
/// Closing the writer after the single JSON payload supplies EOF to `--input -`.
#[cfg(test)]
fn execute(command: Command, operation: Operation, input: Option<String>) -> Result<String, Error> {
    execute_cancellable(
        command,
        operation,
        input,
        &std::sync::atomic::AtomicBool::new(false),
    )
}

fn execute_cancellable(
    command: Command,
    operation: Operation,
    input: Option<String>,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<String, Error> {
    let check = || {
        crate::report::work::check(cancel).map_err(|e| Error::CommandFailed {
            operation,
            message: e.to_string(),
        })
    };
    check()?;
    let mut process = process::Process::spawn(command).map_err(|error| Error::Unavailable {
        operation,
        message: error.to_string(),
    })?;
    if let Some(input) = input {
        process
            .writer
            .as_ref()
            .unwrap()
            .send(input)
            .map_err(|error| Error::CommandFailed {
                operation,
                message: error.to_string(),
            })?;
    }
    process.writer.take();
    let mut stdout = String::new();
    let mut stderr = Vec::new();
    let mut closed = 0;
    let mut failure = None;
    let mut status = None;
    loop {
        // Returning drops Process, kills its whole group, drains and joins its pipe workers.
        check()?;
        for _ in 0..128 {
            let Ok(event) = process.events.try_recv() else {
                break;
            };
            match event {
                process::Event::Line(line) => {
                    stdout.push_str(&line);
                    stdout.push('\n');
                }
                process::Event::Diagnostic(bytes) => stderr.extend(bytes),
                process::Event::Closed(_) => closed += 1,
                process::Event::Failed(channel, message) => {
                    if channel != "stdin" {
                        closed += 1;
                    }
                    failure.get_or_insert(if channel == "stdout" {
                        Error::InvalidResponse { operation, message }
                    } else {
                        Error::CommandFailed { operation, message }
                    });
                }
            }
        }
        if status.is_none() {
            status = process
                .child
                .try_wait()
                .map_err(|error| Error::CommandFailed {
                    operation,
                    message: error.to_string(),
                })?;
        }
        if let Some(status) = status
            && closed == 2
        {
            if !status.success() {
                return Err(Error::CommandFailed {
                    operation,
                    message: format!(
                        "gh exited with {status}: {}",
                        String::from_utf8_lossy(&stderr).trim()
                    ),
                });
            }
            return match failure {
                Some(error) => Err(error),
                None => Ok(stdout),
            };
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[derive(Deserialize)]
struct Issue {
    number: u64,
    html_url: String,
    // The REST issues endpoint also returns pull requests. They cannot deduplicate reports.
    #[serde(default)]
    pull_request: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct ListedIssue {
    #[serde(flatten)]
    issue: Issue,
    // Null is valid for an empty body; an omitted field is not a complete search result.
    #[serde(deserialize_with = "Option::deserialize")]
    body: Option<String>,
}
impl Issue {
    fn reference(&self, operation: Operation) -> Result<Reference, Error> {
        if self.number == 0 || !valid_url(&self.html_url) {
            return Err(invalid(
                operation,
                "issue requires a positive number and an absolute HTTP(S) URL",
            ));
        }
        Ok(Reference::Issue {
            identifier: self.number.to_string(),
            url: self.html_url.clone(),
        })
    }
}

fn valid_url(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    !url.chars().any(|c| c.is_whitespace() || c.is_control())
        && rest
            .split('/')
            .next()
            .is_some_and(|host| !host.is_empty() && !host.contains(['@', '\\', '?', '#']))
}

fn invalid(operation: Operation, message: impl ToString) -> Error {
    Error::InvalidResponse {
        operation,
        message: message.to_string(),
    }
}

fn submit_with(
    report: &Report,
    mut run: impl FnMut(Operation, Option<String>) -> Result<String, Error>,
) -> Result<Outcome, Error> {
    let response = run(Operation::Search, None)?;
    // `gh api --paginate` emits one JSON array per page, not a single outer array.
    let pages = serde_json::Deserializer::from_str(&response).into_iter::<Vec<ListedIssue>>();
    let mut count = 0;
    let mut existing = None;
    for page in pages {
        count += 1;
        for ListedIssue { issue, body } in page.map_err(|e| invalid(Operation::Search, e))? {
            let reference = issue.reference(Operation::Search)?;
            if issue.pull_request.is_none()
                && markdown::has_signature(body.unwrap_or_default().as_bytes(), report.signature())
                    .map_err(|e| invalid(Operation::Search, e))?
            {
                existing.get_or_insert(reference);
            }
        }
    }
    if count == 0 {
        return Err(invalid(Operation::Search, "missing issue pages"));
    }
    // Even after a hit, validate all returned pages before claiming successful search.
    if let Some(reference) = existing {
        return Ok(Outcome::Existing { reference });
    }
    let input =
        serde_json::json!({"title": report.title(), "body": report.to_markdown()}).to_string();
    let response = run(Operation::Publish, Some(input))?;
    let issue: Issue =
        serde_json::from_str(&response).map_err(|e| invalid(Operation::Publish, e))?;
    if issue.pull_request.is_some() {
        return Err(invalid(
            Operation::Publish,
            "issue creation returned a pull request",
        ));
    }
    Ok(Outcome::Created {
        reference: issue.reference(Operation::Publish)?,
    })
}

#[cfg(test)]
mod tests;
