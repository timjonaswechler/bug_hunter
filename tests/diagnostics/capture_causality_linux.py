"""Prepare and run one bounded Bevy 0.19.1 Linux/X11 capture experiment.

Preparation performs source checks, builds, and headless tests only. The default
mode is the explicitly gated GUI run. All patched sources and build products
live below target/capture-causality-linux/.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import tomllib

ROOT = Path(__file__).resolve().parents[2]
TESTS = ROOT / "tests"
sys.path.insert(0, str(TESTS))
from mesh_picking import rgb_pixels  # noqa: E402
from diagnostics.capture_causality_linux_instrument import (  # noqa: E402
    instrument_adapter,
    instrument_bevy,
)
from diagnostics.capture_causality_linux_support import (  # noqa: E402
    chain_event,
    classify_capture_result,
    intersection_area,
    load_complete_chain,
    raw_stats,
    read_json_sources,
    rect,
    write_json,
)

WORK = ROOT / "target/capture-causality-linux"
TEMP_ROOT = WORK / "root"
DEPENDENCY = WORK / "deps/bevy_render"
CLI_TARGET = WORK / "cargo-target"
APP_TARGET = WORK / "app-cargo-target"
BEVY_VERSION = "0.19.1"
RUST_TOOLCHAIN = "1.95.0"
LOG_ENV = "WOODPECKER_CAPTURE_CAUSALITY_LINUX_LOG_DIR"
RAW_ENV = "WOODPECKER_CAPTURE_CAUSALITY_LINUX_RAW_DIR"


def run_checked(arguments, **kwargs):
    print("+", " ".join(map(str, arguments)), flush=True)
    subprocess.run(arguments, check=True, **kwargs)


def command_text(arguments):
    completed = subprocess.run(arguments, capture_output=True, text=True, check=False)
    return {
        "command": list(arguments),
        "exit": completed.returncode,
        "stdout": completed.stdout,
        "stderr": completed.stderr,
    }


def registry_crate(name, version):
    matches = list(Path.home().glob(f".cargo/registry/src/*/{name}-{version}"))
    if len(matches) != 1:
        raise RuntimeError(f"expected one cached {name} {version}, found {matches}")
    return matches[0]


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def protected_hashes():
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "bevy_test_apps/Cargo.toml",
             ROOT / "bevy_test_apps/Cargo.lock"]
    paths.extend(sorted((ROOT / "src/session/screenshot").glob("**/*")))
    return {str(path.relative_to(ROOT)): sha256(path) for path in paths if path.is_file()}


def locked_versions(path):
    with Path(path).open("rb") as file:
        lock = tomllib.load(file)
    wanted = {"bevy", "bevy_render", "wgpu", "wgpu-core", "wgpu-hal", "wgpu-types"}
    return sorted(
        {f"{item['name']} {item['version']}" for item in lock["package"] if item["name"] in wanted}
    )


def environment_snapshot():
    os_release = {}
    for line in Path("/etc/os-release").read_text().splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            os_release[key] = value.strip('"')
    selected_env = {
        key: os.environ.get(key, "")
        for key in (
            "XDG_SESSION_TYPE", "XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP",
            "DESKTOP_SESSION", "DISPLAY", "WAYLAND_DISPLAY",
        )
    }
    return {
        "os_release": os_release,
        "kernel": platform.uname()._asdict(),
        "session": selected_env,
        "desktop_versions": [
            command_text(["cinnamon", "--version"]),
            command_text(["muffin", "--version"]),
        ],
        "gpu_pci": command_text(["lspci", "-nnk"]),
        "glx": command_text(["glxinfo", "-B"]),
        "vulkan": command_text(["vulkaninfo", "--summary"]),
        "rustc": command_text(["rustc", f"+{RUST_TOOLCHAIN}", "--version"]),
        "cargo": command_text(["cargo", f"+{RUST_TOOLCHAIN}", "--version"]),
        "root_lock": locked_versions(ROOT / "Cargo.lock"),
        "test_app_lock": locked_versions(ROOT / "bevy_test_apps/Cargo.lock"),
    }


def replace_once(path, old, new):
    text = Path(path).read_text()
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one source block, found {count}")
    Path(path).write_text(text.replace(old, new))


def inject_linux_native(app):
    module = app / "src/bin/capture_causality_linux_native/mod.rs"
    module.parent.mkdir()
    shutil.copy2(TESTS / "diagnostics/capture_causality_linux_native.rs", module)
    shutil.copy2(
        TESTS / "diagnostics/capture_scene_visibility_jsonl.rs",
        module.parent / "jsonl.rs",
    )
    blend = app / "src/bin/blend_modes.rs"
    text = blend.read_text()
    marker = "use rand::{Rng, SeedableRng, rngs::StdRng};\n"
    text = text.replace(
        marker,
        marker + '#[cfg(target_os = "linux")]\nmod capture_causality_linux_native;\n',
        1,
    )
    text = text.replace(
        "fn main() {\n    let mut app = App::new();",
        "fn main() {\n    let mut app = App::new();\n"
        '    #[cfg(target_os = "linux")]\n'
        "    capture_causality_linux_native::install(&mut app);",
        1,
    )
    blend.write_text(text)


def prepare():
    before = protected_hashes()
    registry = registry_crate("bevy_render", BEVY_VERSION)
    registry_before = sha256(registry / "src/view/window/screenshot.rs")
    if TEMP_ROOT.exists():
        shutil.rmtree(TEMP_ROOT)
    if DEPENDENCY.exists():
        shutil.rmtree(DEPENDENCY)
    WORK.mkdir(parents=True, exist_ok=True)
    CLI_TARGET.mkdir(exist_ok=True)
    APP_TARGET.mkdir(exist_ok=True)
    shutil.copytree(
        ROOT,
        TEMP_ROOT,
        ignore=shutil.ignore_patterns(".git", "target", "node", "__pycache__", "*.pyc"),
    )
    (TEMP_ROOT / "target").symlink_to(CLI_TARGET, target_is_directory=True)
    DEPENDENCY.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(registry, DEPENDENCY)
    instrument_bevy(DEPENDENCY)
    instrument_adapter(TEMP_ROOT)

    app = TEMP_ROOT / "bevy_test_apps"
    inject_linux_native(app)
    manifest = app / "Cargo.toml"
    manifest.write_text(
        manifest.read_text()
        + f'''\n[target.'cfg(target_os = "linux")'.dependencies]\nraw-window-handle = "0.6"\n\n[patch.crates-io]\nbevy_render = {{ path = "{DEPENDENCY}" }}\n'''
    )

    run_checked([
        sys.executable, "-m", "py_compile",
        str(TESTS / "diagnostics/capture_causality_linux.py"),
        str(TESTS / "diagnostics/capture_causality_linux_deck.py"),
        str(TESTS / "diagnostics/capture_causality_linux_instrument.py"),
        str(TESTS / "diagnostics/capture_causality_linux_support.py"),
    ], cwd=ROOT)
    run_checked([
        sys.executable,
        str(TESTS / "diagnostics/capture_causality_linux_support_test.py"),
        "-v",
    ], cwd=TESTS / "diagnostics")
    run_checked([
        sys.executable,
        str(TESTS / "diagnostics/capture_scene_visibility_support_test.py"),
        "-v",
    ], cwd=TESTS / "diagnostics")
    run_checked([
        "cargo", f"+{RUST_TOOLCHAIN}", "build", "--offline", "--locked", "--features", "cli", "--bin", "woodpecker",
    ], cwd=TEMP_ROOT)
    app_env = os.environ.copy()
    app_env["CARGO_TARGET_DIR"] = str(APP_TARGET)
    run_checked([
        "cargo", f"+{RUST_TOOLCHAIN}", "build", "--offline", "--manifest-path", str(manifest),
        "--features", "slice", "--bin", "blend_modes",
    ], cwd=TEMP_ROOT, env=app_env)
    run_checked([
        "cargo", f"+{RUST_TOOLCHAIN}", "test", "--offline", "--manifest-path", str(manifest), "--lib",
    ], cwd=TEMP_ROOT, env=app_env)

    versions = {
        "root": locked_versions(ROOT / "Cargo.lock"),
        "bevy_test_apps": locked_versions(ROOT / "bevy_test_apps/Cargo.lock"),
        "registry_bevy_render": str(registry),
        "registry_screenshot_sha256": registry_before,
        "temporary_test_app_lock_sha256": sha256(manifest.with_name("Cargo.lock")),
    }
    environment = environment_snapshot()
    write_json(WORK / "versions.json", versions)
    write_json(WORK / "environment-preflight.json", environment)
    assert before == protected_hashes(), "preparation changed protected root files"
    assert registry_before == sha256(registry / "src/view/window/screenshot.rs"), (
        "preparation changed the Cargo registry source"
    )
    result = {
        "event": "build_ready",
        "work": str(WORK),
        "scene": "blend_modes",
        "capture_sequence": [
            "visible_unfocused", "partially_covered", "fully_covered", "restored_visible"
        ],
        "versions": versions,
        "environment": environment,
        "gui_command": f"{sys.executable} tests/diagnostics/capture_causality_linux.py",
    }
    write_json(WORK / "build-ready.json", result)
    print(json.dumps(result, indent=2), flush=True)
    return result


class JsonlWriter:
    def __init__(self, directory, source):
        directory = Path(directory)
        directory.mkdir(parents=True, exist_ok=True)
        self.source = source
        self.path = directory / f"{source}-{os.getpid()}.jsonl"
        self.file = self.path.open("a", buffering=1)

    def write(self, event, **fields):
        row = {
            "source": self.source,
            "pid": os.getpid(),
            "unix_time_ns": time.time_ns(),
            "event": event,
            **fields,
        }
        self.file.write(json.dumps(row, separators=(",", ":")) + "\n")
        self.file.flush()
        os.fsync(self.file.fileno())
        return row

    def close(self):
        self.file.close()


def wait_until(callback, description, timeout=25):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = callback()
        if value:
            return value
        time.sleep(0.025)
    raise AssertionError(description)


def native_rows(log_directory, session_id):
    return [
        row for row in read_json_sources(log_directory)
        if row.get("source") == "x11_main" and row.get("session_id") == session_id
    ]


def stable_native(log_directory, session_id, predicate, description, after_sample=0, timeout=12):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        rows = [
            row for row in native_rows(log_directory, session_id)
            if row["sample_id"] > after_sample
        ]
        tail = []
        for row in reversed(rows):
            if not predicate(row):
                break
            tail.append(row)
        tail.reverse()
        if len(tail) >= 8:
            first = next(
                (
                    row for row in tail
                    if tail[-1]["unix_time_ns"] - row["unix_time_ns"] >= 100_000_000
                ),
                None,
            )
            if first is not None:
                window = tail[tail.index(first):]
                stable = {
                    (
                        row["x11_window"], row["bevy_visible"], row["map_state"], row["focused"],
                        row["visibility_state"], rect(row), row["bevy_physical_width"],
                        row["bevy_physical_height"], row["bevy_scale_factor"],
                        json.dumps(row["cameras"], sort_keys=True),
                    )
                    for row in window
                }
                if len(stable) == 1:
                    return {
                        "description": description,
                        "sample_count": len(window),
                        "quiet_span_ns": window[-1]["unix_time_ns"] - window[0]["unix_time_ns"],
                        "first": window[0],
                        "last": window[-1],
                    }
        time.sleep(0.02)
    raise AssertionError(f"native X11 state not confirmed: {description}")


def png_stats(path):
    width, height, rows = rgb_pixels(path)
    pixels = [row[index:index + 3] for row in rows for index in range(0, len(row), 3)]
    return {
        "width": width,
        "height": height,
        "sha256": sha256(path),
        "unique_rgb": len({bytes(pixel) for pixel in pixels}),
        "bright_pixels": sum(all(value >= 200 for value in pixel) for pixel in pixels),
        "dark_pixels": sum(all(value <= 25 for value in pixel) for pixel in pixels),
        "red_dominant_pixels": sum(
            pixel[0] >= pixel[1] + 50 and pixel[0] >= pixel[2] + 50 for pixel in pixels
        ),
    }


def chain_for_request(log_directory, request_id, session_id, timeout=12):
    return wait_until(
        lambda: load_complete_chain(log_directory, request_id, session_id),
        f"no complete capture chain for request {request_id} in {session_id}",
        timeout=timeout,
    )


def raw_for_chain(raw_directory, chain):
    image = chain_event(chain, "image_returned")
    pattern = f"capture-{image['capture_id']}-buffer-{image['buffer_id']}-*.raw"
    matches = list(Path(raw_directory).glob(pattern))
    if len(matches) != 1:
        raise AssertionError((pattern, matches))
    stats = raw_stats(matches[0])
    assert stats["byte_length"] == image["byte_length"], (stats, image)
    assert stats["nonzero_all_bytes"] == image["nonzero_all_bytes"], (stats, image)
    stats["path"] = str(matches[0])
    return stats


def process_running(pid):
    if not pid:
        return False
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def experiment():
    if sys.platform != "linux":
        raise SystemExit("capture_causality_linux.py requires Linux")
    if os.environ.get("XDG_SESSION_TYPE") != "x11" or not os.environ.get("DISPLAY"):
        raise SystemExit("this prepared probe requires the current X11 session; no backend switch is automatic")
    cli_binary = TEMP_ROOT / "target/debug/woodpecker"
    app_binary = APP_TARGET / "debug/blend_modes"
    if not cli_binary.exists() or not app_binary.exists():
        raise SystemExit("prepared binaries are missing; run with --prepare-only first")

    evidence = Path(tempfile.mkdtemp(prefix="capture-causality-linux-run-", dir=ROOT / "target"))
    log_directory = evidence / "events"
    raw_directory = evidence / "raw-readbacks"
    writer = JsonlWriter(log_directory, "controller")
    env = os.environ.copy()
    env.update(
        CARGO_NET_OFFLINE="true",
        CARGO_TARGET_DIR=str(APP_TARGET),
        RUSTUP_TOOLCHAIN=RUST_TOOLCHAIN,
        **{LOG_ENV: str(log_directory), RAW_ENV: str(raw_directory)},
    )
    server_log_handle = (evidence / "server.stderr.log").open("w")
    server = subprocess.Popen(
        [str(cli_binary), "--address", "127.0.0.1:0", "server", "start",
         "--artifact-dir", str(evidence / "artifacts"), "--shutdown-seconds", "10"],
        cwd=TEMP_ROOT,
        env=env,
        stdout=subprocess.PIPE,
        stderr=server_log_handle,
        text=True,
    )
    deck = None
    deck_stderr = None
    session = None
    app_pid = None
    result = {
        "status": "running",
        "classification": None,
        "evidence": str(evidence),
        "scene": "blend_modes",
        "versions": json.loads((WORK / "versions.json").read_text()),
        "environment_preflight": str(WORK / "environment-preflight.json"),
        "setup_ticks": 1,
        "captures": [],
    }
    cursor = None
    submitted_commands = []

    try:
        if not select.select([server.stdout], [], [], 15)[0]:
            raise AssertionError("server did not start")
        address = json.loads(server.stdout.readline())["address"]
        writer.write("server_started", server_pid=server.pid, address=address)

        def cli(*arguments):
            response = subprocess.run(
                [str(cli_binary), "--address", address, *arguments],
                cwd=TEMP_ROOT,
                env=env,
                capture_output=True,
                text=True,
                timeout=45,
            )
            writer.write(
                "cli",
                arguments=list(arguments),
                exit=response.returncode,
                stdout=response.stdout,
                stderr=response.stderr,
            )
            if response.returncode != 0:
                raise AssertionError((arguments, response.stdout, response.stderr))
            return json.loads(response.stdout) if response.stdout.strip() else None

        created = cli("session", "create", "--config", str(TEMP_ROOT / "tests/fixtures/blend_modes.toml"))
        session = created["id"]
        detail = wait_until(
            lambda: (value if (value := cli("session", "inspect", session))["state"] == "Ready" else None),
            "blend_modes did not become Ready",
        )
        artifact_dir = Path(detail["artifact_dir"])

        def command(name, arguments):
            nonlocal cursor
            submitted_commands.append({"command": name, "arguments": arguments})
            pending = cli(
                "session", "submit", session, "--command",
                json.dumps({"command": name, "arguments": arguments}),
            )
            request_id = pending["request_id"]

            def outcome():
                nonlocal cursor
                args = ["session", "poll", session, "--wait-ms", "100"]
                if cursor is not None:
                    args += ["--cursor", json.dumps(cursor)]
                activity = cli(*args)
                cursor = activity["cursor"]
                return next(
                    (
                        entry["event"] for entry in activity["entries"]
                        if entry["event"].get("request_id") == request_id
                        and entry["event"]["kind"] != "pending"
                    ),
                    None,
                )

            return request_id, wait_until(outcome, f"no outcome for {name}")

        def inspect_frozen_state():
            _, summary = command("inspect.query", {
                "source": "entities", "entity": None, "with": [], "without": [],
                "projection": {"kind": "summary"},
            })
            camera = next(
                item["entity"] for item in summary["output"]["items"]
                if item["result"]["name"] == "camera"
            )
            _, detail_event = command("inspect.query", {
                "source": "entities", "entity": camera, "with": [], "without": [],
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
            })
            return detail_event["output"]["items"][0]["result"]["components"]

        _, warped = command("tick.warp.start", {"ticks": 1})
        assert warped["kind"] == "completed", warped
        frozen = inspect_frozen_state()
        initial_native = stable_native(
            log_directory,
            session,
            lambda row: row["bevy_visible"] and row["map_state"] == "viewable",
            "owned target mapped before deck startup",
        )
        target_xid = initial_native["last"]["x11_window"]
        app_pid = initial_native["last"]["pid"]
        target_rect = rect(initial_native["last"])

        deck_stderr = (evidence / "deck.stderr.log").open("w")
        deck = subprocess.Popen(
            [
                sys.executable,
                str(TESTS / "diagnostics/capture_causality_linux_deck.py"),
                "--target-xid", str(target_xid),
                "--log-dir", str(log_directory),
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=deck_stderr,
            text=True,
            bufsize=1,
            env=env,
        )
        if not select.select([deck.stdout], [], [], 5)[0]:
            raise AssertionError("X11 deck did not start")
        ready = json.loads(deck.stdout.readline())
        assert ready["event"] == "deck_ready" and ready["target_xid"] == target_xid, ready

        last_sample = initial_native["last"]["sample_id"]

        def place(ordinal, state, expected_visibility):
            nonlocal last_sample, target_rect
            placement_id = f"capture-{ordinal}-{state}"
            (artifact_dir / "linux-capture-placement-id").write_text(placement_id + "\n")
            args = " ".join(str(int(value)) for value in target_rect)
            deck.stdin.write(f"{state} {placement_id} {args}\n")
            deck.stdin.flush()
            if not select.select([deck.stdout], [], [], 5)[0]:
                raise AssertionError(f"X11 deck command timed out: {state}")
            deck_row = json.loads(deck.stdout.readline())
            assert (
                deck_row["event"] == "deck_state"
                and deck_row["state"] == state
                and deck_row["placement_id"] == placement_id
            ), deck_row
            target_area = deck_row["target_area"]
            overlap = deck_row["intersection_area"]
            if state == "small":
                assert overlap == 0, deck_row
            elif state == "partial":
                assert 0 < overlap < target_area, deck_row
                assert overlap * 2 == target_area, deck_row
            else:
                assert overlap == target_area, deck_row
            target_rect = tuple(deck_row["target_rect"])

            def predicate(row):
                base = (
                    row["x11_window"] == target_xid
                    and row.get("placement_id") == placement_id
                    and row["bevy_visible"]
                    and row["map_state"] == "viewable"
                    and not row["focused"]
                    and rect(row) == tuple(float(value) for value in target_rect)
                )
                if not base:
                    return False
                if expected_visibility is None:
                    return row["visibility_state"] in (None, "unobscured")
                return row["visibility_state"] == expected_visibility

            native = stable_native(
                log_directory,
                session,
                predicate,
                f"{state} with {expected_visibility or 'baseline geometry'}",
                after_sample=last_sample,
            )
            last_sample = native["last"]["sample_id"]
            return {
                "state": state,
                "placement_id": placement_id,
                "deck": deck_row,
                "native": native,
                "intersection_area": overlap,
                "target_area": target_area,
            }

        def capture(ordinal, label, placement):
            before = inspect_frozen_state()
            assert before == frozen, f"simulation changed before {label}"
            relative = f"screenshots/{ordinal:02}-{label}.png"
            request_id, event = command("screenshot.capture", {"path": relative})
            chain = chain_for_request(log_directory, request_id, session)
            placement_id = placement["placement_id"]
            assert all(row.get("placement_id") == placement_id for row in chain["bevy"]), chain
            assert all(
                row.get("placement_id") == placement_id for row in chain["surface_acquire"]
            ), chain
            assert all(row.get("placement_id") == placement_id for row in chain["adapter"]), chain
            after = inspect_frozen_state()
            assert after == frozen, f"simulation changed during {label}"
            path = artifact_dir / relative
            item = {
                "ordinal": ordinal,
                "label": label,
                "request_id": request_id,
                "event": event,
                "placement": placement,
                "chain": chain,
                "raw": raw_for_chain(raw_directory, chain),
                "png": png_stats(path) if path.exists() else None,
                "png_path": str(path) if path.exists() else None,
                "state_unchanged": True,
            }
            result["captures"].append(item)
            write_json(evidence / "result.json", result)
            return item

        sequence = [
            ("small", None, "visible-unfocused"),
            ("partial", "partially_obscured", "partially-covered"),
            ("cover", "fully_obscured", "fully-covered"),
            ("small", "unobscured", "restored-visible"),
        ]
        for ordinal, (state, visibility, label) in enumerate(sequence, 1):
            placement = place(ordinal, state, visibility)
            item = capture(ordinal, label, placement)
            if ordinal == 1:
                stats = item["png"]
                assert item["event"]["kind"] == "completed", item
                assert stats is not None and (stats["width"], stats["height"]) == (1280, 720), stats
                assert stats["unique_rgb"] >= 16, stats
                assert stats["bright_pixels"] > 0 and stats["dark_pixels"] > 0, stats
                assert stats["red_dominant_pixels"] > 0, stats

        tick_commands = [
            item for item in submitted_commands if item["command"].startswith("tick.")
        ]
        assert tick_commands == [
            {"command": "tick.warp.start", "arguments": {"ticks": 1}}
        ], tick_commands
        adapter_rows = [
            row for row in read_json_sources(log_directory)
            if row.get("source") == "bevy_render"
            and row.get("session_id") == session
            and row.get("event") == "adapter_selected"
        ]
        assert len(adapter_rows) == 1, adapter_rows
        classification = classify_capture_result(result["captures"])
        result.update(
            status="completed",
            classification=classification,
            adapter=adapter_rows[0],
            frozen_state=frozen,
            target_xid=target_xid,
            app_pid=app_pid,
            capture_count=4,
            tick_count=1,
        )

        cli("session", "stop", session)
        session = None
        cli("server", "stop")
        assert server.wait(timeout=15) == 0
        return result
    except BaseException as error:
        if result["classification"] is None:
            result["classification"] = "inconclusive_state_control_or_infrastructure"
        result.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        cleanup = {}
        if deck is not None:
            if deck.poll() is None:
                try:
                    deck.stdin.write("quit\n")
                    deck.stdin.flush()
                    if select.select([deck.stdout], [], [], 3)[0]:
                        writer.write("deck_quit_ack", row=json.loads(deck.stdout.readline()))
                    deck.wait(timeout=5)
                except Exception as error:
                    writer.write("deck_cleanup_error", error=f"{type(error).__name__}: {error}")
                    deck.kill()
                    deck.wait(timeout=5)
            cleanup.update(
                deck_pid=deck.pid,
                deck_returncode=deck.returncode,
                deck_running=process_running(deck.pid),
            )
            if deck.stdin:
                deck.stdin.close()
            if deck.stdout:
                deck.stdout.close()
        if session is not None and server.poll() is None:
            try:
                cli("session", "stop", session)
                session = None
            except Exception as error:
                writer.write("session_cleanup_error", error=f"{type(error).__name__}: {error}")
        if server.poll() is None:
            server.send_signal(signal.SIGINT)
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=5)
        time.sleep(0.1)
        cleanup.update(
            server_pid=server.pid,
            server_returncode=server.returncode,
            server_running=process_running(server.pid),
            app_pid=app_pid,
            app_running=process_running(app_pid),
        )
        result["cleanup"] = cleanup
        server_log_handle.flush()
        server_log = (evidence / "server.stderr.log").read_text()
        result["gpu_error_markers"] = [
            marker for marker in ("DeviceLost", "Validation Error", "validation error")
            if marker in server_log
        ]
        if any(cleanup.get(key) for key in ("deck_running", "server_running", "app_running")):
            result["status"] = "failed"
            result["classification"] = "inconclusive_state_control_or_infrastructure"
            result["cleanup_error"] = "one or more owned test processes are still running"
        write_json(evidence / "cleanup.json", cleanup)
        write_json(evidence / "result.json", result)
        writer.close()
        server.stdout.close()
        server_log_handle.close()
        if deck_stderr is not None:
            deck_stderr.close()
        print(json.dumps({
            "event": "gui_measurement_finished",
            "evidence": str(evidence),
            "status": result["status"],
            "classification": result["classification"],
            "cleanup": cleanup,
        }, indent=2), flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    if args.prepare_only:
        prepare()
    else:
        experiment()


if __name__ == "__main__":
    main()
