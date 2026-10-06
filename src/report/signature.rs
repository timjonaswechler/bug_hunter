use super::{Failure, normalize};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Full versioned identity, including algorithm. Diagnostics do not participate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Signature {
    value: String,
}

impl Signature {
    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub(super) fn create(failure: &Failure, project: &std::path::Path) -> Self {
        let message = normalize::message(failure.message().unwrap_or_default(), project);
        let fields = [
            ("kind", normalize::kind(failure)),
            ("message", message.as_str()),
        ];
        let mut hash = Sha256::new();
        hash.update(b"bug_hunter.signature\0");
        hash.update(1u32.to_be_bytes());
        hash.update((fields.len() as u32).to_be_bytes());
        for (name, value) in fields {
            hash.update((name.len() as u32).to_be_bytes());
            hash.update(name.as_bytes());
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value.as_bytes());
        }
        Self {
            value: format!("v1:sha256:{:x}", hash.finalize()),
        }
    }
}
