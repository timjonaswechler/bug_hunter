"""Build and run a bounded visible/covered/visible macOS capture experiment.

All patched dependencies and copied package sources live below target/. The
normal screenshot guard remains enabled. Raw Bevy readbacks are recorded by the
temporary bevy_render copy before that guard sees them.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
TESTS = ROOT / "tests"
sys.path.insert(0, str(TESTS))
from mesh_picking import rgb_pixels  # noqa: E402
from slice import CLI  # noqa: E402
from diagnostics.capture_causality_instrument import instrument  # noqa: E402


def registry_crate(name, version):
    matches = list(Path.home().glob(f".cargo/registry/src/*/{name}-{version}"))
    if len(matches) != 1:
        raise RuntimeError(f"expected one cached {name} {version}, found {matches}")
    return matches[0]


def run_checked(arguments, **kwargs):
    print("+", " ".join(map(str, arguments)), flush=True)
    subprocess.run(arguments, check=True, **kwargs)


def prepare(work):
    deps = work / "deps"
    app = work / "bevy_test_apps"
    shutil.copytree(registry_crate("bevy_render", "0.19.1"), deps / "bevy_render")
    instrument(deps / "bevy_render")
    shutil.copytree(
        ROOT / "bevy_test_apps",
        app,
        ignore=shutil.ignore_patterns("target"),
    )
    native = app / "src/bin/capture_causality_native.rs"
    shutil.copy2(TESTS / "diagnostics/capture_causality_native.rs", native)
    blend = (app / "src/bin/blend_modes.rs").read_text()
    blend = blend.replace(
        "use rand::{Rng, SeedableRng, rngs::StdRng};",
        "use rand::{Rng, SeedableRng, rngs::StdRng};\n"
        "#[cfg(target_os = \"macos\")]\nmod capture_causality_native;",
    )
    blend = blend.replace(
        'title: "Controlled blend modes test".into(),',
        'title: std::env::var("WOODPECKER_CAUSALITY_TITLE")'
        '.unwrap_or_else(|_| "Woodpecker capture causality".into()),',
    )
    blend = blend.replace(
        "fn main() {\n    let mut app = App::new();",
        "fn main() {\n    let mut app = App::new();\n"
        "    #[cfg(target_os = \"macos\")]\n"
        "    capture_causality_native::install(&mut app);",
    )
    (app / "src/bin/blend_modes.rs").write_text(blend)

    manifest = (app / "Cargo.toml").read_text()
    manifest = manifest.replace(
        'woodpecker = { path = "../", optional = true }',
        f'woodpecker = {{ path = "{ROOT}", optional = true }}',
    )
    manifest += f"""

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6.4"
objc2-app-kit = {{ version = "0.3.2", default-features = false, features = ["std", "NSResponder", "NSView", "NSWindow"] }}
raw-window-handle = "0.6"

[patch.crates-io]
bevy_render = {{ path = "{deps / "bevy_render"}" }}
"""
    (app / "Cargo.toml").write_text(manifest)
    fixture = work / "capture-causality.toml"
    fixture.write_text(
        f"""version = 1

[launch]
manifest_path = "{app / "Cargo.toml"}"
package = "bevy_test_apps"
features = ["slice"]
arguments = []

[launch.target]
kind = "binary"
name = "blend_modes"

[tick.pace]
kind = "as_fast_as_possible"

[report]
tracing_errors = false
output = "reports"

[report.provider]
kind = "local"
"""
    )
    helper = work / "capture-causality-deck"
    run_checked(
        [
            "xcrun",
            "swiftc",
            str(TESTS / "diagnostics/capture_causality_deck.swift"),
            "-o",
            str(helper),
        ],
        cwd=ROOT,
    )
    run_checked(
        ["cargo", "build", "--offline", "--locked", "--features", "cli", "--bin",
         "woodpecker"],
        cwd=ROOT,
    )
    run_checked(
        ["cargo", "build", "--offline", "--manifest-path", str(app / "Cargo.toml"),
         "--features", "slice", "--bin", "blend_modes"],
        cwd=ROOT,
    )
    return fixture, helper


def wait_for_native(log_path, predicate, description, after_sample=0, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if log_path.exists():
            rows = [json.loads(line) for line in log_path.read_text().splitlines()]
            native = [
                row["fields"] for row in rows
                if row.get("source") == "appkit_main"
                and row["fields"]["sample_id"] > after_sample
            ]
            for row in reversed(native):
                if predicate(row):
                    return row
        time.sleep(0.05)
    raise AssertionError(f"{description} was not confirmed by AppKit on the main thread")


def png_stats(path):
    width, height, rows = rgb_pixels(path)
    data = path.read_bytes()
    return {
        "width": width,
        "height": height,
        "nonzero_rgb_bytes": sum(v != 0 for row in rows for v in row),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def capture_chain(log_path, after_capture_id=0, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if log_path.exists():
            rows = [json.loads(line) for line in log_path.read_text().splitlines()]
            prepared = [
                row for row in rows
                if row.get("event") == "screenshot_prepared"
                and row["capture_id"] > after_capture_id
            ]
            if prepared:
                start = prepared[0]
                chain = [
                    row for row in rows
                    if row.get("capture_id") == start["capture_id"]
                ]
                if any(row.get("event") == "image_returned" for row in chain):
                    acquire = [
                        row for row in rows
                        if row.get("event") == "surface_acquire"
                        and row["frame_id"] == start["frame_id"]
                    ]
                    return {"capture": chain, "surface_acquire": acquire}
        time.sleep(0.05)
    raise AssertionError("raw Bevy readback did not complete within the fixed deadline")


def experiment(work, fixture, helper):
    evidence = Path(tempfile.mkdtemp(prefix="capture-causality-live-", dir=ROOT / "target"))
    log_path = evidence / "causality.jsonl"
    native_log_path = evidence / "appkit-main.jsonl"
    raw_dir = evidence / "raw-readbacks"
    env = os.environ.copy()
    env.update(
        CARGO_NET_OFFLINE="true",
        WOODPECKER_CAPTURE_CAUSALITY_LOG=str(log_path),
        WOODPECKER_CAPTURE_CAUSALITY_NATIVE_LOG=str(native_log_path),
        WOODPECKER_CAPTURE_CAUSALITY_RAW_DIR=str(raw_dir),
        WOODPECKER_CAUSALITY_TITLE=f"Woodpecker causality {os.getpid()}",
    )
    server_log = (evidence / "server.log").open("w")
    server = subprocess.Popen(
        [str(CLI), "--address", "127.0.0.1:0", "server", "start",
         "--artifact-dir", str(evidence / "artifacts"), "--shutdown-seconds", "10"],
        cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=server_log, text=True,
    )
    deck = None
    active = None
    commands = (evidence / "commands.jsonl").open("w")
    try:
        assert select.select([server.stdout], [], [], 10)[0], "server did not start"
        address = json.loads(server.stdout.readline())["address"]

        def cli(*args):
            result = subprocess.run(
                [str(CLI), "--address", address, *args], cwd=ROOT, env=env,
                capture_output=True, text=True, timeout=45,
            )
            commands.write(json.dumps({"args": args, "exit": result.returncode,
                                       "stdout": result.stdout, "stderr": result.stderr}) + "\n")
            commands.flush()
            assert result.returncode == 0, (args, result.stdout, result.stderr)
            return json.loads(result.stdout)

        def until(check, description, timeout=20):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                value = check()
                if value:
                    return value
                time.sleep(0.025)
            raise AssertionError(description)

        created = cli("session", "create", "--config", str(fixture))
        active = created["id"]
        detail = until(
            lambda: (d if (d := cli("session", "inspect", active))["state"] == "Ready"
                     else None),
            "session did not become Ready",
        )
        artifact_dir = Path(detail["artifact_dir"])
        cursor = None

        def command(name, arguments):
            nonlocal cursor
            pending = cli("session", "submit", active, "--command",
                          json.dumps({"command": name, "arguments": arguments}))

            def outcome():
                nonlocal cursor
                args = ["session", "poll", active, "--wait-ms", "100"]
                if cursor is not None:
                    args += ["--cursor", json.dumps(cursor)]
                activity = cli(*args)
                cursor = activity["cursor"]
                return next(
                    (entry["event"] for entry in activity["entries"]
                     if entry["event"].get("request_id") == pending["request_id"]
                     and entry["event"]["kind"] != "pending"),
                    None,
                )
            return until(outcome, f"no outcome for {name}")

        def inspect_state():
            summary = command(
                "inspect.query",
                {
                    "source": "entities",
                    "entity": None,
                    "with": [],
                    "without": [],
                    "projection": {"kind": "summary"},
                },
            )
            camera = next(
                item["entity"] for item in summary["output"]["items"]
                if item["result"]["name"] == "camera"
            )
            detail = command(
                "inspect.query",
                {
                    "source": "entities",
                    "entity": camera,
                    "with": [],
                    "without": [],
                    "projection": {
                        "kind": "components",
                        "selection": {
                            "kind": "listed",
                            "type_paths": [
                                "blend_modes::SceneState",
                                "bevy_camera::camera::Camera",
                                "bevy_transform::components::transform::Transform",
                            ],
                        },
                    },
                },
            )
            return detail["output"]["items"][0]["result"]["components"]

        assert command("tick.warp.start", {"ticks": 1})["kind"] == "completed"
        frozen_state = inspect_state()
        deck = subprocess.Popen(
            [str(helper)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=(evidence / "deck.log").open("w"), text=True,
        )

        def deck_state(state):
            deck.stdin.write(state + "\n")
            deck.stdin.flush()
            assert select.select([deck.stdout], [], [], 3)[0], f"deck {state} timeout"
            assert json.loads(deck.stdout.readline())["deck_state"] == state

        last_capture_id = 0

        def capture(label):
            nonlocal last_capture_id
            relative = f"screenshots/{label}.png"
            event = command("screenshot.capture", {"path": relative})
            path = artifact_dir / relative
            chain = capture_chain(log_path, last_capture_id)
            last_capture_id = chain["capture"][0]["capture_id"]
            return {
                "label": label,
                "event": event,
                "png": png_stats(path) if path.exists() else None,
                "chain": chain,
                "state_after": inspect_state(),
            }

        deck_state("small")
        first_native = wait_for_native(
            native_log_path,
            lambda row: row["visible"] and not row["key"]
            and not row["miniaturized"] and row["occlusion_visible"],
            "visible, unfocused baseline",
        )
        baseline = capture("01-visible-unfocused")

        deck_state("cover")
        covered_native = wait_for_native(
            native_log_path,
            lambda row: row["visible"] and not row["miniaturized"]
            and not row["occlusion_visible"],
            "fully covered state",
            after_sample=first_native["sample_id"],
        )
        covered = capture("02-covered")

        deck_state("small")
        restored_native = wait_for_native(
            native_log_path,
            lambda row: row["visible"] and not row["key"]
            and not row["miniaturized"] and row["occlusion_visible"],
            "restored visible, unfocused state",
            after_sample=covered_native["sample_id"],
        )
        restored = capture("03-restored")

        assert baseline["event"]["kind"] == "completed", baseline
        assert covered["event"]["kind"] == "rejected", covered
        assert covered["event"]["error"]["code"] == "screenshot_window_unavailable", covered
        assert restored["event"]["kind"] == "completed", restored
        assert baseline["png"] == restored["png"], (baseline, restored)
        assert baseline["png"]["nonzero_rgb_bytes"] > 0, baseline
        assert not (artifact_dir / "screenshots/02-covered.png").exists()
        assert all(
            item["state_after"] == frozen_state
            for item in (baseline, covered, restored)
        ), "capture or visibility transition changed the frozen simulation state"

        def event(chain, name):
            return next(row for row in chain["capture"] if row["event"] == name)

        assert event(baseline["chain"], "queue_submitted")["copy_encoded"] is True
        assert event(baseline["chain"], "map_success")["copy_encoded"] is True
        assert event(baseline["chain"], "image_returned")["nonzero_rgb_bytes"] > 0
        assert event(covered["chain"], "copy_skipped")["reason"] == "swap_chain_view_missing"
        assert event(covered["chain"], "queue_submitted")["copy_encoded"] is False
        assert event(covered["chain"], "map_success")["copy_encoded"] is False
        assert event(covered["chain"], "image_returned")["nonzero_rgb_bytes"] == 0
        assert any(
            row["outcome"] == "occluded"
            for row in covered["chain"]["surface_acquire"]
        )
        assert event(restored["chain"], "queue_submitted")["copy_encoded"] is True
        assert event(restored["chain"], "map_success")["copy_encoded"] is True
        assert event(restored["chain"], "image_returned")["nonzero_rgb_bytes"] > 0
        ids = {
            (
                event(item["chain"], "image_returned")["capture_id"],
                event(item["chain"], "image_returned")["buffer_id"],
                event(item["chain"], "image_returned")["capture_texture_id"],
            )
            for item in (baseline, covered, restored)
        }
        assert len(ids) == 3, ids
        result = {
            "evidence": str(evidence),
            "ticks": 1,
            "state_stable": True,
            "native": [first_native, covered_native, restored_native],
            "captures": [baseline, covered, restored],
            "restored_png_equals_baseline": True,
        }
        (evidence / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result, indent=2))
        cli("session", "stop", active)
        active = None
        cli("server", "stop")
        server.wait(timeout=15)
        return evidence
    finally:
        if deck and deck.poll() is None:
            try:
                deck.stdin.write("quit\n")
                deck.stdin.flush()
                deck.wait(timeout=3)
            except Exception:
                deck.terminate()
        if active and server.poll() is None:
            try:
                cli("session", "stop", active)
            except Exception:
                pass
        if server.poll() is None:
            server.send_signal(signal.SIGINT)
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.terminate()
        commands.close()
        server.stdout.close()
        server_log.close()


def main():
    if sys.platform != "darwin":
        raise SystemExit("capture_causality_live.py requires macOS")
    parser = argparse.ArgumentParser()
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    work = ROOT / "target/capture-causality"
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    fixture, helper = prepare(work)
    print(f"Prepared: {work}", flush=True)
    if not args.prepare_only:
        experiment(work, fixture, helper)


if __name__ == "__main__":
    main()
