use super::{Failure, Signature};
use crate::session::{self, Session};
use serde::Serialize;

/// Immutable report. Construction captures context and computes identity once.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    title: String,
    failure: Failure,
    signature: Signature,
    context: Context,
}
impl Report {
    pub fn create(failure: Failure, session: &Session) -> Self {
        let (context, project) = session.report_context();
        Self::from_snapshot(failure, context, &project)
    }

    pub(super) fn from_snapshot(
        failure: Failure,
        context: Context,
        project: &std::path::Path,
    ) -> Self {
        Self {
            title: super::normalize::title(&failure),
            signature: Signature::create(&failure, project),
            failure,
            context,
        }
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn failure(&self) -> &Failure {
        &self.failure
    }
    pub fn signature(&self) -> &Signature {
        &self.signature
    }
    pub fn context(&self) -> &Context {
        &self.context
    }
    pub fn to_markdown(&self) -> String {
        super::markdown::render(self)
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
