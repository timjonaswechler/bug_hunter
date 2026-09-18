//! Failure observation and immutable report context. Formatting and providers follow separately.
use crate::session::Error;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(crate) mod capture;
mod failure;
pub(crate) mod marker;
pub(crate) mod observer;
mod snapshot;
pub use failure::{BacktraceStatus, Failure, Location, Origin};
pub use snapshot::{Application, Context, Platform, Report, SourceRevision, Toolchain};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub tracing_errors: bool,
    pub output: PathBuf,
    pub provider: provider::Config,
}

pub mod provider {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
    pub enum Config {
        Local,
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), Error> {
        if self.output.as_os_str().is_empty()
            || self
                .output
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(Error::new(
                "invalid_config",
                "report output must be a nonempty relative path of normal components",
            ));
        }
        Ok(())
    }
}
