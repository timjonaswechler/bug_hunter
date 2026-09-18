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

    pub(super) fn resolve(&self, cancel: &Arc<AtomicBool>) -> Result<Resolved, Error> {
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
        let package = packages.iter().find(|p| p["name"] == self.package).unwrap();
        let package_dir = package["manifest_path"]
            .as_str()
            .and_then(|p| std::path::Path::new(p).parent());
        let source = package_dir.and_then(|directory| {
            let commit = optional_command(
                Command::new("git")
                    .args(["rev-parse", "--verify", "HEAD"])
                    .current_dir(directory),
                cancel,
            )?;
            let dirty = optional_command(
                Command::new("git")
                    .args(["status", "--porcelain", "--untracked-files=normal"])
                    .current_dir(directory),
                cancel,
            )?;
            Some(crate::report::SourceRevision {
                commit,
                dirty: !dirty.is_empty(),
            })
        });
        let application = crate::report::Application {
            package: self.package.clone(),
            version: package["version"].as_str().unwrap().into(),
            target: self.target.clone(),
            features: self.features.clone(),
            arguments: self.arguments.clone(),
            source,
        };
        let toolchain = crate::report::Toolchain {
            cargo: optional_command(
                Command::new(&cargo).arg("--version").current_dir(&project),
                cancel,
            ),
            rustc: optional_command(
                Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                    .arg("--version")
                    .current_dir(&project),
                cancel,
            ),
        };
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
        Ok(Resolved {
            project,
            command,
            application,
            toolchain,
        })
    }
}

pub(super) struct Resolved {
    pub project: PathBuf,
    pub command: Command,
    pub application: crate::report::Application,
    pub toolchain: crate::report::Toolchain,
}

// Metadata is optional, bounded and cancellable. A missing or hung tool must not prevent launch.
fn optional_command(command: &mut Command, cancel: &AtomicBool) -> Option<String> {
    let mut owned = Command::new(command.get_program());
    owned.args(command.get_args());
    if let Some(dir) = command.get_current_dir() {
        owned.current_dir(dir);
    }
    let mut process = process::Process::spawn(owned).ok()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    let mut output = String::new();
    let mut closed = 0;
    loop {
        if cancel.load(Ordering::Acquire) || std::time::Instant::now() >= deadline {
            return None;
        }
        for _ in 0..128 {
            let Ok(event) = process.events.try_recv() else {
                break;
            };
            match event {
                process::Event::Line(line) => {
                    output.push_str(&line);
                    output.push('\n');
                    if output.len() > 64 * 1024 {
                        return None;
                    }
                }
                process::Event::Closed(_) => closed += 1,
                process::Event::Failed(..) => return None,
                process::Event::Diagnostic(_) => {}
            }
        }
        if let Some(status) = process.child.try_wait().ok()?
            && closed == 2
        {
            return status.success().then(|| output.trim().to_owned());
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_metadata_is_trimmed_and_missing_failed_or_hung_tools_are_nonfatal() {
        let cancel = AtomicBool::new(false);
        assert_eq!(
            optional_command(
                Command::new("sh").args(["-c", "printf '  version\\n'"]),
                &cancel
            ),
            Some("version".into())
        );
        assert!(
            optional_command(&mut Command::new("/nonexistent/woodpecker-tool"), &cancel).is_none()
        );
        assert!(optional_command(Command::new("sh").args(["-c", "exit 2"]), &cancel).is_none());
        let start = std::time::Instant::now();
        assert!(optional_command(Command::new("sh").args(["-c", "sleep 30"]), &cancel).is_none());
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        assert!(
            optional_command(
                Command::new("sh").args(["-c", "sleep 30"]),
                &AtomicBool::new(true)
            )
            .is_none()
        );
    }
}
