use super::Diagnostic;
use cap_std::fs::{Dir, OpenOptions};
use std::{
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

pub(super) fn failed(error: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::new("screenshot_failed", error)
}

fn path_error(error: io::Error) -> Diagnostic {
    if error.kind() == io::ErrorKind::PermissionDenied {
        Diagnostic::new("invalid_screenshot_path", error)
    } else {
        failed(error)
    }
}

/// All filesystem operations are relative to this directory capability, including
/// symlink resolution. Re-check at write time, not just when accepting the command.
pub(super) fn prepare(root: &Dir, path: &str) -> Result<bool, Diagnostic> {
    super::validate(path)?;
    let parent = Path::new(path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty());
    if let Some(parent) = parent {
        match root.open_dir(parent) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                root.create_dir_all(parent).map_err(path_error)?;
                root.open_dir(parent).map_err(path_error)?;
            }
            Err(error) => return Err(path_error(error)),
        }
    }
    match root.metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(_) => Err(failed("screenshot destination is not a regular file")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // A dangling in-root link is still an existing entry to replace.
            Ok(root.symlink_metadata(path).is_ok())
        }
        Err(error) => Err(path_error(error)),
    }
}

pub(super) fn write(root: &Dir, path: &str, png: &[u8]) -> Result<bool, Diagnostic> {
    let overwritten = prepare(root, path)?;
    let path = Path::new(path);
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let directory = root.open_dir(parent).map_err(path_error)?;
    let name = path.file_name().expect("validated path");
    let (temporary, mut file) = loop {
        let temporary = format!(
            ".woodpecker-{}-{}.tmp",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        );
        match directory.open_with(&temporary, OpenOptions::new().write(true).create_new(true)) {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(failed(error)),
        }
    };
    let result = (|| {
        file.write_all(png).map_err(failed)?;
        file.sync_all().map_err(failed)?;
        directory
            .rename(&temporary, &directory, name)
            .map_err(failed)?;
        Ok(overwritten)
    })();
    if result.is_err() {
        let _ = directory.remove_file(&temporary);
    }
    result
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::fs;

    pub(crate) struct Sandbox(std::path::PathBuf);
    impl Sandbox {
        pub(crate) fn new() -> Self {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join(format!(
                    "screenshot-test-{}-{}",
                    std::process::id(),
                    NEXT_FILE.fetch_add(1, Ordering::Relaxed)
                ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        pub(crate) fn root(&self, name: &str) -> Dir {
            let path = self.0.join(name);
            fs::create_dir(&path).unwrap();
            Dir::open_ambient_dir(path, cap_std::ambient_authority()).unwrap()
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn replacement_is_atomic_and_roots_are_independent() {
        let sandbox = Sandbox::new();
        let first = sandbox.root("first");
        let second = sandbox.root("second");
        assert!(!write(&first, "images/ä.png", b"first").unwrap());
        assert!(write(&first, "images/ä.png", b"replacement").unwrap());
        assert!(!write(&second, "images/ä.png", b"second").unwrap());
        assert_eq!(first.read("images/ä.png").unwrap(), b"replacement");
        assert_eq!(second.read("images/ä.png").unwrap(), b"second");
        first.create_dir("directory.png").unwrap();
        assert_eq!(
            write(&first, "directory.png", b"bad").unwrap_err().code,
            "screenshot_failed"
        );
        assert!(first.metadata("directory.png").unwrap().is_dir());
        assert_eq!(first.read_dir("images").unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_and_replacement_after_acceptance_are_rejected() {
        use std::os::unix::fs::symlink;
        let sandbox = Sandbox::new();
        let root = sandbox.root("root");
        let outside = sandbox.root("outside");
        outside.write("keep.png", b"unchanged").unwrap();
        symlink(sandbox.0.join("outside"), sandbox.0.join("root/absolute")).unwrap();
        symlink("../outside", sandbox.0.join("root/relative")).unwrap();
        symlink("../outside/keep.png", sandbox.0.join("root/leaf.png")).unwrap();
        symlink(
            "../outside/missing.png",
            sandbox.0.join("root/dangling.png"),
        )
        .unwrap();
        for path in [
            "absolute/new.png",
            "relative/new.png",
            "leaf.png",
            "dangling.png",
        ] {
            assert_eq!(
                write(&root, path, b"bad").unwrap_err().code,
                "invalid_screenshot_path",
                "{path}"
            );
        }
        assert!(!prepare(&root, "later/image.png").unwrap());
        root.remove_dir("later").unwrap();
        symlink("../outside", sandbox.0.join("root/later")).unwrap();
        assert_eq!(
            write(&root, "later/image.png", b"bad").unwrap_err().code,
            "invalid_screenshot_path"
        );
        assert_eq!(outside.read("keep.png").unwrap(), b"unchanged");
        assert_eq!(outside.entries().unwrap().count(), 1);
        root.create_dir("inside").unwrap();
        symlink("inside", sandbox.0.join("root/link")).unwrap();
        assert!(!write(&root, "link/valid.png", b"safe").unwrap());
        assert_eq!(root.read("inside/valid.png").unwrap(), b"safe");
    }
}
