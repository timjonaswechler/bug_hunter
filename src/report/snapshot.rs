use super::Failure;
use crate::session::{self, Session};
use serde::Serialize;

/// Immutable observation snapshot. Title, signature, rendering and providers are not yet exposed.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    failure: Failure,
    context: Context,
}
impl Report {
    pub fn create(failure: Failure, session: &Session) -> Self {
        Self {
            failure,
            context: session.report_context(),
        }
    }
    pub fn failure(&self) -> &Failure {
        &self.failure
    }
    pub fn context(&self) -> &Context {
        &self.context
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Context {
    pub application: Application,
    pub woodpecker_version: String,
    pub protocol_version: u32,
    pub capabilities: session::Capabilities,
    pub tick: crate::command::tick::Config,
    pub platform: Platform,
    pub toolchain: Toolchain,
    pub commands: Vec<session::history::Entry>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Application {
    pub package: String,
    pub version: String,
    pub target: session::launch::Target,
    pub features: Vec<String>,
    pub arguments: Vec<String>,
    pub source: Option<SourceRevision>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SourceRevision {
    pub commit: String,
    pub dirty: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct Platform {
    pub os: String,
    pub arch: String,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Toolchain {
    pub cargo: Option<String>,
    pub rustc: Option<String>,
}
