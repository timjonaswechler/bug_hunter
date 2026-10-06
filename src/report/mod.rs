//! Failure observation, immutable report snapshots and shared Markdown rendering.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(crate) mod capture;
mod failure;
mod markdown;
pub(crate) mod marker;
mod normalize;
pub(crate) mod observer;
pub mod provider;
mod signature;
mod snapshot;
#[cfg(test)]
mod tests;
pub(crate) mod work;
pub use failure::{BacktraceStatus, Failure, Location, Origin};
pub use signature::Signature;
pub use snapshot::{Application, Context, Platform, Report, SourceRevision, Toolchain};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub tracing_errors: bool,
    pub output: PathBuf,
    pub provider: provider::Config,
}

impl Config {
    pub fn validate(&self) -> Result<(), crate::session::Error> {
        if !provider::local::valid_output(&self.output) {
            return Err(crate::session::Error::new(
                "invalid_config",
                "report output must be a nonempty relative path of normal components",
            ));
        }
        Ok(())
    }
}

/// Persist using the destination selected at session start. This synchronous
/// operation does no work on the coordinator and also works after session end.
pub fn submit(
    report: &Report,
    session: &crate::session::Session,
) -> Result<provider::Outcome, Error> {
    let destination = session.report_destination();
    destination.submit(report)
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", content = "error", rename_all = "snake_case")]
pub enum Error {
    Local(provider::local::Error),
    FallbackFailed {
        provider: provider::Error,
        local: provider::local::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(error) => write!(f, "local report: {error}"),
            Self::FallbackFailed { provider, local } => {
                write!(
                    f,
                    "report provider failed: {provider}; local fallback failed: {local}"
                )
            }
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Local(error) => Some(error),
            Self::FallbackFailed { provider, .. } => Some(provider),
        }
    }
}

pub(crate) struct Destination {
    root: std::sync::Arc<cap_std::fs::Dir>,
    config: Config,
    project: PathBuf,
}
impl Destination {
    pub(crate) fn new(
        root: std::sync::Arc<cap_std::fs::Dir>,
        config: Config,
        project: PathBuf,
    ) -> Self {
        Self {
            root,
            config,
            project,
        }
    }

    fn submit(&self, report: &Report) -> Result<provider::Outcome, Error> {
        self.run(report, &std::sync::atomic::AtomicBool::new(false))
            .unlimited()
    }

    pub(crate) fn run(
        &self,
        report: &Report,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> work::Result {
        self.submit_using(report, cancel, || {
            provider::github::submit_cancellable(&self.project, report, cancel)
        })
    }

    #[cfg(test)]
    fn submit_with(
        &self,
        report: &Report,
        remote: impl FnOnce() -> Result<provider::Outcome, provider::github::Error>,
    ) -> Result<provider::Outcome, Error> {
        self.submit_using(report, &std::sync::atomic::AtomicBool::new(false), remote)
            .unlimited()
    }

    fn submit_using(
        &self,
        report: &Report,
        cancel: &std::sync::atomic::AtomicBool,
        remote: impl FnOnce() -> Result<provider::Outcome, provider::github::Error>,
    ) -> work::Result {
        if work::check(cancel).is_err() {
            return work::Result::interrupted();
        }
        let provider_error = match self.config.provider {
            provider::Config::Local => {
                return work::Result::complete(
                    provider::local::submit_cancellable(
                        &self.root,
                        &self.config.output,
                        report,
                        cancel,
                    )
                    .map_err(Error::Local),
                    cancel,
                );
            }
            provider::Config::Github => match remote() {
                Ok(outcome) => return work::Result::Submitted { outcome },
                Err(_) if work::check(cancel).is_err() => {
                    return work::Result::interrupted();
                }
                Err(error) => provider::Error::Github(error),
            },
        };
        let result = match provider::local::submit_cancellable(
            &self.root,
            &self.config.output,
            report,
            cancel,
        ) {
            Ok(
                provider::Outcome::Created {
                    reference: provider::Reference::File(reference),
                }
                | provider::Outcome::Existing {
                    reference: provider::Reference::File(reference),
                },
            ) => Ok(provider::Outcome::Fallback {
                reference,
                provider_error,
            }),
            Ok(_) => unreachable!("local provider only returns file references"),
            Err(local) => Err(Error::FallbackFailed {
                provider: provider_error,
                local,
            }),
        };
        work::Result::complete(result, cancel)
    }
}
