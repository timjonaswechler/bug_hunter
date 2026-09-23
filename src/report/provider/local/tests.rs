use super::*;
use crate::report::{self, Failure};
use std::{
    fs,
    sync::{Arc, Barrier},
};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "report-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn root(&self, name: &str) -> Dir {
        let path = self.0.join(name);
        fs::create_dir_all(&path).unwrap();
        Dir::open_ambient_dir(path, cap_std::ambient_authority()).unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn report() -> Report {
    report::tests::report(Failure::panic(Some("fixture failure".into()), None, None))
}

fn path(report: &Report) -> PathBuf {
    Path::new("reports").join(format!(
        "{}.md",
        report.signature().as_str().replace(':', "-")
    ))
}

#[test]
fn creates_complete_markdown_and_preserves_existing_reports_and_root_isolation() {
    let sandbox = Sandbox::new();
    let root = sandbox.root("first");
    let second = sandbox.root("second");
    let report = report();
    let path = path(&report);
    assert!(
        matches!(submit(&root, Path::new("reports"), &report).unwrap(),
        Outcome::Created { reference: Reference::File(FileReference { path: p }) } if p == path)
    );
    let expected = report.to_markdown();
    assert_eq!(root.read(&path).unwrap(), expected.as_bytes());
    assert!(!second.exists(&path));
    // Same identity, different diagnostics: never refresh the stored snapshot.
    let changed = report::tests::report(Failure::panic(
        Some("fixture failure".into()),
        None,
        Some("new backtrace".into()),
    ));
    assert_ne!(changed.to_markdown(), expected);
    assert!(matches!(
        submit(&root, Path::new("reports"), &changed).unwrap(),
        Outcome::Existing { .. }
    ));
    assert_eq!(root.read(&path).unwrap(), expected.as_bytes());
    assert!(matches!(
        submit(&second, Path::new("reports"), &changed).unwrap(),
        Outcome::Created { .. }
    ));
    assert_eq!(
        second.read(&path).unwrap(),
        changed.to_markdown().as_bytes()
    );
    assert_eq!(root.read_dir("reports").unwrap().count(), 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(sandbox.0.join("first").join(path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn output_validation_is_shared_with_config_and_rejects_interior_dot_components() {
    let sandbox = Sandbox::new();
    let root = sandbox.root("root");
    for invalid in [
        "",
        ".",
        "..",
        "./reports",
        "reports/.",
        "reports/./nested",
        "reports/../out",
        "/absolute",
        "reports\0",
    ] {
        let config = report::Config {
            tracing_errors: false,
            output: invalid.into(),
            provider: super::super::Config::Local,
        };
        assert_eq!(
            config.validate().unwrap_err().code(),
            "invalid_config",
            "{invalid:?}"
        );
        assert!(matches!(
            submit(&root, Path::new(invalid), &report()),
            Err(Error::InvalidPath { .. })
        ));
    }
    assert_eq!(root.entries().unwrap().count(), 0);
    assert!(matches!(
        submit(&root, Path::new("nested/日本語"), &report()).unwrap(),
        Outcome::Created { .. }
    ));
}

#[test]
fn marker_matching_requires_a_full_unambiguous_line_outside_diagnostic_fences() {
    let sandbox = Sandbox::new();
    let root = sandbox.root("root");
    root.create_dir("reports").unwrap();
    let report = report();
    let path = path(&report);
    let marker = format!(
        "<!-- bug_hunter-signature: {} -->",
        report.signature().as_str()
    );
    for content in [
        String::new(),
        "no marker".into(),
        marker.replace("v1:", "v2:"),
        marker.replace("sha256:", "other:"),
        format!("prefix {marker}"),
        format!("{marker} suffix"),
        format!(" {marker}"),
        format!("{marker}\n{marker}"),
        format!("<!-- bug_hunter-signature: other -->\n{marker}"),
        format!("````text\n```\n{marker}\n````\n"),
        format!("   ~~~~text\n~~~\n{marker}\n   ~~~~ \t\n"),
        format!("```\n{marker}"),
        format!("> {marker}"),
    ] {
        root.write(&path, content.as_bytes()).unwrap();
        assert_eq!(
            submit(&root, Path::new("reports"), &report).unwrap_err(),
            Error::Conflict { path: path.clone() },
            "{content:?}"
        );
        assert_eq!(root.read(&path).unwrap(), content.as_bytes());
    }
    root.write(&path, b"\xff invalid data").unwrap();
    assert!(matches!(
        submit(&root, Path::new("reports"), &report),
        Err(Error::Conflict { .. })
    ));
    for content in [
        marker.clone(),
        format!("# report\r\n\r\n{marker}\r\n"),
        format!("````text\n{marker}\n````\n{marker}\n"),
        format!("~~~text\n{marker}\n~~~~~\n{marker}\n"),
    ] {
        root.write(&path, content.as_bytes()).unwrap();
        assert!(
            matches!(
                submit(&root, Path::new("reports"), &report).unwrap(),
                Outcome::Existing { .. }
            ),
            "{content:?}"
        );
    }
}

#[test]
fn concurrent_writers_publish_exactly_one_complete_snapshot() {
    let sandbox = Sandbox::new();
    let root = sandbox.root("root");
    let report = report();
    let path = path(&report);
    let barrier = Arc::new(Barrier::new(8));
    let outcomes = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let barrier = barrier.clone();
                let root = &root;
                let report = &report;
                let path = &path;
                scope.spawn(move || {
                    submit_with(root, Path::new("reports"), report, |file, text| {
                        // Every writer has observed absence and created its own temp file.
                        assert!(!root.exists(path));
                        file.write_all(text.as_bytes())?;
                        file.sync_all()?;
                        barrier.wait();
                        Ok(())
                    })
                    .unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| matches!(o, Outcome::Created { .. }))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| matches!(o, Outcome::Existing { .. }))
            .count(),
        7
    );
    assert_eq!(root.read(&path).unwrap(), report.to_markdown().as_bytes());
    assert_eq!(root.read_dir("reports").unwrap().count(), 1);
}

#[test]
fn competing_conflict_is_read_without_overwrite_and_partial_writes_are_cleaned_up() {
    let sandbox = Sandbox::new();
    let root = sandbox.root("root");
    let report = report();
    let path = path(&report);
    let error = submit_with(&root, Path::new("reports"), &report, |file, text| {
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        root.write(&path, b"competing incompatible report")?;
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error, Error::Conflict { path: path.clone() });
    assert_eq!(root.read(&path).unwrap(), b"competing incompatible report");
    assert_eq!(root.read_dir("reports").unwrap().count(), 1);
    root.remove_file(&path).unwrap();
    for complete in [false, true] {
        let error = submit_with(&root, Path::new("reports"), &report, |file, text| {
            file.write_all(if complete {
                text.as_bytes()
            } else {
                b"partial"
            })?;
            assert!(!root.exists(&path));
            Err(io::Error::other(if complete {
                "injected sync failure"
            } else {
                "injected write failure"
            }))
        })
        .unwrap_err();
        assert!(
            matches!(error, Error::Filesystem { operation: Operation::Write, path: p, .. } if p == path)
        );
        assert!(!root.exists(&path));
        assert_eq!(root.read_dir("reports").unwrap().count(), 0);
    }
}

#[cfg(unix)]
#[test]
fn symlinks_directories_fifos_and_swapped_destinations_are_rejected() {
    use std::os::unix::fs::symlink;
    let sandbox = Sandbox::new();
    let root = sandbox.root("root");
    let outside = sandbox.root("outside");
    outside.write("keep", b"unchanged").unwrap();
    root.create_dir("inside").unwrap();
    for (name, target) in [
        ("absolute", sandbox.0.join("outside")),
        ("relative", "../outside".into()),
        ("in-root", "inside".into()),
        ("dangling", "absent".into()),
    ] {
        symlink(target, sandbox.0.join("root").join(name)).unwrap();
        assert!(
            matches!(
                submit(&root, Path::new(name), &report()),
                Err(Error::InvalidPath { .. })
            ),
            "{name}"
        );
    }
    root.write("regular", b"file, not directory").unwrap();
    assert!(matches!(
        submit(&root, Path::new("regular/reports"), &report()),
        Err(Error::InvalidPath { .. })
    ));
    root.create_dir("reports").unwrap();
    let report = report();
    let path = path(&report);
    for target in ["../../outside/keep", "../../outside/missing", "../regular"] {
        symlink(target, sandbox.0.join("root").join(&path)).unwrap();
        assert!(matches!(
            submit(&root, Path::new("reports"), &report),
            Err(Error::InvalidPath { .. })
        ));
        root.remove_file(&path).unwrap();
    }
    root.create_dir(&path).unwrap();
    assert!(matches!(
        submit(&root, Path::new("reports"), &report),
        Err(Error::InvalidPath { .. })
    ));
    root.remove_dir(&path).unwrap();
    let fifo = std::ffi::CString::new(
        sandbox
            .0
            .join("root")
            .join(&path)
            .as_os_str()
            .as_encoded_bytes(),
    )
    .unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert!(matches!(
        submit(&root, Path::new("reports"), &report),
        Err(Error::InvalidPath { .. })
    ));
    root.remove_file(&path).unwrap();
    let error = submit_with(&root, Path::new("reports"), &report, |file, text| {
        file.write_all(text.as_bytes())?;
        symlink("../../outside/keep", sandbox.0.join("root").join(&path))?;
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error, Error::InvalidPath { .. }));
    assert_eq!(outside.read("keep").unwrap(), b"unchanged");
    assert_eq!(outside.entries().unwrap().count(), 1);
    assert_eq!(root.read_dir("reports").unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn unreadable_existing_file_returns_read_error_and_does_not_publish() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    let root = sandbox.root("root");
    root.create_dir("reports").unwrap();
    let report = report();
    let path = path(&report);
    root.write(&path, b"keep").unwrap();
    fs::set_permissions(
        sandbox.0.join("root").join(&path),
        fs::Permissions::from_mode(0o0),
    )
    .unwrap();
    let result = submit(&root, Path::new("reports"), &report);
    fs::set_permissions(
        sandbox.0.join("root").join(&path),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    // Root bypasses Unix mode bits; it still must not overwrite a conflict.
    if unsafe { libc::geteuid() } == 0 {
        assert!(matches!(result, Err(Error::Conflict { .. })));
    } else {
        assert!(
            matches!(result, Err(Error::Filesystem { operation: Operation::Read, path: p, .. }) if p == path)
        );
    }
    assert_eq!(root.read(&path).unwrap(), b"keep");
    assert_eq!(root.read_dir("reports").unwrap().count(), 1);
}

#[test]
fn failed_publication_returns_write_error_without_partial_destination() {
    let sandbox = Sandbox::new();
    let root = sandbox.root("root");
    let report = report();
    let path = path(&report);
    let error = submit_with(&root, Path::new("reports"), &report, |file, text| {
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        // A removed source makes the actual filesystem publication fail.
        let temporary = root.read_dir("reports")?.next().unwrap()?.file_name();
        root.remove_file(Path::new("reports").join(temporary))?;
        Ok(())
    })
    .unwrap_err();
    assert!(
        matches!(&error, Error::Filesystem { operation: Operation::Write, path: p, .. } if p == &path)
    );
    assert!(!root.exists(path));
    assert_eq!(root.read_dir("reports").unwrap().count(), 0);
    let wrapped = report::Error::Local(error);
    assert!(std::error::Error::source(&wrapped).is_some());
    assert!(wrapped.to_string().contains("write"));
}

#[test]
fn cancellation_before_and_during_write_never_publishes_partial_data() {
    let sandbox = Sandbox::new();
    let root = sandbox.root("root");
    let report = report();
    let cancel = AtomicBool::new(true);
    assert!(submit_cancellable(&root, Path::new("reports"), &report, &cancel).is_err());
    assert!(!root.exists("reports"));
    cancel.store(false, Ordering::Release);
    assert!(
        submit_with_cancel(
            &root,
            Path::new("reports"),
            &report,
            &cancel,
            |file, text| {
                file.write_all(text.as_bytes())?;
                cancel.store(true, Ordering::Release);
                Ok(())
            }
        )
        .is_err()
    );
    assert_eq!(root.read_dir("reports").unwrap().count(), 0);
    assert!(!root.exists(path(&report)));
}
