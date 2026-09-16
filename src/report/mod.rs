//! Report configuration shared by launch paths. Report execution is outside the first slice.
use crate::session::Error;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
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
        if self.tracing_errors {
            return Err(Error::new(
                "unsupported_feature",
                "tracing observation is not implemented in this slice",
            ));
        }
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
