// Included in server::tests; isolated PATH prevents real GitHub IO.
#[cfg(unix)]
#[test]
fn blocked_provider_accumulates_failure_snapshots_without_blocking_session() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    const ROOT: &str = "WOODPECKER_REPORT_LOAD_ROOT";
    const COUNT: usize = 64;
    if let Some(root) = std::env::var_os(ROOT) {
        let root = PathBuf::from(root);
        let mut inner = inner();
        let state = Arc::get_mut(&mut inner.0).unwrap();
        // Retain all measurement events; queue retention is independent of Activity.
        state.activity_bytes = 64 * 1024 * 1024;
        state.timeout = Duration::from_secs(10);
        let mut config = report_create();
        config.launch.target = launch::Target::Example {
            name: "activity_fixture".into(),
        };
        config.launch.arguments = vec!["--continuous-failures".into()];
        config.report.tracing_errors = true;
        config.report.provider = report::provider::Config::Github;
        let detail = inner.create(config).unwrap();
        fs::write(
            root.join("artifact-dir"),
            detail.artifact_dir.to_string_lossy().as_bytes(),
        )
        .unwrap();
        let entry = inner.select(&detail.id).unwrap();
        wait(|| entry.detail().state != Lifecycle::Starting);
        assert_eq!(entry.detail().state, Lifecycle::Ready);
        let mut samples = Vec::new();
        for number in 1..=COUNT {
            submit_tick(&entry);
            wait(|| {
                let events = activity(&entry);
                events
                    .iter()
                    .filter(|e| e["kind"] == "completed" && e["command"] == "tick.warp.start")
                    .count()
                    == number
                    && events
                        .iter()
                        .filter(|e| e["kind"] == "event" && e["event"]["kind"] == "failure")
                        .count()
                        == number
            });
            wait(|| root.join("entered").exists());
            assert_eq!(entry.detail().state, Lifecycle::Ready);
            assert!(entry.snapshot().pending.is_empty());
            assert!(!activity(&entry).iter().any(|e| e["kind"] == "report"));
            // One active provider, all subsequent reports still waiting.
            assert_eq!(
                fs::read_dir(&root)
                    .unwrap()
                    .flatten()
                    .filter(|e| e.file_name().to_string_lossy().starts_with("request-"))
                    .count(),
                1
            );
            if [1, 16, 32, COUNT].contains(&number) {
                let output = Command::new("/bin/ps")
                    .args(["-o", "rss=", "-p", &std::process::id().to_string()])
                    .output()
                    .unwrap();
                assert!(output.status.success());
                let rss_kib: u64 = String::from_utf8(output.stdout)
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                samples.push(serde_json::json!({"failures":number,"active":1,"waiting":number-1,"server_rss_kib":rss_kib}));
                fs::write(
                    root.join("samples.json"),
                    serde_json::to_vec_pretty(&samples).unwrap(),
                )
                .unwrap();
            }
        }
        // No more ticks: release the barrier and require every accepted failure's
        // report exactly once, with its original immutable command prefix.
        fs::write(root.join("release"), "").unwrap();
        wait(|| {
            activity(&entry)
                .iter()
                .filter(|e| e["kind"] == "report")
                .count()
                == COUNT
        });
        let events = activity(&entry);
        let reports: Vec<_> = events.iter().filter(|e| e["kind"] == "report").collect();
        let mut sizes = Vec::new();
        for (index, event) in reports.iter().enumerate() {
            assert_eq!(event["result"]["status"], "submitted", "{event}");
            assert_eq!(
                event["report"]["context"]["commands"]
                    .as_array()
                    .unwrap()
                    .len(),
                (index + 1).min(50)
            );
            sizes.push(serde_json::to_vec(&event["report"]).unwrap().len());
        }
        assert!(sizes.last().unwrap() > sizes.first().unwrap());
        assert_eq!(
            fs::read_dir(&root)
                .unwrap()
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("request-"))
                .count(),
            COUNT
        );
        entry.stop();
        wait(|| entry.done.load(Ordering::Acquire));
        assert_eq!(entry.detail().state, Lifecycle::Ended);
        inner.stop(false);
        wait(|| inner.finished().is_some());
        assert!(inner.finished().unwrap().is_ok());
        let pid: i32 = fs::read_to_string(root.join("entered"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
        fs::write(root.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "acceptance":"passed","failures":COUNT,"samples":samples,
            "serialized_report_bytes":sizes,"total_serialized_report_bytes":sizes.iter().sum::<usize>(),
            "measurement_scope":"bounded; RSS includes activity, history and allocator, not queue alone",
            "session":detail.id
        })).unwrap()).unwrap();
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/report-queue-load")
        .join(random_id().unwrap());
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        bin.join("gh"),
        r#"#!/bin/sh
set -eu
root="$WOODPECKER_REPORT_LOAD_ROOT"
if [ "$4" = GET ]; then printf '[]\n'; exit 0; fi
/bin/cat > "$root/request-$$"
printf '%s' "$$" > "$root/entered"
while [ ! -f "$root/release" ]; do /bin/sleep 0.01; done
printf '{"number":42,"html_url":"https://example.test/org/repo/issues/42"}\n'
"#,
    )
    .unwrap();
    fs::set_permissions(bin.join("gh"), fs::Permissions::from_mode(0o700)).unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact","server::tests::blocked_provider_accumulates_failure_snapshots_without_blocking_session","--nocapture"])
        .env(ROOT,&root).env("PATH",std::env::join_paths(paths).unwrap()).output().unwrap();
    fs::write(root.join("stdout.log"), &output.stdout).unwrap();
    fs::write(root.join("stderr.log"), &output.stderr).unwrap();
    eprintln!("report queue evidence: {}", root.display());
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
