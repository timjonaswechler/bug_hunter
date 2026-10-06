//! Private versioned stderr framing. Each write fits the minimum POSIX PIPE_BUF.
use super::Failure;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) const PREFIX: &[u8] = b"\x1eWOODPECKER_REPORT:";
pub(super) const MAX_LINE: usize = 512;
pub(super) const CHUNK: usize = 128;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Payload {
    Failure { failure: Failure },
    LayerStatus { installed: bool },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Frame {
    pub version: u32,
    pub event: String,
    pub index: u32,
    pub count: u32,
    pub checksum: String,
    pub data: String,
}

pub(super) fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn encode(payload: &Payload, event: String) -> Vec<Vec<u8>> {
    let bytes = serde_json::to_vec(payload).expect("marker payload");
    let count = u32::try_from(bytes.len().div_ceil(CHUNK)).expect("marker size");
    let checksum = checksum(&bytes);
    bytes
        .chunks(CHUNK)
        .enumerate()
        .map(|(index, data)| {
            let frame = Frame {
                version: 1,
                event: event.clone(),
                index: index as u32,
                count,
                checksum: checksum.clone(),
                data: STANDARD.encode(data),
            };
            let mut line = PREFIX.to_vec();
            serde_json::to_writer(&mut line, &frame).expect("marker frame");
            line.push(b'\n');
            assert!(line.len() <= MAX_LINE);
            line
        })
        .collect()
}

#[cfg(all(test, feature = "server"))]
pub(crate) fn fixture(failure: Failure) -> Vec<u8> {
    encode(&Payload::Failure { failure }, "fixture".into())
        .into_iter()
        .flatten()
        .collect()
}

pub(super) fn emit(payload: Payload) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let event = format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    for line in encode(&payload, event) {
        #[cfg(unix)]
        loop {
            // No stderr lock, formatter or tracing call from a panic hook. A pipe write below
            // PIPE_BUF is atomic; retry only EINTR before any bytes were transferred.
            let written =
                unsafe { libc::write(libc::STDERR_FILENO, line.as_ptr().cast(), line.len()) };
            if written == -1
                && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
            {
                continue;
            }
            break;
        }
        #[cfg(not(unix))]
        {
            use std::io::Write;
            let _ = std::io::stderr().write_all(&line);
        }
    }
}
