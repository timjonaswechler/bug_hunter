//! Version 1 persistence. The coordinator never performs file IO.
use super::super::{Error, Outcome};
use crate::command::Command;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, File, OpenOptions};
use serde_json::{Value, json};
use std::io::{self, BufWriter, Write};

pub(crate) fn validate(path: &str) -> Result<(), Error> {
    if path.is_empty()
        || !path.ends_with(".jsonl")
        || path.contains(['\\', '\0'])
        || path.as_bytes().get(1) == Some(&b':')
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(Error::new("invalid_recording_path", path));
    }
    Ok(())
}

fn path_error(error: io::Error, path: &str) -> Error {
    if matches!(error.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)) {
        Error::new("invalid_recording_path", format!("{path}: {error}"))
    } else {
        Error::io(format!("{path}: {error}"))
    }
}

fn parent(root: &Dir, path: &str) -> Result<(Dir, String), Error> {
    validate(path)?;
    let mut parts: Vec<_> = path.split('/').collect();
    let name = parts.pop().unwrap().to_owned();
    let mut directory = root.try_clone().map_err(Error::io)?;
    for part in parts {
        directory = match directory.open_dir_nofollow(part) {
            Ok(directory) => directory,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match directory.create_dir(part) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(path_error(error, path)),
                }
                directory
                    .open_dir_nofollow(part)
                    .map_err(|e| path_error(e, path))?
            }
            Err(error) => return Err(path_error(error, path)),
        };
    }
    Ok((directory, name))
}

// Replay shares the writer's lexical rules and opens every component without following links.
pub(crate) fn open_read(root: &Dir, path: &str) -> Result<File, Error> {
    validate(path)?;
    let mut parts: Vec<_> = path.split('/').collect();
    let name = parts.pop().unwrap();
    let mut directory = root.try_clone().map_err(|e| path_error(e, path))?;
    for part in parts {
        directory = directory
            .open_dir_nofollow(part)
            .map_err(|e| path_error(e, path))?;
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    // A swapped-in FIFO must not leave the loader blocked in open before metadata validation.
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = directory.open_with(name, &options).map_err(|e| {
        if directory.symlink_metadata(name).is_ok_and(|m| !m.is_file()) {
            Error::new("invalid_recording_path", path)
        } else {
            path_error(e, path)
        }
    })?;
    if !file.metadata().map_err(|e| path_error(e, path))?.is_file() {
        return Err(Error::new("invalid_recording_path", path));
    }
    Ok(file)
}

pub(super) trait Sink: Write + Send {
    fn sync_all(&self) -> io::Result<()>;
}

impl Sink for File {
    fn sync_all(&self) -> io::Result<()> {
        File::sync_all(self)
    }
}

pub(super) struct Recording<W: Write = File> {
    stream: BufWriter<W>,
    pub path: String,
    count: u64,
}

impl Recording {
    pub fn start(root: &Dir, path: String) -> Result<Self, Error> {
        Self::start_with(root, path, |file| file)
    }
}

impl<W: Sink> Recording<W> {
    pub(super) fn start_with(
        root: &Dir,
        path: String,
        wrap: impl FnOnce(File) -> W,
    ) -> Result<Self, Error> {
        let (directory, name) = parent(root, &path)?;
        let options = OpenOptions::new()
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No)
            .clone();
        let file = directory.open_with(&name, &options).map_err(|error| {
            if directory
                .symlink_metadata(&name)
                .is_ok_and(|m| !m.is_file())
            {
                Error::new("invalid_recording_path", &path)
            } else if error.kind() == io::ErrorKind::AlreadyExists {
                Error::new("recording_path_exists", &path)
            } else {
                path_error(error, &path)
            }
        })?;
        let mut recording = Self {
            stream: BufWriter::new(wrap(file)),
            path,
            count: 0,
        };
        if let Err(error) = recording
            .line(&json!({"type":"recording_started","format_version":1}))
            .and_then(|_| recording.stream.flush())
        {
            let path = recording.path.clone();
            drop(recording);
            let _ = directory.remove_file(name);
            return Err(Error::io(format!("{path}: {error}")));
        }
        Ok(recording)
    }

    fn line(&mut self, value: &Value) -> io::Result<()> {
        serde_json::to_writer(&mut self.stream, value)?;
        self.stream.write_all(b"\n")
    }

    pub fn append(&mut self, entries: Vec<Value>) -> Result<(), Error> {
        for entry in entries {
            self.line(&entry)
                .map_err(|e| Error::io(format!("{}: {e}", self.path)))?;
            self.count = self.count.checked_add(1).expect("recording count overflow");
        }
        self.stream
            .flush()
            .map_err(|e| Error::io(format!("{}: {e}", self.path)))
    }

    pub fn finish(mut self, outcome: &str) -> Result<u64, Error> {
        self.line(
            &json!({"type":"recording_ended", "outcome":outcome, "recorded_commands":self.count}),
        )
        .and_then(|_| self.stream.flush())
        .and_then(|_| self.stream.get_ref().sync_all())
        .map_err(|e| Error::io(format!("{}: {e}", self.path)))?;
        Ok(self.count)
    }
}

pub(super) fn entry(command: &Command, outcome: Option<&Outcome>) -> Value {
    let outcome = match outcome {
        Some(Ok(output)) => json!({"status":"completed","output":output}),
        Some(Err(Error::Io { message })) => {
            json!({"status":"io_failed","error":{"message":message}})
        }
        Some(Err(Error::Protocol { code, message })) => {
            json!({"status":"protocol_failed","error":{"code":code,"message":message}})
        }
        None | Some(Err(Error::Ended)) => json!({"status":"unanswered"}),
        Some(Err(error)) => {
            json!({"status":"rejected","error":{"code":error.code(),"message":error.message()}})
        }
    };
    let mut value = serde_json::to_value(command).expect("recordable command");
    value["type"] = json!("command");
    value["outcome"] = outcome;
    value
}
