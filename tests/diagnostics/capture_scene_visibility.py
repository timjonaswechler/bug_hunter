"""Prepare and run the Bevy 0.19.1 scene-visibility diagnosis.

Preparation only compiles code and runs headless checks. GUI modes are explicit.
All source instrumentation and manifest edits happen in a copy below target/.
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
from diagnostics.capture_scene_visibility_instrument import (  # noqa: E402
    instrument_adapter,
    instrument_bevy,
)
from diagnostics.capture_scene_visibility_support import (  # noqa: E402
    VisibilityController,
    correlate_capture_rows,
    read_json_sources,
)

WORK = ROOT / "target/capture-scene-visibility"
TEMP_ROOT = WORK / "root"
BEVY_VERSION = "0.19.1"
SCENES = ("blend_modes", "mesh_picking", "ui")
BINARIES = {"blend_modes": "blend_modes", "mesh_picking": "mesh_picking", "ui": "context_menu"}
TEST_FILES = {"blend_modes": "blend_modes.py", "mesh_picking": "mesh_picking.py", "ui": "ui.py"}
FIXTURES = {"blend_modes": "blend_modes.toml", "mesh_picking": "mesh_picking.toml", "ui": "context_menu.toml"}


def run_checked(arguments, **kwargs):
    print("+", " ".join(map(str, arguments)), flush=True)
    subprocess.run(arguments, check=True, **kwargs)


def registry_crate(name, version):
    matches = list(Path.home().glob(f".cargo/registry/src/*/{name}-{version}"))
    if len(matches) != 1:
        raise RuntimeError(f"expected one cached {name} {version}, found {matches}")
    return matches[0]


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def protected_hashes():
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    paths.extend(sorted((ROOT / "src/session/screenshot").glob("**/*")))
    return {str(path.relative_to(ROOT)): sha256(path) for path in paths if path.is_file()}


def replace_once(path, old, new):
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one source block, found {count}")
    path.write_text(text.replace(old, new))


def inject_native(app):
    module = app / "src/bin/capture_scene_visibility_native/mod.rs"
    module.parent.mkdir()
    shutil.copy2(TESTS / "diagnostics/capture_scene_visibility_native.rs", module)
    shutil.copy2(
        TESTS / "diagnostics/capture_scene_visibility_jsonl.rs",
        module.parent / "jsonl.rs",
    )
    for name in ("blend_modes", "mesh_picking", "context_menu"):
        path = app / f"src/bin/{name}.rs"
        text = path.read_text()
        module_declaration = "#[cfg(target_os = \"macos\")]\nmod capture_scene_visibility_native;\n\n"
        if text.startswith("//! "):
            split = text.index("\n\n") + 2
            text = text[:split] + module_declaration + text[split:]
        else:
            text = module_declaration + text
        text = text.replace(
            "    let mut app = App::new();",
            "    let mut app = App::new();\n"
            "    #[cfg(target_os = \"macos\")]\n"
            "    capture_scene_visibility_native::install(&mut app);",
            1,
        )
        path.write_text(text)


def patch_full_test(path, scene):
    text = path.read_text()
    if "import os\n" not in text:
        marker = "import json\n"
        text = text.replace(marker, marker + "import os\n", 1)
    import_marker = "from diagnostics.capture_scene_visibility_support import VisibilityController\n"
    if import_marker not in text:
        first_local = "from slice import"
        index = text.index(first_local)
        text = text[:index] + import_marker + text[index:]
    body_indent = "                " if scene == "ui" else "            "
    address_line = body_indent + 'address = json.loads(server.stdout.readline())["address"]\n'
    controller = (
        address_line
        + body_indent + "visibility = VisibilityController(\n"
        + body_indent + "    os.environ[\"WOODPECKER_CAPTURE_SCENE_VISIBILITY_DECK\"],\n"
        + body_indent + "    os.environ[\"WOODPECKER_CAPTURE_SCENE_VISIBILITY_NATIVE_LOG_DIR\"],\n"
        + body_indent + "    os.environ[\"WOODPECKER_CAPTURE_SCENE_VISIBILITY_FULL_EVIDENCE\"],\n"
        + body_indent + ")\n"
        + body_indent + "visibility_condition = os.environ[\"WOODPECKER_CAPTURE_SCENE_VISIBILITY_CONDITION\"]\n"
    )
    if address_line not in text:
        raise RuntimeError(f"address seam missing in {path}")
    text = text.replace(address_line, controller, 1)

    if scene == "ui":
        seam = '                    before = state(target)\n                    result = command("screenshot.capture", {"path": path}, target=target)\n'
        replacement = (
            '                    before = state(target)\n'
            '                    root = Path(cli("session", "inspect", target)["artifact_dir"])\n'
            '                    visibility.place(target, root, visibility_condition)\n'
            '                    result = command("screenshot.capture", {"path": path}, target=target)\n'
        )
        if seam not in text:
            raise RuntimeError(f"capture seam missing in {path}")
        text = text.replace(seam, replacement, 1)
        replay = '                assert command("replay.start", {"path": recording_path}) == {\n'
        text = text.replace(
            replay,
            '                visibility.place(session, artifact_root, visibility_condition)\n' + replay,
            1,
        )
    else:
        variable = "before" if scene == "blend_modes" else "frozen"
        seam = f'                {variable} = snapshot()\n                path = f"screenshots/{{name}}.png"\n'
        replacement = (
            f'                {variable} = snapshot()\n'
            '                visibility.place(session, artifact_dir, visibility_condition)\n'
            '                path = f"screenshots/{name}.png"\n'
        )
        if seam not in text:
            raise RuntimeError(f"capture seam missing in {path}")
        text = text.replace(seam, replacement, 1)

    indent = "            " if scene == "ui" else "        "
    finally_seam = f"{indent}finally:\n{indent}    if server.poll() is None:\n"
    finally_replacement = (
        f"{indent}finally:\n"
        f"{indent}    if 'visibility' in locals():\n"
        f"{indent}        visibility.close()\n"
        f"{indent}    if server.poll() is None:\n"
    )
    if finally_seam not in text:
        raise RuntimeError(f"cleanup seam missing in {path}")
    path.write_text(text.replace(finally_seam, finally_replacement, 1))


def prepare(scenes=SCENES):
    scenes = tuple(dict.fromkeys(scenes))
    unknown = set(scenes) - set(SCENES)
    if unknown:
        raise ValueError(f"unknown scenes: {sorted(unknown)}")
    before = protected_hashes()
    if TEMP_ROOT.exists():
        shutil.rmtree(TEMP_ROOT)
    WORK.mkdir(parents=True, exist_ok=True)
    shutil.copytree(
        ROOT,
        TEMP_ROOT,
        ignore=shutil.ignore_patterns(".git", "target", "__pycache__", "*.pyc"),
    )
    (WORK / "cargo-target").mkdir(exist_ok=True)
    (WORK / "app-cargo-target").mkdir(exist_ok=True)
    (TEMP_ROOT / "target").symlink_to(WORK / "cargo-target", target_is_directory=True)
    dependency = WORK / "deps/bevy_render"
    if dependency.exists():
        shutil.rmtree(dependency)
    dependency.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(registry_crate("bevy_render", BEVY_VERSION), dependency)
    instrument_bevy(dependency)
    instrument_adapter(TEMP_ROOT)

    app = TEMP_ROOT / "bevy_test_apps"
    inject_native(app)
    manifest = app / "Cargo.toml"
    manifest.write_text(
        manifest.read_text()
        + f'''\n[target.'cfg(target_os = "macos")'.dependencies]\nobjc2 = "0.6.4"\nobjc2-app-kit = {{ version = "0.3.2", default-features = false, features = ["std", "NSResponder", "NSView", "NSWindow"] }}\nraw-window-handle = "0.6"\n\n[patch.crates-io]\nbevy_render = {{ path = "{dependency}" }}\n'''
    )
    for scene, filename in TEST_FILES.items():
        patch_full_test(TEMP_ROOT / "tests" / filename, scene)

    helper = WORK / "capture-scene-visibility-deck"
    run_checked([
        "xcrun", "swiftc", str(TESTS / "diagnostics/capture_scene_visibility_deck.swift"),
        "-o", str(helper),
    ], cwd=ROOT)
    run_checked([
        "cargo", "build", "--offline", "--locked", "--features", "cli", "--bin", "woodpecker",
    ], cwd=TEMP_ROOT)
    app_env = os.environ.copy()
    app_env["CARGO_TARGET_DIR"] = str(WORK / "app-cargo-target")
    for scene in scenes:
        run_checked([
            "cargo", "build", "--offline", "--manifest-path", str(manifest),
            "--features", "slice", "--bin", BINARIES[scene],
        ], cwd=TEMP_ROOT, env=app_env)

    versions = {
        "root_bevy": locked_versions(ROOT / "Cargo.lock"),
        "test_app_bevy": locked_versions(ROOT / "bevy_test_apps/Cargo.lock"),
        "platform": subprocess.run(["sw_vers"], capture_output=True, text=True, check=True).stdout.strip(),
        "rustc": subprocess.run(["rustc", "--version"], capture_output=True, text=True, check=True).stdout.strip(),
        "cargo": subprocess.run(["cargo", "--version"], capture_output=True, text=True, check=True).stdout.strip(),
    }
    (WORK / "versions.json").write_text(json.dumps(versions, indent=2) + "\n")
    assert before == protected_hashes(), "preparation changed protected root files"
    return helper, versions


def locked_versions(lock):
    import tomllib
    data = tomllib.loads(lock.read_text())
    wanted = {"bevy", "bevy_render", "wgpu", "wgpu-core", "wgpu-hal"}
    return sorted(
        {f"{item['name']} {item['version']}" for item in data["package"] if item["name"] in wanted}
    )


def wait_until(callback, description, timeout=25):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = callback()
        if value:
            return value
        time.sleep(0.025)
    raise AssertionError(description)


def png_stats(path):
    width, height, rows = rgb_pixels(path)
    raw = b"".join(rows)
    return {
        "width": width,
        "height": height,
        "nonzero_rgb_bytes": sum(value != 0 for value in raw),
        "unique_rgb": len({bytes(row[index:index + 3]) for row in rows for index in range(0, len(row), 3)}),
        "sha256": sha256(path),
    }


def chain_for_request(log_directory, request_id, session_id, timeout=12):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        chain = correlate_capture_rows(
            read_json_sources(log_directory),
            request_id,
            session_id,
        )
        if chain is not None:
            return chain
        time.sleep(0.02)
    raise AssertionError(f"no complete capture chain for request {request_id} in {session_id}")


def chain_event(chain, name):
    rows = [row for row in chain["bevy"] if row.get("event") == name]
    assert len(rows) == 1, (name, rows)
    return rows[0]


def diagnostic_scene(scene, helper, versions):
    evidence = Path(tempfile.mkdtemp(prefix=f"capture-scene-visibility-{scene}-", dir=ROOT / "target"))
    log_directory = evidence / "capture-chain"
    native_log = evidence / "appkit-main"
    raw_dir = evidence / "raw-readbacks"
    env = os.environ.copy()
    env.update(
        CARGO_NET_OFFLINE="true",
        CARGO_TARGET_DIR=str(WORK / "app-cargo-target"),
        WOODPECKER_CAPTURE_SCENE_VISIBILITY_LOG_DIR=str(log_directory),
        WOODPECKER_CAPTURE_SCENE_VISIBILITY_NATIVE_LOG_DIR=str(native_log),
        WOODPECKER_CAPTURE_SCENE_VISIBILITY_RAW_DIR=str(raw_dir),
    )
    cli_binary = TEMP_ROOT / "target/debug/woodpecker"
    server_log = (evidence / "server.log").open("w")
    commands_file = (evidence / "commands.jsonl").open("w")
    server = subprocess.Popen(
        [str(cli_binary), "--address", "127.0.0.1:0", "server", "start",
         "--artifact-dir", str(evidence / "artifacts"), "--shutdown-seconds", "10"],
        cwd=TEMP_ROOT, env=env, stdout=subprocess.PIPE, stderr=server_log, text=True,
    )
    controller = None
    session = None
    result = {"status": "running", "scene": scene, "evidence": str(evidence), "versions": versions}
    try:
        assert select.select([server.stdout], [], [], 15)[0], "server did not start"
        address = json.loads(server.stdout.readline())["address"]

        def cli(*args):
            response = subprocess.run(
                [str(cli_binary), "--address", address, *args], cwd=TEMP_ROOT, env=env,
                capture_output=True, text=True, timeout=45,
            )
            commands_file.write(json.dumps({"arguments": args, "exit": response.returncode,
                                            "stdout": response.stdout, "stderr": response.stderr}) + "\n")
            commands_file.flush()
            assert response.returncode == 0, (args, response.stdout, response.stderr)
            return json.loads(response.stdout) if response.stdout.strip() else None

        created = cli("session", "create", "--config", str(TEMP_ROOT / "tests/fixtures" / FIXTURES[scene]))
        session = created["id"]
        detail = wait_until(
            lambda: (value if (value := cli("session", "inspect", session))["state"] == "Ready" else None),
            f"{scene} did not become Ready",
        )
        artifact_dir = Path(detail["artifact_dir"])
        cursor = None

        def command(name, arguments):
            nonlocal cursor
            pending = cli("session", "submit", session, "--command", json.dumps({"command": name, "arguments": arguments}))
            request_id = pending["request_id"]

            def outcome():
                nonlocal cursor
                args = ["session", "poll", session, "--wait-ms", "100"]
                if cursor is not None:
                    args += ["--cursor", json.dumps(cursor)]
                activity = cli(*args)
                cursor = activity["cursor"]
                return next((entry["event"] for entry in activity["entries"]
                             if entry["event"].get("request_id") == request_id
                             and entry["event"]["kind"] != "pending"), None)
            return request_id, wait_until(outcome, f"no outcome for {name}")

        def inspect_components(names):
            _, summary = command("inspect.query", {
                "source": "entities", "entity": None, "with": [], "without": [],
                "projection": {"kind": "summary"},
            })
            by_name = {item["result"]["name"]: item["entity"] for item in summary["output"]["items"]}
            answer = {}
            for name, paths in names.items():
                entity = by_name[name]
                _, detail_event = command("inspect.query", {
                    "source": "entities", "entity": entity, "with": [], "without": [],
                    "projection": {"kind": "components", "selection": {"kind": "listed", "type_paths": paths}},
                })
                answer[name] = detail_event["output"]["items"][0]["result"]["components"]
            return answer

        # Existing fixtures need only their existing explicit startup/layout ticks.
        setup_ticks = {"blend_modes": 1, "mesh_picking": 11, "ui": 3}[scene]
        _, warped = command("tick.warp.start", {"ticks": setup_ticks})
        assert warped["kind"] == "completed", warped
        component_sets = {
            "blend_modes": {"camera": ["blend_modes::SceneState", "bevy_transform::components::transform::Transform"]},
            "mesh_picking": {
                "center-cube": ["mesh_picking::MeshInteractionState", "bevy_transform::components::transform::Transform"],
                "left-sphere": ["mesh_picking::MeshInteractionState", "bevy_transform::components::transform::Transform"],
                "right-cylinder": ["mesh_picking::MeshInteractionState", "bevy_transform::components::transform::Transform"],
            },
            "ui": {"background": ["context_menu::SessionState"]},
        }[scene]
        frozen = inspect_components(component_sets)
        controller = VisibilityController(helper, native_log, evidence)
        captures = []
        for ordinal, state in enumerate(("visible", "partial", "covered", "visible"), 1):
            placement = controller.place(session, artifact_dir, state)
            label = ("visible-unfocused", "partially-covered", "fully-covered", "restored")[ordinal - 1]
            relative = f"screenshots/{ordinal:02}-{label}.png"
            before = inspect_components(component_sets)
            assert before == frozen, f"{scene} state changed before {label}"
            request_id, event = command("screenshot.capture", {"path": relative})
            chain = chain_for_request(log_directory, request_id, session)
            after = inspect_components(component_sets)
            assert after == frozen, f"{scene} state changed during {label} capture"
            path = artifact_dir / relative
            captures.append({
                "ordinal": ordinal,
                "label": label,
                "placement": placement,
                "request_id": request_id,
                "event": event,
                "path": str(path),
                "png": png_stats(path) if path.exists() else None,
                "file_exists": path.exists(),
                "chain": chain,
                "state_unchanged": True,
            })

        baseline, partial, covered, restored = captures
        for item in (baseline, partial, restored):
            assert item["event"]["kind"] == "completed", item
            assert chain_event(item["chain"], "queue_submitted")["copy_encoded"] is True
            assert chain_event(item["chain"], "map_success")["copy_encoded"] is True
            assert chain_event(item["chain"], "image_returned")["nonzero_rgb_bytes"] > 0
            assert item["png"]["unique_rgb"] >= 16, item["png"]
            assert any(row["outcome"] in ("success", "suboptimal") for row in item["chain"]["surface_acquire"])
        assert baseline["png"] == partial["png"] == restored["png"]
        assert covered["event"]["kind"] == "rejected", covered
        assert covered["event"]["error"]["code"] == "screenshot_window_unavailable", covered
        assert not covered["file_exists"]
        assert chain_event(covered["chain"], "copy_skipped")["reason"] == "swap_chain_view_missing"
        assert chain_event(covered["chain"], "queue_submitted")["copy_encoded"] is False
        assert chain_event(covered["chain"], "map_success")["copy_encoded"] is False
        assert chain_event(covered["chain"], "image_returned")["nonzero_rgb_bytes"] == 0
        assert any(row["outcome"] == "occluded" for row in covered["chain"]["surface_acquire"])
        ids = {
            (chain_event(item["chain"], "image_returned")["capture_id"],
             chain_event(item["chain"], "image_returned")["buffer_id"],
             chain_event(item["chain"], "image_returned")["capture_texture_id"])
            for item in captures
        }
        assert len(ids) == 4
        result.update(
            status="passed",
            setup_ticks=setup_ticks,
            capture_count=4,
            captures=captures,
            state_unchanged=True,
            partial_equals_baseline=True,
            restored_equals_baseline=True,
            covered_guard_rejection=True,
            diagnostic_scope="guard confirmation, not a complete scene acceptance",
        )
        cli("session", "stop", session)
        session = None
        cli("server", "stop")
        assert server.wait(timeout=15) == 0
        return evidence, result
    except BaseException as error:
        result.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        cleanup = {}
        if controller is not None:
            cleanup.update(controller.close())
        if session is not None and server.poll() is None:
            try:
                cli("session", "stop", session)
            except Exception:
                pass
        if server.poll() is None:
            server.send_signal(signal.SIGINT)
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=5)
        cleanup.update(server_pid=server.pid, server_returncode=server.returncode,
                       server_running=server.poll() is None)
        result["cleanup"] = cleanup
        (evidence / "cleanup.json").write_text(json.dumps(cleanup, indent=2) + "\n")
        (evidence / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        commands_file.close()
        server.stdout.close()
        server_log.close()


def run_full_test(scene, condition, helper, versions):
    evidence = Path(tempfile.mkdtemp(prefix=f"capture-scene-full-{scene}-{condition}-", dir=ROOT / "target"))
    env = os.environ.copy()
    env.update(
        CARGO_NET_OFFLINE="true",
        CARGO_TARGET_DIR=str(WORK / "app-cargo-target"),
        WOODPECKER_CAPTURE_SCENE_VISIBILITY_LOG_DIR=str(evidence / "capture-chain"),
        WOODPECKER_CAPTURE_SCENE_VISIBILITY_NATIVE_LOG_DIR=str(evidence / "appkit-main"),
        WOODPECKER_CAPTURE_SCENE_VISIBILITY_RAW_DIR=str(evidence / "raw-readbacks"),
        WOODPECKER_CAPTURE_SCENE_VISIBILITY_DECK=str(helper),
        WOODPECKER_CAPTURE_SCENE_VISIBILITY_FULL_EVIDENCE=str(evidence),
        WOODPECKER_CAPTURE_SCENE_VISIBILITY_CONDITION=condition,
    )
    arguments = [sys.executable, str(TEMP_ROOT / "tests" / TEST_FILES[scene])]
    if scene == "ui":
        arguments += ["--capture-dir", str(evidence / "images")]
    completed = subprocess.run(arguments, cwd=TEMP_ROOT, env=env, capture_output=True, text=True, timeout=900)
    (evidence / "stdout.log").write_text(completed.stdout)
    (evidence / "stderr.log").write_text(completed.stderr)
    expected_pass = condition != "covered"
    guard_failure = "screenshot_window_unavailable" in completed.stderr or "screenshot_window_unavailable" in completed.stdout
    result = {
        "scene": scene,
        "condition": condition,
        "evidence": str(evidence),
        "command": arguments,
        "exit": completed.returncode,
        "expected_pass": expected_pass,
        "strict_acceptance_passed": completed.returncode == 0,
        "guard_rejection_observed": guard_failure,
        "versions": versions,
    }
    if expected_pass:
        assert completed.returncode == 0, result
    else:
        assert completed.returncode != 0 and guard_failure, result
    (evidence / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    return evidence, result


def main():
    if sys.platform != "darwin":
        raise SystemExit("capture_scene_visibility.py requires macOS")
    parser = argparse.ArgumentParser()
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--prepare-scenes", nargs="+", choices=SCENES)
    parser.add_argument("--scene", choices=SCENES)
    parser.add_argument("--full-test", choices=SCENES)
    parser.add_argument("--condition", choices=("visible", "partial", "covered", "restored"))
    args = parser.parse_args()
    if args.prepare_scenes and not args.prepare_only:
        parser.error("--prepare-scenes requires --prepare-only")
    selected = args.prepare_scenes or ([args.scene] if args.scene else [args.full_test] if args.full_test else SCENES)
    helper, versions = prepare(selected)
    print(json.dumps({"event": "build_ready", "work": str(WORK), "scenes": selected,
                      "versions": versions}, indent=2))
    if args.prepare_only:
        return
    if args.scene:
        evidence, result = diagnostic_scene(args.scene, helper, versions)
        print(json.dumps({"event": "gui_measurement_complete", "evidence": str(evidence),
                          "cleanup": result["cleanup"]}, indent=2))
        return
    if args.full_test and args.condition:
        evidence, result = run_full_test(args.full_test, args.condition, helper, versions)
        print(json.dumps({"event": "full_test_complete", "evidence": str(evidence), "result": result}, indent=2))
        return
    parser.error("choose --prepare-only, --scene, or --full-test with --condition")


if __name__ == "__main__":
    main()
