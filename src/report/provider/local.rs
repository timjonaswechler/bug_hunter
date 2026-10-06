//! No-follow traversal and atomic, no-clobber publication inside a directory capability.
use super::{FileReference, Outcome, Reference};
use crate::report::{Report, markdown, work};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, File, OpenOptions};
use serde::Serialize;
use std::{
    io::{self, BufReader, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Read,
    Write,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Error {
    InvalidPath {
        path: PathBuf,
    },
    Conflict {
        path: PathBuf,
    },
    Filesystem {
        operation: Operation,
        path: PathBuf,
        message: String,
    },
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath { path } => write!(f, "invalid report path: {}", path.display()),
            Self::Conflict { path } => write!(f, "report signature conflict: {}", path.display()),
            Self::Filesystem {
                operation,
                path,
                message,
            } => {
                let operation = match operation {
                    Operation::Read => "read",
                    Operation::Write => "write",
                };
                write!(f, "report {operation} {}: {message}", path.display())
            }
        }
    }
}
impl std::error::Error for Error {}

fn filesystem(operation: Operation, path: &Path, error: io::Error) -> Error {
    Error::Filesystem {
        operation,
        path: path.into(),
        message: error.to_string(),
    }
}

fn path_error(operation: Operation, path: &Path, error: io::Error) -> Error {
    if matches!(error.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)) {
        Error::InvalidPath { path: path.into() }
    } else {
        filesystem(operation, path, error)
    }
}

pub(crate) fn valid_output(path: &Path) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    !bytes.is_empty()
        && !bytes.contains(&0)
        && path.components().all(|c| matches!(c, Component::Normal(_)))
        // Path::components silently removes interior "." components.
        && !bytes.split(|b| *b == b'/' || (cfg!(windows) && *b == b'\\'))
            .any(|part| matches!(part, b"." | b".."))
}

fn directory(root: &Dir, output: &Path, cancel: &AtomicBool) -> Result<Dir, Error> {
    work::check(cancel).map_err(|e| filesystem(Operation::Write, output, e))?;
    if !valid_output(output) {
        return Err(Error::InvalidPath {
            path: output.into(),
        });
    }
    let mut directory = root
        .try_clone()
        .map_err(|e| filesystem(Operation::Read, output, e))?;
    let mut path = PathBuf::new();
    for component in output.components() {
        work::check(cancel).map_err(|e| filesystem(Operation::Write, output, e))?;
        let part = component.as_os_str();
        path.push(part);
        directory = match directory.open_dir_nofollow(part) {
            Ok(directory) => directory,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match directory.create_dir(part) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(path_error(Operation::Write, &path, error)),
                }
                directory
                    .open_dir_nofollow(part)
                    .map_err(|e| path_error(Operation::Read, &path, e))?
            }
            Err(error) => return Err(path_error(Operation::Read, &path, error)),
        };
    }
    Ok(directory)
}

/// None means absent, not "unreadable". Never follow a destination link, and
/// open nonblocking so a raced-in FIFO cannot hang submission.
fn existing(
    directory: &Dir,
    path: &Path,
    report: &Report,
    cancel: &AtomicBool,
) -> Result<Option<Outcome>, Error> {
    work::check(cancel).map_err(|e| filesystem(Operation::Read, path, e))?;
    let name = path.file_name().expect("generated report filename");
    match directory.symlink_metadata(name) {
        Ok(metadata) if !metadata.is_file() => {
            return Err(Error::InvalidPath { path: path.into() });
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(path_error(Operation::Read, path, error)),
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = directory
        .open_with(name, &options)
        .map_err(|e| path_error(Operation::Read, path, e))?;
    if !file
        .metadata()
        .map_err(|e| filesystem(Operation::Read, path, e))?
        .is_file()
    {
        return Err(Error::InvalidPath { path: path.into() });
    }
    if !markdown::has_signature(
        work::Reader {
            inner: BufReader::new(file),
            cancel,
        },
        report.signature(),
    )
    .map_err(|e| filesystem(Operation::Read, path, e))?
    {
        return Err(Error::Conflict { path: path.into() });
    }
    Ok(Some(Outcome::Existing {
        reference: reference(path),
    }))
}

fn reference(path: &Path) -> Reference {
    Reference::File(FileReference { path: path.into() })
}

struct Temporary<'a> {
    directory: &'a Dir,
    name: String,
    file: File,
    remove_on_drop: bool,
}
impl<'a> Temporary<'a> {
    fn new(directory: &'a Dir, path: &Path) -> Result<Self, Error> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..128 {
            let name = format!(
                ".woodpecker-report-{}-{}.tmp",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let mut options = OpenOptions::new();
            options
                .write(true)
                .create_new(true)
                .follow(FollowSymlinks::No);
            #[cfg(unix)]
            {
                use cap_std::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match directory.open_with(&name, &options) {
                Ok(file) => {
                    return Ok(Self {
                        directory,
                        name,
                        file,
                        remove_on_drop: true,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(filesystem(Operation::Write, path, error)),
            }
        }
        Err(filesystem(
            Operation::Write,
            path,
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "temporary report name collisions",
            ),
        ))
    }
}
impl Drop for Temporary<'_> {
    fn drop(&mut self) {
        // On any failed write/publication, preserve the primary error.
        if self.remove_on_drop {
            let _ = self.directory.remove_file(&self.name);
        }
    }
}

#[cfg(test)]
pub(crate) fn submit(root: &Dir, output: &Path, report: &Report) -> Result<Outcome, Error> {
    submit_cancellable(root, output, report, &AtomicBool::new(false))
}

pub(crate) fn submit_cancellable(
    root: &Dir,
    output: &Path,
    report: &Report,
    cancel: &AtomicBool,
) -> Result<Outcome, Error> {
    submit_with_cancel(root, output, report, cancel, |file, text| {
        for chunk in text.as_bytes().chunks(64 * 1024) {
            work::check(cancel)?;
            file.write_all(chunk)?;
        }
        work::check(cancel)?;
        file.sync_all()?;
        work::check(cancel)
    })
}

#[cfg(test)]
fn submit_with(
    root: &Dir,
    output: &Path,
    report: &Report,
    write: impl FnOnce(&mut File, &str) -> io::Result<()>,
) -> Result<Outcome, Error> {
    submit_with_cancel(root, output, report, &AtomicBool::new(false), write)
}

fn submit_with_cancel(
    root: &Dir,
    output: &Path,
    report: &Report,
    cancel: &AtomicBool,
    write: impl FnOnce(&mut File, &str) -> io::Result<()>,
) -> Result<Outcome, Error> {
    let directory = directory(root, output, cancel)?;
    let path = output.join(format!(
        "{}.md",
        report.signature().as_str().replace(':', "-")
    ));
    if let Some(outcome) = existing(&directory, &path, report, cancel)? {
        return Ok(outcome);
    }
    let mut temporary = Temporary::new(&directory, &path)?;
    write(&mut temporary.file, &report.to_markdown())
        .map_err(|e| filesystem(Operation::Write, &path, e))?;
    work::check(cancel).map_err(|e| filesystem(Operation::Write, &path, e))?;
    let result = match directory.hard_link(&temporary.name, &directory, path.file_name().unwrap()) {
        Ok(()) => Ok(Outcome::Created {
            reference: reference(&path),
        }),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            // The winning file is now complete. Inspect it even if its content
            // was supplied by another program, not by this provider.
            existing(&directory, &path, report, cancel)?.ok_or_else(|| {
                filesystem(
                    Operation::Read,
                    &path,
                    io::Error::new(io::ErrorKind::NotFound, "competing report disappeared"),
                )
            })
        }
        Err(error) => Err(path_error(Operation::Write, &path, error)),
    };
    if result.is_ok() {
        directory
            .remove_file(&temporary.name)
            .map_err(|e| filesystem(Operation::Write, &path, e))?;
        temporary.remove_on_drop = false;
    }
    result
}

#[cfg(test)]
mod tests;
