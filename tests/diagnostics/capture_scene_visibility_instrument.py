"""Instrument disposable Bevy 0.19.1 and woodpecker source copies.

The base Bevy edits reuse capture_causality_instrument. Further edits add
process/session metadata and adapter request mapping. Nothing below target/ is
used as an input path.
"""

from pathlib import Path
import shutil

from diagnostics.capture_causality_instrument import instrument as instrument_bevy_base


JSONL_WRITER = Path(__file__).with_name("capture_scene_visibility_jsonl.rs")


def replace(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one source block, found {count}")
    path.write_text(text.replace(old, new))


def instrument_bevy(root: Path) -> None:
    instrument_bevy_base(root)
    source = root / "src"
    shutil.copy2(JSONL_WRITER, source / "capture_scene_visibility_jsonl.rs")
    replace(
        source / "lib.rs",
        "mod capture_causality;\n",
        "mod capture_causality;\nmod capture_scene_visibility_jsonl;\n",
    )
    diagnostic = source / "capture_causality.rs"
    replace(
        diagnostic,
        """use std::{
    fs::OpenOptions,
    io::Write,
    sync::atomic::{AtomicU64, Ordering},
};
""",
        """use std::sync::atomic::{AtomicU64, Ordering};

use crate::capture_scene_visibility_jsonl::DiagnosticJsonl;
""",
    )
    replace(
        diagnostic,
        "static TEXTURE_ID: AtomicU64 = AtomicU64::new(0);\n",
        """static TEXTURE_ID: AtomicU64 = AtomicU64::new(0);
static LOG: DiagnosticJsonl = DiagnosticJsonl::new(
    "bevy_render",
    "WOODPECKER_CAPTURE_SCENE_VISIBILITY_LOG_DIR",
);
""",
    )
    replace(
        diagnostic,
        """pub(crate) fn enabled() -> bool {
    std::env::var_os("WOODPECKER_CAPTURE_CAUSALITY_LOG").is_some()
}
""",
        """pub(crate) fn enabled() -> bool {
    LOG.enabled()
}
""",
    )
    replace(
        diagnostic,
        """pub(crate) fn log(fields: &str) {
    let Some(path) = std::env::var_os("WOODPECKER_CAPTURE_CAUSALITY_LOG") else {
        return;
    };
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{{\\\"source\\\":\\\"bevy_render\\\",{fields}}}");
}
""",
        """pub(crate) fn log(fields: &str) {
    if !LOG.enabled() {
        return;
    }
    let artifact_dir = std::env::var("WOODPECKER_ARTIFACT_DIR").unwrap_or_default();
    let session_id = std::path::Path::new(&artifact_dir)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let row = format!(
        "{{\\\"source\\\":\\\"bevy_render\\\",\\\"pid\\\":{},\\\"session_id\\\":{:?},\\\"artifact_dir\\\":{:?},{fields}}}",
        std::process::id(),
        session_id,
        artifact_dir,
    );
    LOG.write_json(&row)
        .expect("failed to write bevy_render visibility diagnostic");
}
""",
    )
    replace(
        diagnostic,
        'std::env::var_os("WOODPECKER_CAPTURE_CAUSALITY_RAW_DIR")',
        'std::env::var_os("WOODPECKER_CAPTURE_SCENE_VISIBILITY_RAW_DIR")',
    )


def instrument_adapter(root: Path) -> None:
    source = root / "src"
    shutil.copy2(JSONL_WRITER, source / "capture_scene_visibility_jsonl.rs")
    replace(
        source / "lib.rs",
        "pub mod command;\n",
        "mod capture_scene_visibility_jsonl;\npub mod command;\n",
    )
    capture = source / "session/screenshot/capture.rs"
    replace(
        capture,
        'const COMMAND: &str = "screenshot.capture";\n',
        '''const COMMAND: &str = "screenshot.capture";
static VISIBILITY_LOG: crate::capture_scene_visibility_jsonl::DiagnosticJsonl =
    crate::capture_scene_visibility_jsonl::DiagnosticJsonl::new(
        "woodpecker_adapter",
        "WOODPECKER_CAPTURE_SCENE_VISIBILITY_LOG_DIR",
    );

fn visibility_log(value: serde_json::Value) {
    if !VISIBILITY_LOG.enabled() {
        return;
    }
    let artifact_dir = std::env::var("WOODPECKER_ARTIFACT_DIR").unwrap_or_default();
    let session_id = Path::new(&artifact_dir)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let row = serde_json::json!({
        "source": "woodpecker_adapter",
        "pid": std::process::id(),
        "session_id": session_id,
        "artifact_dir": artifact_dir,
        "fields": value,
    });
    VISIBILITY_LOG
        .write_json(&row.to_string())
        .expect("failed to write woodpecker_adapter visibility diagnostic");
}
''',
    )
    replace(
        capture,
        '''fn response(id: u64, result: Result<Output, Diagnostic>) -> Message {
    match result {
''',
        '''fn response(id: u64, result: Result<Output, Diagnostic>) -> Message {
    visibility_log(match &result {
        Ok(output) => serde_json::json!({
            "event": "adapter_response",
            "request_id": id,
            "outcome": "completed",
            "path": output.path,
            "width": output.width,
            "height": output.height,
            "overwritten": output.overwritten,
        }),
        Err(error) => serde_json::json!({
            "event": "adapter_response",
            "request_id": id,
            "outcome": "rejected",
            "error_code": error.code,
        }),
    });
    match result {
''',
    )
    replace(
        capture,
        '''                let entity = world.spawn(Screenshot::window(job.window)).id();
                service.active = Some(Active {
''',
        '''                let entity = world.spawn(Screenshot::window(job.window)).id();
                visibility_log(serde_json::json!({
                    "event": "adapter_capture_started",
                    "request_id": job.id,
                    "screenshot_entity": entity.to_string(),
                    "window_entity": job.window.to_string(),
                    "path": job.path,
                }));
                service.active = Some(Active {
''',
    )


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("bevy_render_copy", type=Path)
    parser.add_argument("woodpecker_copy", type=Path)
    args = parser.parse_args()
    instrument_bevy(args.bevy_render_copy.resolve())
    instrument_adapter(args.woodpecker_copy.resolve())
