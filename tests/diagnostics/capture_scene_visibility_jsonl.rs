//! Process- and source-sharded JSONL writer for disposable diagnostics only.

use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

struct OpenLog {
    path: PathBuf,
    file: File,
}

pub struct DiagnosticJsonl {
    source: &'static str,
    directory_env: &'static str,
    open: Mutex<Option<OpenLog>>,
}

impl DiagnosticJsonl {
    pub const fn new(source: &'static str, directory_env: &'static str) -> Self {
        Self {
            source,
            directory_env,
            open: Mutex::new(None),
        }
    }

    pub fn enabled(&self) -> bool {
        std::env::var_os(self.directory_env).is_some()
    }

    pub fn path_for(directory: &Path, source: &str, pid: u32) -> PathBuf {
        directory.join(format!("{source}-{pid}.jsonl"))
    }

    pub fn write_json(&self, json: &str) -> io::Result<()> {
        let directory = std::env::var_os(self.directory_env).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not set", self.directory_env),
            )
        })?;
        self.write_json_in(Path::new(&directory), json)
    }

    pub fn write_json_in(&self, directory: &Path, json: &str) -> io::Result<()> {
        if json.as_bytes().contains(&b'\n') || json.as_bytes().contains(&b'\r') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "diagnostic JSON must be one finished line",
            ));
        }
        let mut finished_line = Vec::with_capacity(json.len() + 1);
        finished_line.extend_from_slice(json.as_bytes());
        finished_line.push(b'\n');

        let path = Self::path_for(directory, self.source, std::process::id());
        let mut guard = self
            .open
            .lock()
            .map_err(|_| io::Error::other("diagnostic JSONL writer mutex poisoned"))?;
        if guard.as_ref().is_none_or(|open| open.path != path) {
            std::fs::create_dir_all(directory)?;
            let file = OpenOptions::new().create(true).append(true).open(&path)?;
            *guard = Some(OpenLog {
                path: path.clone(),
                file,
            });
        }
        // The complete serialized object and newline are emitted while this
        // source's process-local mutex is held. Other processes and sources use
        // different files by construction.
        guard.as_mut().unwrap().file.write_all(&finished_line)?;
        guard.as_mut().unwrap().file.flush()
    }
}
