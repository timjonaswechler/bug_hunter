//! Streaming marker reassembly. Only fully validated markers disappear from human diagnostics.
use super::{
    Failure, Origin,
    marker::{self, Frame, PREFIX, Payload},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::collections::{BTreeMap, VecDeque};

pub(crate) enum Notice {
    Failure(Failure),
    Invalid(String),
    LayerStatus(bool),
}

#[derive(Default)]
pub(crate) struct Output {
    pub diagnostics: Vec<u8>,
    pub notices: Vec<Notice>,
}

struct Piece {
    event: Option<String>,
    bytes: Vec<u8>,
}
struct Group {
    count: u32,
    checksum: String,
    chunks: BTreeMap<u32, Vec<u8>>,
}

pub(crate) struct Observer {
    buffer: Vec<u8>,
    pieces: VecDeque<Piece>,
    groups: BTreeMap<String, Group>,
    tracing_errors: bool,
    panic_seen: bool,
}

impl Observer {
    pub fn new(tracing_errors: bool) -> Self {
        Self {
            buffer: Vec::new(),
            pieces: VecDeque::new(),
            groups: BTreeMap::new(),
            tracing_errors,
            panic_seen: false,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Output {
        self.buffer.extend_from_slice(bytes);
        let mut output = Output::default();
        loop {
            if self.buffer.starts_with(PREFIX) {
                if let Some(end) = self.buffer.iter().position(|b| *b == b'\n') {
                    let line: Vec<_> = self.buffer.drain(..=end).collect();
                    self.line(line, &mut output);
                } else if self.buffer.len() > marker::MAX_LINE {
                    // This is already invalid, independent of any later LF.
                    let bytes = std::mem::take(&mut self.buffer);
                    self.pieces.push_back(Piece { event: None, bytes });
                    output.notices.push(Notice::Invalid(
                        "marker line exceeds atomic write size".into(),
                    ));
                } else {
                    break;
                }
            } else if let Some(start) = self.buffer.windows(PREFIX.len()).position(|s| s == PREFIX)
            {
                let bytes = self.buffer.drain(..start).collect();
                self.pieces.push_back(Piece { event: None, bytes });
            } else {
                // Keep only a potential split prefix; ordinary stderr need not have an LF.
                let keep = (1..PREFIX.len())
                    .rev()
                    .find(|n| self.buffer.ends_with(&PREFIX[..*n]))
                    .unwrap_or(0);
                let end = self.buffer.len() - keep;
                if end != 0 {
                    let bytes = self.buffer.drain(..end).collect();
                    self.pieces.push_back(Piece { event: None, bytes });
                }
                break;
            }
        }
        self.flush(&mut output);
        output
    }

    pub fn finish(&mut self) -> Output {
        let mut output = Output::default();
        if !self.buffer.is_empty() {
            if self.buffer.starts_with(PREFIX) {
                output
                    .notices
                    .push(Notice::Invalid("incomplete marker at stderr EOF".into()));
            }
            self.pieces.push_back(Piece {
                event: None,
                bytes: std::mem::take(&mut self.buffer),
            });
        }
        for event in std::mem::take(&mut self.groups).into_keys() {
            self.resolve(&event, false);
            output
                .notices
                .push(Notice::Invalid(format!("incomplete marker event {event}")));
        }
        self.flush(&mut output);
        output
    }

    pub fn process_exit(&self, status: String, intentional: bool) -> Option<Failure> {
        (!intentional && !self.panic_seen).then(|| Failure::process_exit(status))
    }

    fn line(&mut self, bytes: Vec<u8>, output: &mut Output) {
        let parsed = serde_json::from_slice::<Frame>(&bytes[PREFIX.len()..]);
        let frame = match parsed {
            Ok(frame)
                if bytes.len() <= marker::MAX_LINE
                    && frame.version == 1
                    && !frame.event.is_empty()
                    && frame.event.len() <= 64
                    && frame.count > 0
                    && frame.index < frame.count
                    && frame.checksum.len() == 64
                    && frame
                        .checksum
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
            {
                frame
            }
            _ => {
                self.pieces.push_back(Piece { event: None, bytes });
                output
                    .notices
                    .push(Notice::Invalid("invalid marker frame".into()));
                return;
            }
        };
        let event = frame.event.clone();
        self.pieces.push_back(Piece {
            event: Some(event.clone()),
            bytes,
        });
        let group = self.groups.entry(event.clone()).or_insert_with(|| Group {
            count: frame.count,
            checksum: frame.checksum.clone(),
            chunks: BTreeMap::new(),
        });
        let data = STANDARD
            .decode(&frame.data)
            .ok()
            .filter(|d| !d.is_empty() && d.len() <= marker::CHUNK);
        let valid = group.count == frame.count
            && group.checksum == frame.checksum
            && !group.chunks.contains_key(&frame.index)
            && data.is_some();
        if !valid {
            self.groups.remove(&event);
            self.resolve(&event, false);
            output.notices.push(Notice::Invalid(format!(
                "inconsistent marker event {event}"
            )));
            return;
        }
        group.chunks.insert(frame.index, data.unwrap());
        if group.chunks.len() != group.count as usize {
            return;
        }
        let group = self.groups.remove(&event).unwrap();
        let payload: Vec<_> = group.chunks.into_values().flatten().collect();
        let decoded = (marker::checksum(&payload) == group.checksum)
            .then(|| serde_json::from_slice::<Payload>(&payload).ok())
            .flatten();
        let valid = match decoded {
            Some(Payload::LayerStatus { installed }) => {
                output.notices.push(Notice::LayerStatus(installed));
                true
            }
            Some(Payload::Failure { failure }) => match failure.origin() {
                Origin::Panic { .. } => {
                    self.panic_seen = true;
                    output.notices.push(Notice::Failure(failure));
                    true
                }
                Origin::TracingError { .. } => {
                    if self.tracing_errors {
                        output.notices.push(Notice::Failure(failure));
                    }
                    true
                }
                // Process exit is inferred from the owned child, never supplied by stderr.
                Origin::ProcessExit { .. } => false,
            },
            None => false,
        };
        self.resolve(&event, valid);
        if !valid {
            output
                .notices
                .push(Notice::Invalid(format!("invalid marker payload {event}")));
        }
    }

    fn resolve(&mut self, event: &str, valid: bool) {
        for piece in &mut self.pieces {
            if piece.event.as_deref() == Some(event) {
                piece.event = None;
                if valid {
                    piece.bytes.clear();
                }
            }
        }
    }

    fn flush(&mut self, output: &mut Output) {
        while self.pieces.front().is_some_and(|p| p.event.is_none()) {
            output
                .diagnostics
                .extend(self.pieces.pop_front().unwrap().bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn panic() -> Payload {
        Payload::Failure {
            failure: Failure::panic(Some("Grüße 🦜".into()), None, Some("frame\n".repeat(200))),
        }
    }
    #[test]
    fn every_split_and_interleaved_chunks_preserve_other_bytes_exactly() {
        let a = marker::encode(&panic(), "a".into());
        let b = marker::encode(&Payload::LayerStatus { installed: true }, "b".into());
        assert!(a.iter().all(|line| line.len() <= 512));
        let mut bytes = b"ordinary\xff without LF".to_vec();
        for i in 0..a.len() {
            bytes.extend(&a[i]);
            if i < b.len() {
                bytes.extend(&b[i]);
            }
            bytes.extend(b"diagnostic\n");
        }
        bytes.extend(b"last");
        let mut expected = b"ordinary\xff without LF".to_vec();
        expected.extend(b"diagnostic\n".repeat(a.len()));
        expected.extend(b"last");
        for split in 0..=bytes.len() {
            let mut observer = Observer::new(false);
            let mut first = observer.push(&bytes[..split]);
            let second = observer.push(&bytes[split..]);
            first.diagnostics.extend(second.diagnostics);
            first.notices.extend(second.notices);
            first.diagnostics.extend(observer.finish().diagnostics);
            assert_eq!(first.diagnostics, expected);
            assert_eq!(first.notices.len(), 2);
            assert!(observer.process_exit("0".into(), false).is_none());
        }
    }
    #[test]
    fn corruption_and_incomplete_markers_remain_diagnostics_and_do_not_prove_a_panic() {
        let good = marker::encode(&panic(), "a".into()).concat();
        let mut bad = good.clone();
        let index = bad
            .windows(12)
            .position(|s| s == b"\"checksum\":\"")
            .unwrap()
            + 12;
        bad[index] = b'z';
        for bytes in [
            &bad[..],
            &good[..good.len() - 1],
            &good[..100],
            b"\x1eWOODPECKER_REPORT:{no}\n",
        ] {
            let mut observer = Observer::new(true);
            let mut result = observer.push(bytes);
            let end = observer.finish();
            result.diagnostics.extend(end.diagnostics);
            result.notices.extend(end.notices);
            assert_eq!(result.diagnostics, bytes);
            assert!(
                result
                    .notices
                    .iter()
                    .any(|n| matches!(n, Notice::Invalid(_)))
            );
            assert!(
                !result
                    .notices
                    .iter()
                    .any(|n| matches!(n, Notice::Failure(_)))
            );
            assert!(observer.process_exit("0".into(), false).is_some());
            assert!(observer.process_exit("101".into(), true).is_none());
        }
    }
    #[test]
    fn tracing_is_opt_in_and_plain_error_text_is_not_a_failure() {
        let bytes = marker::encode(
            &Payload::Failure {
                failure: Failure::tracing_error("message".into(), None, None),
            },
            "x".into(),
        )
        .concat();
        for enabled in [true, false] {
            let mut observer = Observer::new(enabled);
            let output = observer.push(&bytes);
            assert!(output.diagnostics.is_empty());
            assert_eq!(output.notices.len(), usize::from(enabled));
            let plain = b"ERROR handled Err\nthread panicked in a human log\n";
            let output = observer.push(plain);
            assert_eq!(output.diagnostics, plain);
            assert!(output.notices.is_empty());
        }
    }

    #[test]
    fn similar_text_and_partial_prefixes_are_not_markers() {
        for bytes in [
            b"WOODPECKER_REPORT:{}".as_slice(),
            b"\x1eWOODPECKER_REPOR",
            b"\xfftext without LF",
        ] {
            let mut observer = Observer::new(true);
            let mut result = observer.push(bytes);
            let end = observer.finish();
            result.diagnostics.extend(end.diagnostics);
            assert_eq!(result.diagnostics, bytes);
            assert!(result.notices.is_empty());
            assert!(end.notices.is_empty());
        }
    }
}
