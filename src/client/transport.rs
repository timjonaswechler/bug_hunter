//! JSON-lines input/output adapters for Controlled Sessions.
//!
//! Input polling is nonblocking. The standard input adapter reads lines on a background thread;
//! output implementations define their own synchronization and serialization behavior.

use crate::protocol::{Ready, Response};
use std::{
    io::{self, BufRead, Write},
    sync::{
        Mutex,
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread,
};

/// One event from a JSON-lines input source.
#[derive(Debug, PartialEq, Eq)]
pub enum Input {
    /// A line with its terminating newline removed.
    Line(String),
    /// Normal end of the input stream.
    Eof,
    /// Reader failure; the background reader terminates after sending it.
    Error(String),
}

/// Mutex-protected receiver polled by the control plugin.
pub struct JsonLinesInput {
    receiver: Mutex<Receiver<Input>>,
}

impl JsonLinesInput {
    /// Starts a background stdin line-reader using a bounded channel of `capacity` events.
    pub fn stdin(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        thread::spawn(move || read_lines(io::stdin().lock(), sender));
        Self {
            receiver: Mutex::new(receiver),
        }
    }

    /// Takes ownership of a custom input receiver.
    pub fn from_receiver(receiver: Receiver<Input>) -> Self {
        Self {
            receiver: Mutex::new(receiver),
        }
    }

    /// Polls without blocking.
    ///
    /// Returns [`TryRecvError::Empty`] when no event is ready and
    /// [`TryRecvError::Disconnected`] when every sender has gone away.
    pub fn try_recv(&self) -> Result<Input, TryRecvError> {
        self.receiver
            .lock()
            .expect("automation input mutex poisoned")
            .try_recv()
    }
}

fn read_lines(reader: impl BufRead, sender: SyncSender<Input>) {
    for line in reader.lines() {
        let input = match line {
            Ok(line) => Input::Line(line),
            Err(error) => Input::Error(error.to_string()),
        };
        let stop = matches!(input, Input::Error(_));
        if sender.send(input).is_err() || stop {
            return;
        }
    }
    let _ = sender.send(Input::Eof);
}

/// Sink for startup [`Ready`] metadata and correlated [`Response`] values.
///
/// Implementations are shared across schedules and must provide any required synchronization and
/// serialization framing.
pub trait Output: Send + Sync + 'static {
    /// Writes the startup handshake.
    fn ready(&self, ready: &Ready) -> io::Result<()>;
    /// Writes one protocol response.
    fn response(&self, response: &Response) -> io::Result<()>;
}

/// Standard-output sink that writes and flushes one JSON value per line.
#[derive(Default)]
pub struct StdoutOutput;

impl Output for StdoutOutput {
    fn ready(&self, ready: &Ready) -> io::Result<()> {
        write_json(ready)
    }

    fn response(&self, response: &Response) -> io::Result<()> {
        write_json(response)
    }
}

fn write_json(value: &impl serde::Serialize) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, value).map_err(io::Error::other)?;
    writeln!(lock)?;
    lock.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reader_reports_lines_then_eof() {
        let (sender, receiver) = mpsc::sync_channel(4);
        read_lines(Cursor::new("one\ntwo\n"), sender);
        assert_eq!(receiver.recv().unwrap(), Input::Line("one".into()));
        assert_eq!(receiver.recv().unwrap(), Input::Line("two".into()));
        assert_eq!(receiver.recv().unwrap(), Input::Eof);
    }
}
