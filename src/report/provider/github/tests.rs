use super::*;
use crate::report::{self, Destination, Failure, provider};
use cap_std::fs::Dir;
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!(
                "github-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
        fs::create_dir_all(&root).unwrap();
        let fixture = Self(root);
        fixture.write("search.stdout", "[]");
        fixture.write("publish.stdout", issue(42, Value::Null).to_string());
        for op in ["search", "publish"] {
            fixture.write(&format!("{op}.stderr"), "");
            fixture.write(&format!("{op}.status"), "0");
        }
        fixture.write(
            "gh",
            r#"#!/bin/sh
set -eu
case "$4" in
  GET) operation=search;;
  POST) operation=publish;;
  *) exit 99;;
esac
printf '%s\n' "$operation" >> calls
printf '%s\n' "$@" > "$operation.args"
printf '%s\n' "$PWD" > "$operation.cwd"
printf '%s\n' "$GH_PROMPT_DISABLED" > "$operation.prompt"
printf '%s\n' "${GH_REPO-}" "${GH_HOST-}" > "$operation.context"
# Fill stderr before reading stdin to catch serialized pipe handling.
/bin/cat "$operation.stderr" >&2
/bin/cat > "$operation.stdin"
/bin/cat "$operation.stdout"
exit "$(/bin/cat "$operation.status")"
"#,
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(fixture.0.join("gh"), fs::Permissions::from_mode(0o700)).unwrap();
        }
        fixture
    }
    fn write(&self, name: &str, content: impl AsRef<[u8]>) {
        fs::write(self.0.join(name), content).unwrap();
    }
    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.0.join(name)).unwrap()
    }
    fn run(&self, report: &Report) -> Result<Outcome, Error> {
        submit_with(report, |operation, input| {
            let mut command = command(&self.0, self.0.join("gh"), operation);
            command
                .env("GH_REPO", "example/project")
                .env("GH_HOST", "github.example.test");
            execute(command, operation, input)
        })
    }
    fn destination(&self, provider: provider::Config) -> Destination {
        let root = self.0.join("artifacts");
        fs::create_dir_all(&root).unwrap();
        Destination::new(
            Arc::new(Dir::open_ambient_dir(&root, cap_std::ambient_authority()).unwrap()),
            report::Config {
                tracing_errors: false,
                output: "reports".into(),
                provider,
            },
            self.0.clone(),
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn report() -> Report {
    report::tests::report(Failure::panic(
        Some("--title='日本語' <test> `fence`\nmore".into()),
        None,
        None,
    ))
}
fn marker(report: &Report) -> String {
    format!(
        "<!-- bug_hunter-signature: {} -->",
        report.signature().as_str()
    )
}
fn issue(number: u64, body: Value) -> Value {
    json!({"number":number, "html_url":format!("https://github.example.test/example/project/issues/{number}"),
        "body":body, "state":"closed"})
}
fn reference(number: u64) -> Reference {
    Reference::Issue {
        identifier: number.to_string(),
        url: format!("https://github.example.test/example/project/issues/{number}"),
    }
}

#[test]
fn paginated_closed_issue_match_skips_pull_requests_and_uses_exact_markers() {
    let fixture = Fixture::new();
    let report = report();
    let marker = marker(&report);
    let mut pull = issue(1, json!(marker));
    pull["pull_request"] = json!({"url":"https://github.example.test/pulls/1"});
    fixture.write(
        "search.stdout",
        format!(
            "{}\n{}\n{}",
            json!([
                pull,
                issue(2, json!(format!("prefix {marker}"))),
                issue(3, json!(format!("```text\n{marker}\n```"))),
                issue(4, json!(marker.replace("v1:", "v2:"))),
                issue(5, json!(format!("{marker}\n{marker}"))),
                issue(6, Value::Null)
            ]),
            json!([issue(7, json!(format!("# existing\r\n{marker}\r\n")))]),
            json!([])
        ),
    );
    let destination = fixture.destination(provider::Config::Github);
    assert_eq!(
        destination
            .submit_with(&report, || fixture.run(&report))
            .unwrap(),
        Outcome::Existing {
            reference: reference(7)
        }
    );
    assert_eq!(fixture.read("calls"), "search\n");
    assert_eq!(
        fixture.read("search.args"),
        "api\nrepos/{owner}/{repo}/issues?state=all&per_page=100\n--method\nGET\n--paginate\n"
    );
    assert!(fixture.read("search.stdin").is_empty());
    assert_eq!(
        fixture.read("search.cwd").trim(),
        fixture.0.to_str().unwrap()
    );
    assert_eq!(fixture.read("search.prompt"), "1\n");
    assert_eq!(
        fixture.read("search.context"),
        "example/project\ngithub.example.test\n"
    );
    assert!(!fixture.0.join("artifacts/reports").exists());
}

#[test]
fn publish_sends_unchanged_title_and_complete_markdown_over_stdin_without_local_copy() {
    let fixture = Fixture::new();
    // Payload and diagnostic output both exceed pipe capacity.
    let report = report::tests::report(Failure::panic(
        Some(format!(
            "--title='日本語' <test> `fence`\n{}",
            "large message\n".repeat(20_000)
        )),
        None,
        None,
    ));
    fixture.write("publish.stderr", "diagnostic".repeat(20_000));
    let destination = fixture.destination(provider::Config::Github);
    assert_eq!(
        destination
            .submit_with(&report, || fixture.run(&report))
            .unwrap(),
        Outcome::Created {
            reference: reference(42)
        }
    );
    assert_eq!(fixture.read("calls"), "search\npublish\n");
    assert_eq!(
        fixture.read("publish.args"),
        "api\nrepos/{owner}/{repo}/issues\n--method\nPOST\n--input\n-\n"
    );
    let payload: Value = serde_json::from_str(&fixture.read("publish.stdin")).unwrap();
    assert_eq!(
        payload,
        json!({"title":report.title(), "body":report.to_markdown()})
    );
    assert_eq!(fixture.read("publish.prompt"), "1\n");
    assert!(!fixture.0.join("artifacts/reports").exists());
}

#[test]
fn malformed_or_incomplete_search_never_publishes_even_after_an_earlier_hit() {
    let report = report();
    let mut missing_body = issue(1, Value::Null);
    missing_body.as_object_mut().unwrap().remove("body");
    for response in [
        "".into(),
        "null".into(),
        "{}".into(),
        "[[]]".into(),
        "[] trailing".into(),
        json!([{"number":1, "body":null}]).to_string(),
        json!([issue(0, Value::Null)]).to_string(),
        json!([missing_body]).to_string(),
        json!([issue(1, json!(42))]).to_string(),
        format!(
            "{}\n{{\"error\":\"broken next page\"}}",
            json!([issue(7, json!(marker(&report)))])
        ),
    ] {
        let fixture = Fixture::new();
        fixture.write("search.stdout", response);
        assert!(matches!(
            fixture.run(&report),
            Err(Error::InvalidResponse {
                operation: Operation::Search,
                ..
            })
        ));
        assert_eq!(fixture.read("calls"), "search\n");
    }
}

#[test]
fn malformed_publish_response_is_not_a_claimed_remote_success() {
    for response in [
        "".into(),
        "[]".into(),
        "{}".into(),
        "not json".into(),
        json!({"number":"42", "html_url":"https://github.example.test/issues/42"}).to_string(),
        json!({"number":42, "html_url":""}).to_string(),
        json!({"number":42, "html_url":"javascript:alert(1)"}).to_string(),
        json!({"number":0, "html_url":"https://github.example.test/issues/0"}).to_string(),
        json!({"number":42, "html_url":"https://", "pull_request":{}}).to_string(),
        format!("{} trailing", issue(42, Value::Null)),
    ] {
        let fixture = Fixture::new();
        fixture.write("publish.stdout", response);
        assert!(matches!(
            fixture.run(&report()),
            Err(Error::InvalidResponse {
                operation: Operation::Publish,
                ..
            })
        ));
        assert_eq!(fixture.read("calls"), "search\npublish\n");
    }
}

#[test]
fn process_failures_preserve_operation_status_and_diagnostics_without_auth_guessing() {
    let fixture = Fixture::new();
    for operation in [Operation::Search, Operation::Publish] {
        let error = execute(
            command(&fixture.0, fixture.0.join("missing-gh"), operation),
            operation,
            None,
        )
        .unwrap_err();
        assert!(matches!(error, Error::Unavailable { operation: op, .. } if op == operation));
        let name = match operation {
            Operation::Search => "search",
            Operation::Publish => "publish",
        };
        fixture.write(&format!("{name}.status"), "4");
        fixture.write(
            &format!("{name}.stderr"),
            "auth or network diagnostic, not a stable category",
        );
        let error = fixture.run(&report()).unwrap_err();
        assert!(
            matches!(&error, Error::CommandFailed { operation: op, message }
            if *op == operation && message.contains("4") && message.contains("auth or network diagnostic"))
        );
        fixture.write(&format!("{name}.status"), "0");
    }
    fixture.write("search.stdout", [0xff, b'\n']);
    assert!(matches!(
        fixture.run(&report()),
        Err(Error::InvalidResponse {
            operation: Operation::Search,
            ..
        })
    ));
}

#[test]
fn every_remote_error_falls_back_and_keeps_both_causes_when_local_also_fails() {
    let report = report();
    for operation in [Operation::Search, Operation::Publish] {
        for remote in [
            Error::Unavailable {
                operation,
                message: "unavailable".into(),
            },
            Error::CommandFailed {
                operation,
                message: "failed".into(),
            },
            Error::InvalidResponse {
                operation,
                message: "invalid".into(),
            },
        ] {
            let fixture = Fixture::new();
            let destination = fixture.destination(provider::Config::Github);
            // First creates the fallback, second finds it. Both remain explicit fallbacks.
            for _ in 0..2 {
                let Outcome::Fallback {
                    reference,
                    provider_error,
                } = destination
                    .submit_with(&report, || Err(remote.clone()))
                    .unwrap()
                else {
                    panic!("expected explicit fallback");
                };
                assert_eq!(provider_error, provider::Error::Github(remote.clone()));
                assert_eq!(
                    fs::read_to_string(fixture.0.join("artifacts").join(reference.path)).unwrap(),
                    report.to_markdown()
                );
            }
            let path = Path::new("reports").join(format!(
                "{}.md",
                report.signature().as_str().replace(':', "-")
            ));
            destination
                .root
                .write(&path, b"conflicting local file")
                .unwrap();
            let error = destination
                .submit_with(&report, || Err(remote.clone()))
                .unwrap_err();
            assert!(matches!(&error, report::Error::FallbackFailed {
                provider: provider::Error::Github(cause), local: provider::local::Error::Conflict { path: p }
            } if cause == &remote && p == &path));
            assert!(std::error::Error::source(&error).is_some());
            assert!(error.to_string().contains("local fallback failed"));
            assert_eq!(
                destination.root.read(&path).unwrap(),
                b"conflicting local file"
            );
        }
    }
}

#[test]
fn actual_process_error_and_local_selection_use_the_same_persistence_path() {
    let fixture = Fixture::new();
    let report = report();
    fixture.write("publish.status", "1");
    fixture.write("publish.stderr", "fixture remote failure");
    let destination = fixture.destination(provider::Config::Github);
    assert!(matches!(
        destination
            .submit_with(&report, || fixture.run(&report))
            .unwrap(),
        Outcome::Fallback {
            provider_error: provider::Error::Github(Error::CommandFailed {
                operation: Operation::Publish,
                ..
            }),
            ..
        }
    ));
    let local = fixture.destination(provider::Config::Local);
    assert!(matches!(
        local
            .submit_with(&report, || panic!("Local must not invoke gh"))
            .unwrap(),
        Outcome::Existing { .. }
    ));
    assert_eq!(fixture.read("calls"), "search\npublish\n");
    assert!(serde_json::from_value::<provider::Config>(json!({"kind":"github"})).is_ok());
    assert!(
        serde_json::from_value::<provider::Config>(
            json!({"kind":"github","repository":"forbidden"})
        )
        .is_err()
    );
}

#[test]
fn cancelled_queued_work_starts_neither_gh_nor_local_fallback() {
    let fixture = Fixture::new();
    let destination = fixture.destination(provider::Config::Github);
    let result =
        destination.submit_using(&report(), &std::sync::atomic::AtomicBool::new(true), || {
            panic!("cancelled work must not start gh")
        });
    assert!(matches!(result, report::work::Result::Interrupted { .. }));
    assert!(!fixture.0.join("calls").exists());
    assert!(!fixture.0.join("artifacts/reports").exists());
}
