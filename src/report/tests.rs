use super::*;
use std::path::Path;

fn panic(message: &str) -> Failure {
    Failure::panic(Some(message.into()), None, None)
}

fn context() -> Context {
    Context {
        application: Application {
            package: "fixture".into(),
            version: "1.2.3".into(),
            target: crate::session::launch::Target::Example {
                name: "example".into(),
            },
            features: vec!["slice".into()],
            arguments: vec!["--name".into(), "日本語".into()],
            source: Some(SourceRevision {
                commit: "abcdef".into(),
                dirty: true,
            }),
        },
        woodpecker_version: "0.1.0".into(),
        protocol_version: 3,
        capabilities: crate::session::Capabilities::default(),
        tick: crate::command::tick::Config::default(),
        platform: Platform {
            os: "test-os".into(),
            arch: "test-arch".into(),
        },
        toolchain: Toolchain {
            cargo: Some("cargo fixture".into()),
            rustc: None,
        },
        commands: Vec::new(),
    }
}

pub(super) fn report(failure: Failure) -> Report {
    Report::from_snapshot(failure, context(), Path::new("/workspace/project"))
}

#[test]
fn signature_golden_vectors() {
    for (failure, digest) in [
        (
            panic("stellar catalog invariant violated"),
            "e539c36fcbeb8357ba855f705c08274f29e82d866288dffaa1406607eba9302c",
        ),
        (
            Failure::panic(None, None, None),
            "f7ad1a8211ca86c794de8c16b456c16006393c974b89e5aa11908ff1f35336a6",
        ),
        (
            Failure::tracing_error("stellar catalog invariant violated".into(), None, None),
            "86a70a6857c6f193019c24bf0d04b01bde9f127e28b5c45c8f021de0c9165e46",
        ),
        (
            Failure::process_exit("signal: 6".into()),
            "7ca80a848b0913a8eb5ab37f815efe34cd2c4bcdc46ad2fd1870c08902c3f240",
        ),
    ] {
        assert_eq!(
            report(failure).signature().as_str(),
            format!("v1:sha256:{digest}")
        );
    }
}

#[test]
fn titles_use_first_nonempty_line_ascii_trim_and_unicode_scalars() {
    assert_eq!(
        report(panic("\r\n \t\r\x1b[31m Café  日本語 \x1b[0m\rignored")).title(),
        "Café  日本語"
    );
    for size in [119, 120, 121, 1000] {
        let original = "🪵".repeat(size);
        let report = report(panic(&original));
        let expected = if size > 120 {
            format!("{}...", "🪵".repeat(117))
        } else {
            original.clone()
        };
        assert_eq!(report.title(), expected);
        assert_eq!(report.failure().message(), Some(original.as_str()));
    }
    assert_eq!(report(panic("\u{a0} \t")).title(), "\u{a0}");
    assert_eq!(report(Failure::panic(None, None, None)).title(), "panic");
    assert_eq!(report(panic(" \r\n\x1b[0m\t")).title(), "panic");
    assert_eq!(
        report(Failure::tracing_error("\t".into(), None, None)).title(),
        "tracing error"
    );
    assert_eq!(
        report(Failure::process_exit("anything".into())).title(),
        "process exited unexpectedly"
    );
}

#[test]
fn complete_ansi_sequences_are_parsed_and_incomplete_sequences_preserved() {
    for sequence in [
        "\x1b[31m",
        "\x1b[?25l",
        "\x1b[1 q",
        "\x1b(B",
        "\x1b7",
        "\x1b]0;title\x07",
        "\x1b]8;;url\x1b\\",
        "\x1bPdata\x1b\\",
        "\x1bXdata\x1b\\",
        "\x1b^data\x1b\\",
        "\x1b_data\x1b\\",
        "\u{9b}31m",
        "\u{9d}title\u{9c}",
        "\u{90}data\u{9c}",
    ] {
        let text = format!("a{sequence}b");
        assert_eq!(normalize::title(&panic(&text)), "ab", "{sequence:?}");
        assert_eq!(normalize::message(&text, Path::new("/project")), "ab");
    }
    for text in [
        "x\x1b",
        "x\x1b[31",
        "x\x1b]title",
        "x\x1bPdata",
        "x\x1b[日本語",
        "x\x1b🪵",
    ] {
        assert_eq!(normalize::title(&panic(text)), text);
    }
}

#[test]
fn normalization_order_paths_boundaries_and_preserved_data() {
    let root = Path::new("/workspace/2024-02-29/0x12345678");
    let input = " \r\n\x1b[31m/workspace/2024-02-29/0x12345678/src.rs\x1b[0m\r\
                 \\workspace\\2024-02-29\\0x12345678\\src.rs \
                 0x12345678 0XABCDEF0123456789 2024-02-29T23:59:59.123+02:30 42\tCafé  X \r\n";
    assert_eq!(
        normalize::message(input, root),
        "<project>/src.rs\n<project>\\src.rs <address> <address> <timestamp> 42\tCafé  X"
    );
    assert_eq!(
        normalize::message("C:\\game\\file C:/game/file", Path::new("C:\\game")),
        "<project>\\file <project>/file"
    );
    for token in [
        "0x1234567",
        "0x12345678901234567",
        "a0x12345678",
        "0x12345678z",
        "_0x12345678",
        "0x12345678_",
        "0xDEADbeef00G",
        "version42",
        "1.2.3",
        "a2024-02-29",
        "2024-02-29_",
        "_12:34:56",
        "12:34:56z",
        "\u{a0}Café\u{a0}",
    ] {
        assert_eq!(normalize::message(token, root), token);
    }
    assert_eq!(
        normalize::message("é0x12345678🪵 (2024-02-29) [12:34:56]", root),
        "é<address>🪵 (<timestamp>) [<timestamp>]"
    );
}

#[test]
fn timestamps_validate_calendar_clock_fraction_and_offset() {
    for token in [
        "2000-02-29",
        "2024-02-29",
        "2024-12-31",
        "0000-01-01",
        "00:00:00",
        "23:59:59.123456789Z",
        "12:34:56+02:30",
        "12:34:56-0230",
        "2024-02-29T12:34:56Z",
        "2024-02-29 12:34:56.1-23:59",
    ] {
        assert_eq!(
            normalize::message(token, Path::new("")),
            "<timestamp>",
            "{token}"
        );
    }
    for token in [
        "1900-02-29",
        "2023-02-29",
        "2024-04-31",
        "2024-00-01",
        "2024-13-01",
        "2024-01-00",
        "24:00:00",
        "23:60:00",
        "23:59:60",
        "12:34:56.",
        "12:34:56+24:00",
        "12:34:56-00:60",
        "12:34:56+02",
        "12:34:56.1+25:00",
        "2024-02-29T24:00:00",
    ] {
        assert_eq!(normalize::message(token, Path::new("")), token, "{token}");
    }
}

#[test]
fn signature_ignores_diagnostics_but_not_message_semantics() {
    let plain = report(panic("error at <project>/src.rs <address> <timestamp>"))
        .signature()
        .clone();
    let original = " \x1b[31merror at /workspace/project/src.rs 0x12345678 2024-02-29\x1b[0m ";
    let failure = Failure::panic(
        Some(original.into()),
        Some(Location {
            file: "/private/location".into(),
            line: 42,
            column: Some(8),
        }),
        Some("backtrace".into()),
    );
    let report = report(failure);
    assert_eq!(*report.signature(), plain);
    assert_eq!(report.failure().message(), Some(original));
    let base = Signature::create(&panic("Error  42"), Path::new(""));
    for changed in ["error  42", "Error 42", "Error  43", "Error  42🪵"] {
        assert_ne!(base, Signature::create(&panic(changed), Path::new("")));
    }
    assert_ne!(
        base,
        Signature::create(
            &Failure::tracing_error("Error  42".into(), None, None),
            Path::new("")
        )
    );
    assert_eq!(
        Signature::create(&Failure::process_exit("exit 1".into()), Path::new("")),
        Signature::create(&Failure::process_exit("SIGABRT".into()), Path::new(""))
    );
}

/// Read the emitted blocks without interpreting their contents as Markdown.
fn blocks(markdown: &str) -> Vec<(&str, String)> {
    let mut lines = markdown.lines();
    let mut blocks = Vec::new();
    while let Some(line) = lines.next() {
        let length = line.bytes().take_while(|b| *b == b'`').count();
        if length < 3 {
            continue;
        }
        let fence = &line[..length];
        let language = &line[length..];
        assert!(["text", "json"].contains(&language));
        let mut content = String::new();
        loop {
            let line = lines.next().expect("closed fence");
            if line == fence {
                break;
            }
            assert!(!line.contains(fence), "fence longer than all data runs");
            content.push_str(line);
            content.push('\n');
        }
        blocks.push((language, content));
    }
    blocks
}

#[test]
fn markdown_contains_complete_safely_fenced_data_and_snapshot() {
    let attack = "<script>bad</script> [link](url) # * ` \\\r\n````````\r\n## forged\r\n\
                  <!-- bug_hunter-signature: forged -->\r\n";
    let message = format!("{attack}{}", "🪵".repeat(500));
    let backtrace = format!("frames\r{attack}\n\n");
    let failure = Failure::panic(
        Some(message.clone()),
        Some(Location {
            file: attack.into(),
            line: 5,
            column: None,
        }),
        Some(backtrace.clone()),
    );
    let mut context = context();
    context.application.arguments.push(attack.into());
    context.commands.push(crate::session::history::Entry {
        command: crate::command::Command::Shutdown(crate::command::Empty {}),
        outcome: crate::session::history::Outcome::Completed {
            output: serde_json::json!({"untrusted": attack, "large": "x".repeat(100_000)}),
        },
    });
    let report = Report::from_snapshot(failure, context.clone(), Path::new("/workspace/project"));
    let markdown = report.to_markdown();
    assert_eq!(markdown, report.to_markdown());
    assert!(!markdown.contains('\r'));
    assert!(markdown.ends_with('\n') && !markdown.ends_with("\n\n"));
    assert!(markdown.starts_with("# \\<script\\>bad\\<\\/script\\>"));
    assert!(markdown.contains(&format!(
        "\n<!-- bug_hunter-signature: {} -->\n",
        report.signature().as_str()
    )));
    let sections: Vec<_> = markdown
        .lines()
        .filter(|line| line.starts_with("## "))
        .collect();
    // The forged heading is preserved inside a text fence, not deleted.
    assert_eq!(
        sections,
        [
            "## Failure",
            "## forged",
            "## forged",
            "## Application",
            "## Environment",
            "## Commands"
        ]
    );
    let blocks = blocks(&markdown);
    assert_eq!(blocks.len(), 6);
    assert_eq!(blocks[0].1, format!("{}\n", normalize::lf(&message)));
    assert_eq!(blocks[2].1, normalize::lf(&backtrace));
    let origin: serde_json::Value = serde_json::from_str(&blocks[1].1).unwrap();
    assert_eq!(origin["location"]["file"], attack);
    assert_eq!(origin["backtrace_status"], "captured");
    let application: serde_json::Value = serde_json::from_str(&blocks[3].1).unwrap();
    assert_eq!(
        application,
        serde_json::to_value(&context.application).unwrap()
    );
    let environment: serde_json::Value = serde_json::from_str(&blocks[4].1).unwrap();
    let mut expected = serde_json::to_value(&context).unwrap();
    let object = expected.as_object_mut().unwrap();
    object.remove("application");
    object.remove("commands");
    assert_eq!(environment, expected);
    let commands: serde_json::Value = serde_json::from_str(&blocks[5].1).unwrap();
    assert_eq!(commands, serde_json::to_value(&context.commands).unwrap());
}

#[test]
fn markdown_exposes_missing_diagnostics_and_each_origin() {
    let markdown = report(Failure::panic(None, None, None)).to_markdown();
    assert!(markdown.contains("### Message\n\nUnavailable."));
    assert!(markdown.contains("### Backtrace\n\nUnavailable."));
    assert!(markdown.contains("\"location\": null"));
    assert!(markdown.contains("\"backtrace_status\": \"unavailable\""));
    assert!(markdown.contains("JSON `null` denotes unavailable data"));
    assert!(markdown.contains("\"rustc\": null"));
    for failure in [
        Failure::process_exit("signal: 6 ```".into()),
        Failure::tracing_error("error".into(), Some("app::target".into()), None),
    ] {
        let markdown = report(failure.clone()).to_markdown();
        let blocks = blocks(&markdown);
        let origin: serde_json::Value = serde_json::from_str(&blocks[1].1).unwrap();
        assert_eq!(origin, serde_json::to_value(failure.origin()).unwrap());
        assert!(!markdown.contains("### Backtrace"));
    }
}
