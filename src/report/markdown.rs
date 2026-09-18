use super::{Origin, Report, Signature, normalize};
use serde::Serialize;
use std::fmt::Write;
use std::io::{self, BufRead};

fn marker(signature: &Signature) -> String {
    format!("<!-- bug_hunter-signature: {} -->", signature.as_str())
}

/// Only an unambiguous standalone marker outside fenced diagnostic data counts.
/// Work on bytes so unrelated non-UTF-8 data does not disguise a conflicting file.
pub(crate) fn has_signature(mut reader: impl BufRead, signature: &Signature) -> io::Result<bool> {
    let expected = marker(signature);
    let mut line = Vec::new();
    let mut fence: Option<(u8, usize)> = None;
    let mut found = false;
    let mut count = 0;
    while reader.read_until(b'\n', &mut line)? != 0 {
        let content = line.strip_suffix(b"\n").unwrap_or(&line);
        let content = content.strip_suffix(b"\r").unwrap_or(content);
        let indent = content.iter().take_while(|b| **b == b' ').count();
        let trimmed = &content[indent..];
        let character = trimmed.first().copied().unwrap_or(0);
        let run = trimmed.iter().take_while(|b| **b == character).count();
        let delimiter = indent <= 3 && matches!(character, b'`' | b'~') && run >= 3;
        if let Some((opened, length)) = fence {
            if delimiter
                && character == opened
                && run >= length
                && trimmed[run..].iter().all(|b| matches!(b, b' ' | b'\t'))
            {
                fence = None;
            }
        } else if delimiter && (character != b'`' || !trimmed[run..].contains(&b'`')) {
            fence = Some((character, run));
        } else if content.starts_with(b"<!-- bug_hunter-signature:") {
            count += 1;
            found = content == expected.as_bytes();
        }
        line.clear();
    }
    Ok(count == 1 && found)
}

pub(super) fn render(report: &Report) -> String {
    let mut output = String::new();
    output.push_str("# ");
    // H1 text is the only user data outside a fence. Escape Markdown/HTML syntax
    // without changing the title used by callers or future issue providers.
    for c in report.title().chars() {
        if c.is_ascii_punctuation() {
            output.push('\\');
        }
        output.push(c);
    }
    write!(
        output,
        "\n\n{}\n\n## Failure\n\n",
        marker(report.signature())
    )
    .unwrap();
    output.push_str(
        "Missing diagnostic values are unavailable; JSON `null` denotes unavailable data.\n\n",
    );
    output.push_str("### Message\n\n");
    text(&mut output, report.failure().message());
    output.push_str("### Origin\n\n");
    // Keep the full backtrace in its own text block, not a JSON-escaped copy.
    let diagnostics = match report.failure().origin() {
        Origin::Panic {
            location,
            backtrace_status,
            ..
        } => serde_json::json!({
            "kind": "panic", "location": location, "backtrace_status": backtrace_status,
        }),
        Origin::TracingError { target, location } => serde_json::json!({
            "kind": "tracing_error", "target": target, "location": location,
        }),
        Origin::ProcessExit { status } => serde_json::json!({
            "kind": "process_exit", "status": status,
        }),
    };
    json(&mut output, &diagnostics);
    if let Origin::Panic { backtrace, .. } = report.failure().origin() {
        output.push_str("### Backtrace\n\n");
        text(&mut output, backtrace.as_deref());
    }
    let context = report.context();
    output.push_str("## Application\n\n");
    json(&mut output, &context.application);
    output.push_str("## Environment\n\n");
    #[derive(Serialize)]
    struct Environment<'a> {
        woodpecker_version: &'a str,
        protocol_version: u32,
        capabilities: &'a crate::session::Capabilities,
        tick: &'a crate::command::tick::Config,
        platform: &'a super::Platform,
        toolchain: &'a super::Toolchain,
    }
    json(
        &mut output,
        &Environment {
            woodpecker_version: &context.woodpecker_version,
            protocol_version: context.protocol_version,
            capabilities: &context.capabilities,
            tick: &context.tick,
            platform: &context.platform,
            toolchain: &context.toolchain,
        },
    );
    output.push_str("## Commands\n\n");
    json(&mut output, &context.commands);
    // Every block ends with two LFs; the document ends with exactly one.
    output.pop();
    output
}

fn text(output: &mut String, value: Option<&str>) {
    match value {
        Some(value) => fence(output, "text", &normalize::lf(value)),
        None => output.push_str("Unavailable.\n\n"),
    }
}

fn json(output: &mut String, value: &impl Serialize) {
    fence(
        output,
        "json",
        &serde_json::to_string_pretty(value).expect("report data is serializable"),
    );
}

fn fence(output: &mut String, language: &str, value: &str) {
    let mut longest = 0;
    let mut run = 0;
    for byte in value.bytes() {
        run = if byte == b'`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    let fence = "`".repeat(3.max(longest + 1));
    writeln!(output, "{fence}{language}").unwrap();
    output.push_str(value);
    if !value.ends_with('\n') {
        output.push('\n');
    }
    writeln!(output, "{fence}\n").unwrap();
}
