// Included in server::tests to share its owned server fixture and wait helpers.
fn report_create() -> Create {
    let mut config = create("caught");
    config.launch.manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    config.launch.package = "woodpecker".into();
    config.launch.target = launch::Target::Example {
        name: "observation_fixture".into(),
    };
    config
}

fn activity(entry: &entry::Entry) -> Vec<serde_json::Value> {
    let protocol::Result::Activity { entries, .. } = entry.poll(None) else {
        panic!("unexpected activity gap");
    };
    entries.into_iter().map(|e| e.event).collect()
}

fn submit_tick(entry: &entry::Entry) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    assert!(matches!(
        runtime.block_on(
            entry.submit(
                crate::command::tick::warp::Start {
                    ticks: 1,
                    pace: None,
                }
                .into()
            )
        ),
        protocol::Result::Pending { .. }
    ));
}

#[test]
fn report_submit_errors_remain_activity_without_failing_the_live_session() {
    let mut inner = inner();
    Arc::get_mut(&mut inner.0).unwrap().activity_bytes = 2 * 1024 * 1024;
    let detail = inner.create(report_create()).unwrap();
    let entry = inner.select(&detail.id).unwrap();
    wait(|| entry.detail().state != Lifecycle::Starting);
    assert_eq!(entry.detail().state, Lifecycle::Ready);
    std::fs::write(detail.artifact_dir.join("reports"), b"not a directory").unwrap();
    submit_tick(&entry);
    wait(|| activity(&entry).iter().any(|e| e["kind"] == "report"));
    let events = activity(&entry);
    let report = events.iter().find(|e| e["kind"] == "report").unwrap();
    assert_eq!(report["report"]["title"], "caught panic");
    assert_eq!(report["result"]["status"], "failed");
    assert_eq!(report["result"]["error"]["kind"], "local");
    assert_eq!(entry.detail().state, Lifecycle::Ready);
    entry.stop();
    wait(|| entry.done.load(Ordering::Acquire));
    let events = activity(&entry);
    assert_eq!(entry.detail().state, Lifecycle::Ended);
    assert_eq!(events.iter().filter(|e| e["kind"] == "report").count(), 1);
    assert!(
        events
            .iter()
            .any(|e| e["kind"] == "lifecycle" && e["state"] == "Ended")
    );
}

#[test]
fn genuine_failure_during_shutdown_is_reported_before_owner_completion() {
    let mut inner = inner();
    Arc::get_mut(&mut inner.0).unwrap().activity_bytes = 2 * 1024 * 1024;
    let detail = inner.create(create("failure_on_shutdown")).unwrap();
    let entry = inner.select(&detail.id).unwrap();
    wait(|| entry.detail().state != Lifecycle::Starting);
    assert_eq!(entry.detail().state, Lifecycle::Ready);
    std::fs::write(
        detail.artifact_dir.join("failure.marker"),
        report::marker::fixture(report::Failure::panic(
            Some("panic during shutdown".into()),
            None,
            None,
        )),
    )
    .unwrap();
    entry.stop();
    wait(|| entry.done.load(Ordering::Acquire));
    assert_eq!(entry.detail().state, Lifecycle::Failed);
    let events = activity(&entry);
    let failures: Vec<_> = events
        .iter()
        .filter(|e| e["kind"] == "event" && e["event"]["kind"] == "failure")
        .collect();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0]["event"]["failure"]["origin"]["kind"], "panic");
    assert!(
        events
            .iter()
            .any(|e| e["kind"] == "event" && e["event"]["kind"] == "ended")
    );
    let reports: Vec<_> = events.iter().filter(|e| e["kind"] == "report").collect();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["report"]["title"], "panic during shutdown");
    assert_eq!(reports[0]["result"]["status"], "submitted");
    assert_eq!(
        std::fs::read_dir(detail.artifact_dir.join("reports"))
            .unwrap()
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn slow_reports_outlive_sessions_and_share_server_deadline_and_forced_cleanup() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    const ROOT: &str = "WOODPECKER_SERVER_REPORT_TEST_ROOT";
    if let Some(root) = std::env::var_os(ROOT) {
        let root = PathBuf::from(root);
        for phase in ["graceful", "deadline", "force"] {
            let phase_root = root.join(phase);
            fs::create_dir_all(&phase_root).unwrap();
            fs::write(root.join("phase"), phase).unwrap();
            let mut inner = inner();
            let state = Arc::get_mut(&mut inner.0).unwrap();
            state.activity_bytes = 2 * 1024 * 1024;
            state.timeout = Duration::from_millis(800);
            let entries: Vec<_> = (0..2)
                .map(|_| {
                    let mut config = report_create();
                    config.report.provider = report::provider::Config::Github;
                    let detail = inner.create(config).unwrap();
                    inner.select(&detail.id).unwrap()
                })
                .collect();
            wait(|| {
                let states: Vec<_> = entries.iter().map(|e| e.detail()).collect();
                assert!(
                    states.iter().all(|e| e.state != Lifecycle::Failed),
                    "{states:?}"
                );
                states.iter().all(|e| e.state == Lifecycle::Ready)
            });
            for entry in &entries {
                submit_tick(entry);
            }
            wait(|| {
                for entry in &entries {
                    let events = activity(entry);
                    assert!(
                        !events.iter().any(|e| e["kind"] == "report"),
                        "provider exited before fixture barrier in {phase}: {events:?}"
                    );
                }
                fs::read_dir(&phase_root)
                    .unwrap()
                    .filter(|e| {
                        e.as_ref()
                            .unwrap()
                            .file_name()
                            .to_string_lossy()
                            .starts_with("entered-")
                    })
                    .count()
                    == 2
            });
            // Provider is blocked, but events and additional commands still progress.
            for entry in &entries {
                assert!(
                    activity(entry)
                        .iter()
                        .any(|e| e["kind"] == "event" && e["event"]["kind"] == "failure")
                );
                assert!(!activity(entry).iter().any(|e| e["kind"] == "report"));
                submit_tick(entry);
                wait(|| {
                    activity(entry)
                        .iter()
                        .filter(|e| e["kind"] == "completed" && e["command"] == "tick.warp.start")
                        .count()
                        == 2
                });
            }
            inner.stop(false);
            let started = Instant::now();
            let deadline = inner.directory.lock().unwrap().deadline;
            wait(|| entries.iter().all(|e| e.detail().state.terminal()));
            assert!(entries.iter().all(|e| e.detail().state == Lifecycle::Ended));
            assert!(entries.iter().all(|e| !e.done.load(Ordering::Acquire)));
            assert!(
                inner.finished().is_none(),
                "session end must not finish pending reports"
            );
            inner.stop(false);
            assert_eq!(inner.directory.lock().unwrap().deadline, deadline);
            if phase == "graceful" {
                fs::write(phase_root.join("release"), "").unwrap();
            } else if phase == "force" {
                inner.stop(true);
            }
            let mut result = None;
            wait(|| {
                result = inner.finished();
                result.is_some()
            });
            assert_eq!(result.unwrap().is_ok(), phase == "graceful");
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "one shared deadline, not one per report"
            );
            for entry in &entries {
                let events = activity(entry);
                let ended = events
                    .iter()
                    .position(|e| e["kind"] == "lifecycle" && e["state"] == "Ended")
                    .unwrap();
                let completed = events.iter().position(|e| e["kind"] == "report").unwrap();
                assert!(completed > ended);
                assert_eq!(
                    events[completed]["report"]["context"]["commands"]
                        .as_array()
                        .unwrap()
                        .len(),
                    1,
                    "snapshot precedes later commands"
                );
                assert_eq!(
                    events[completed]["result"]["status"],
                    if phase == "graceful" {
                        "submitted"
                    } else {
                        "interrupted"
                    }
                );
                assert!(
                    !entry.detail().artifact_dir.join("reports").exists(),
                    "no fallback starts after forced cancellation"
                );
            }
            for file in fs::read_dir(&phase_root).unwrap() {
                let file = file.unwrap();
                if file.file_name().to_string_lossy().starts_with("entered-") {
                    let pid: i32 = fs::read_to_string(file.path()).unwrap().parse().unwrap();
                    assert_ne!(
                        unsafe { libc::kill(pid, 0) },
                        0,
                        "gh process survived cleanup"
                    );
                }
            }
        }
        return;
    }
    // Only the isolated child test receives the fake gh in PATH.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/server-report-tests")
        .join(random_id().unwrap());
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        bin.join("gh"),
        r#"#!/bin/sh
set -eu
base="$WOODPECKER_SERVER_REPORT_TEST_ROOT"
phase="$(/bin/cat "$base/phase")"
root="$base/$phase"
if [ "$4" = GET ]; then printf '[]\n'; exit 0; fi
/bin/cat > "$root/request-$$"
if [ "$phase" = graceful ]; then
  printf '%s' "$$" > "$root/entered-$$"
  while [ ! -f "$root/release" ]; do /bin/sleep 0.01; done
  printf '{"number":42,"html_url":"https://example.test/org/repo/issues/42"}\n'
else
  # This descendant holds both pipes open. Cleanup must kill the process group.
  /bin/sleep 60 &
  printf '%s' "$!" > "$root/descendant-$$"
  printf '%s' "$$" > "$root/entered-$$"
  wait
fi
"#,
    )
    .unwrap();
    fs::set_permissions(bin.join("gh"), fs::Permissions::from_mode(0o700)).unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "server::tests::slow_reports_outlive_sessions_and_share_server_deadline_and_forced_cleanup", "--nocapture"])
        .env(ROOT, &root).env("PATH", std::env::join_paths(paths).unwrap()).output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
