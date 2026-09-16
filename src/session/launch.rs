//! Intrinsic validation is separate from cancellable environment resolution.
use super::{Error, process};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub manifest_path: PathBuf,
    pub package: String,
    pub target: Target,
    pub features: Vec<String>,
    pub arguments: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Binary { name: String },
    Example { name: String },
}

impl Config {
    pub fn validate(&self) -> Result<(), Error> {
        let name = match &self.target {
            Target::Binary { name } | Target::Example { name } => name,
        };
        if self.manifest_path.as_os_str().is_empty()
            || self.package.trim().is_empty()
            || name.trim().is_empty()
        {
            return Err(Error::new(
                "invalid_config",
                "manifest, package and named target are required",
            ));
        }
        Ok(())
    }

    pub(super) fn resolve(&self, cancel: &Arc<AtomicBool>) -> Result<(PathBuf, Command), Error> {
        self.validate()?;
        if !self.manifest_path.is_file() {
            return Err(Error::new(
                "invalid_config",
                "manifest must be a regular file",
            ));
        }
        let manifest = self.manifest_path.canonicalize().map_err(Error::io)?;
        let project = manifest.parent().unwrap().to_path_buf();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut metadata = Command::new(&cargo);
        metadata
            .args([
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--manifest-path",
            ])
            .arg(&manifest)
            .current_dir(&project);
        let mut child = process::Process::spawn(metadata)?;
        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut closed = 0;
        let status = loop {
            if cancel.load(Ordering::Acquire) {
                return Err(Error::ended());
            }
            while let Ok(event) = child.events.try_recv() {
                match event {
                    process::Event::Line(line) => {
                        stdout.push_str(&line);
                        stdout.push('\n');
                    }
                    process::Event::Diagnostic(bytes) => {
                        stderr.push_str(&String::from_utf8_lossy(&bytes));
                        if stderr.len() > 8192 {
                            stderr = stderr
                                .chars()
                                .rev()
                                .take(4096)
                                .collect::<String>()
                                .chars()
                                .rev()
                                .collect();
                        }
                    }
                    process::Event::Closed(_) => closed += 1,
                    process::Event::Failed(channel, message) => {
                        return Err(Error::new("launch", format!("{channel}: {message}")));
                    }
                }
            }
            if let Some(status) = child.child.try_wait().map_err(Error::io)?
                && closed == 2
            {
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        if !status.success() {
            return Err(Error::new(
                "launch",
                format!("cargo metadata exited with {status}: {stderr}"),
            ));
        }
        let metadata: serde_json::Value =
            serde_json::from_str(&stdout).map_err(|e| Error::new("launch", e))?;
        let packages = metadata["packages"]
            .as_array()
            .ok_or_else(|| Error::new("launch", "missing packages"))?;
        if packages
            .iter()
            .filter(|p| p["name"] == self.package && p["version"].is_string())
            .count()
            != 1
        {
            return Err(Error::new(
                "launch",
                "package and version must resolve uniquely",
            ));
        }
        let mut command = Command::new(cargo);
        command
            .args(["run", "--quiet", "--manifest-path"])
            .arg(manifest)
            .args(["--package", &self.package])
            .current_dir(&project);
        match &self.target {
            Target::Binary { name } => {
                command.args(["--bin", name]);
            }
            Target::Example { name } => {
                command.args(["--example", name]);
            }
        }
        if !self.features.is_empty() {
            command.arg("--features").arg(self.features.join(","));
        }
        command.arg("--").args(&self.arguments);
        Ok((project, command))
    }
}
