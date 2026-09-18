use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod github;
pub mod local;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Config {
    Local,
    Github,
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Serde's internally tagged unit variants ignore extra fields even with
        // deny_unknown_fields. Empty struct variants enforce the config contract.
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum Wire {
            Local {},
            Github {},
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Local {} => Self::Local,
            Wire::Github {} => Self::Github,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FileReference {
    /// Relative to the session's artifact root.
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "reference", rename_all = "snake_case")]
pub enum Reference {
    File(FileReference),
    Issue { identifier: String, url: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Created {
        reference: Reference,
    },
    Existing {
        reference: Reference,
    },
    Fallback {
        reference: FileReference,
        provider_error: Error,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "error", rename_all = "snake_case")]
pub enum Error {
    Github(github::Error),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Github(error) => write!(f, "GitHub: {error}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Github(error) => Some(error),
        }
    }
}
