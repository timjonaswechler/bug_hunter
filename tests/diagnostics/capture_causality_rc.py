"""Build and run the fixed Bevy v0.20.0-rc.1 occlusion experiment.

The app, patched Bevy source, Cargo.lock, build output, and evidence all stay
below target/. This script never edits the repository manifests or the normal
woodpecker adapter.
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
from diagnostics.capture_causality_rc_instrument import instrument  # noqa: E402

BEVY_VERSION = "0.20.0-rc.1"
BEVY_TAG = "v0.20.0-rc.1"
BEVY_COMMIT = "1b1f3ec1bec87386d7c19bda7d7870d4e18235c5"
WORK = ROOT / "target/capture-causality-rc"


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
    paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "src/session/screenshot/capture.rs"]
    paths.extend(sorted((ROOT / "src/session/screenshot/capture").glob("**/*")))
    return {str(path.relative_to(ROOT)): sha256(path) for path in paths if path.is_file()}


def verify_tag():
    result = subprocess.run(
        ["git", "ls-remote", "https://github.com/bevyengine/bevy.git", f"refs/tags/{BEVY_TAG}"],
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
    )
    rows = [line.split() for line in result.stdout.splitlines() if line.strip()]
    assert rows == [[BEVY_COMMIT, f"refs/tags/{BEVY_TAG}"]], rows


def write_bootstrap_manifest(path):
    path.parent.mkdir(parents=True)
    path.write_text(
        f'''[package]
name = "capture_causality_rc_bootstrap"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
bevy_render = "={BEVY_VERSION}"
'''
    )
    (path.parent / "src").mkdir()
    (path.parent / "src/lib.rs").write_text("")


def write_app_manifest(path, dependency):
    path.write_text(
        f'''[package]
name = "capture_causality_rc"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
bevy = {{ version = "={BEVY_VERSION}", default-features = false, features = ["3d"] }}
serde_json = "1"
objc2 = "0.6.4"
objc2-app-kit = {{ version = "0.3.2", default-features = false, features = ["std", "NSResponder", "NSView", "NSWindow"] }}
raw-window-handle = "0.6"

[patch.crates-io]
bevy_render = {{ path = "{dependency}" }}
'''
    )


def resolved_versions(app, env):
    output = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--manifest-path", str(app / "Cargo.toml")],
        check=True,
        capture_output=True,
        text=True,
        env=env,
    )
    metadata = json.loads(output.stdout)
    wanted = {"bevy", "bevy_render", "wgpu", "wgpu-core", "wgpu-hal", "wgpu-types"}
    packages = [
        {
            "name": package["name"],
            "version": package["version"],
            "source": package["source"],
            "manifest_path": package["manifest_path"],
        }
        for package in metadata["packages"]
        if package["name"] in wanted
    ]
    assert any(p["name"] == "bevy" and p["version"] == BEVY_VERSION for p in packages)
    assert any(p["name"] == "bevy_render" and p["version"] == BEVY_VERSION for p in packages)
    assert any(p["name"] == "wgpu" for p in packages)
    return sorted(packages, key=lambda item: (item["name"], item["version"]))


def prepare():
    before = protected_hashes()
    if WORK.exists():
        for child in WORK.iterdir():
            if child.name == "cargo-target":
                continue
            if child.is_dir():
                shutil.rmtree(child)
            else:
                child.unlink()
    WORK.mkdir(parents=True, exist_ok=True)
    verify_tag()

    bootstrap = WORK / "bootstrap/Cargo.toml"
    write_bootstrap_manifest(bootstrap)
    run_checked(["cargo", "fetch", "--manifest-path", str(bootstrap)])

    dependency = WORK / "deps/bevy_render"
    shutil.copytree(registry_crate("bevy_render", BEVY_VERSION), dependency)
    crate_manifest = (dependency / "Cargo.toml").read_text()
    assert f'version = "{BEVY_VERSION}"' in crate_manifest
    instrument(dependency)

    app = WORK / "app"
    (app / "src").mkdir(parents=True)
    shutil.copy2(TESTS / "diagnostics/capture_causality_rc_app.rs", app / "src/main.rs")
    shutil.copy2(
        TESTS / "diagnostics/capture_causality_rc_native.rs",
        app / "src/capture_causality_rc_native.rs",
    )
    write_app_manifest(app / "Cargo.toml", dependency)

    helper = WORK / "capture-causality-rc-deck"
    run_checked(
        [
            "xcrun",
            "swiftc",
            str(TESTS / "diagnostics/capture_causality_rc_deck.swift"),
            "-o",
            str(helper),
        ],
        cwd=ROOT,
    )
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(WORK / "cargo-target")
    run_checked(
        ["cargo", "build", "--manifest-path", str(app / "Cargo.toml")],
        cwd=ROOT,
        env=env,
    )
    versions = {
        "bevy_release_tag": BEVY_TAG,
        "bevy_release_commit": BEVY_COMMIT,
        "packages": resolved_versions(app, env),
        "cargo_lock_sha256": sha256(app / "Cargo.lock"),
    }
    (WORK / "versions.json").write_text(json.dumps(versions, indent=2) + "\n")
    after = protected_hashes()
    assert before == after, "prepare changed a protected root manifest, lockfile, or adapter file"
    return app, helper, env, versions


def read_json_lines(path):
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def native_fields(path, after_sample=0):
    return [
        row["fields"]
        for row in read_json_lines(path)
        if row.get("source") == "appkit_main" and row["fields"]["sample_id"] > after_sample
    ]


def stable_key(row):
    return (
        row["visible"],
        row["key"],
        row["miniaturized"],
        row["occlusion_visible"],
        row["native_x"],
        row["native_y"],
        row["native_width"],
        row["native_height"],
        row["bevy_physical_width"],
        row["bevy_physical_height"],
        row["bevy_scale_factor"],
        row["camera_active"],
        row["camera_target_size"],
        row["camera_viewport"],
        tuple(row["camera_translation"]),
        tuple(row["camera_rotation"]),
        row["fixture"],
    )


def wait_stable_native(path, predicate, description, after_sample=0, timeout=10):
    deadline = time.monotonic() + timeout
    required_samples = 8
    required_span_ns = 100_000_000
    while time.monotonic() < deadline:
        rows = native_fields(path, after_sample)
        matching_tail = []
        for row in reversed(rows):
            if not predicate(row):
                break
            matching_tail.append(row)
        matching_tail.reverse()
        if len(matching_tail) >= required_samples:
            tail = matching_tail[-required_samples:]
            if (
                len({stable_key(row) for row in tail}) == 1
                and tail[-1]["unix_time_ns"] - tail[0]["unix_time_ns"] >= required_span_ns
            ):
                return {
                    "description": description,
                    "sample_count": required_samples,
                    "quiet_span_ns": tail[-1]["unix_time_ns"] - tail[0]["unix_time_ns"],
                    "first": tail[0],
                    "last": tail[-1],
                }
        time.sleep(0.02)
    raise AssertionError(f"{description} did not reach eight stable main-thread samples over 100 ms")


def rect(row, prefix="native_"):
    return (
        float(row[prefix + "x"]),
        float(row[prefix + "y"]),
        float(row[prefix + "width"]),
        float(row[prefix + "height"]),
    )


def intersection_area(a, b):
    ax, ay, aw, ah = a
    bx, by, bw, bh = b
    width = max(0.0, min(ax + aw, bx + bw) - max(ax, bx))
    height = max(0.0, min(ay + ah, by + bh) - max(ay, by))
    return width * height


def png_stats(path):
    width, height, rows = rgb_pixels(path)
    data = path.read_bytes()
    return {
        "width": width,
        "height": height,
        "nonzero_rgb_bytes": sum(value != 0 for row in rows for value in row),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def raw_stats(path):
    data = path.read_bytes()
    return {
        "byte_length": len(data),
        "nonzero_bytes": sum(value != 0 for value in data),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def wait_json(stream, event, timeout=30):
    deadline = time.monotonic() + timeout
    seen = []
    while time.monotonic() < deadline:
        ready, _, _ = select.select([stream], [], [], min(0.25, deadline - time.monotonic()))
        if not ready:
            continue
        line = stream.readline()
        if not line:
            break
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            seen.append({"unparsed": line.rstrip()})
            continue
        seen.append(row)
        if row.get("event") == event:
            return row, seen
    raise AssertionError(f"app did not emit {event}; seen={seen[-10:]}")


def wait_chain(log_path, screenshot_entity, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        rows = read_json_lines(log_path)
        chain = [row for row in rows if row.get("screenshot_entity") == screenshot_entity]
        if any(row.get("event") == "image_returned" for row in chain):
            prepared = next(row for row in chain if row["event"] == "screenshot_prepared")
            acquisitions = [
                row
                for row in rows
                if row.get("event") == "surface_acquire"
                and row.get("frame_id") == prepared["frame_id"]
            ]
            return {"events": chain, "surface_acquire": acquisitions}
        time.sleep(0.02)
    raise AssertionError(f"no complete Bevy chain for screenshot entity {screenshot_entity}")


def chain_event(chain, event):
    matches = [row for row in chain["events"] if row.get("event") == event]
    assert len(matches) == 1, (event, matches, chain)
    return matches[0]


def experiment(app, helper, build_env, versions):
    evidence = Path(tempfile.mkdtemp(prefix="capture-causality-rc-run-", dir=ROOT / "target"))
    log_path = evidence / "causality.jsonl"
    native_log = evidence / "appkit-main.jsonl"
    native_latest = evidence / "appkit-latest.json"
    raw_dir = evidence / "raw-readbacks"
    png_dir = evidence / "png"
    env = build_env.copy()
    env.update(
        WOODPECKER_CAPTURE_CAUSALITY_RC_LOG=str(log_path),
        WOODPECKER_CAPTURE_CAUSALITY_RC_NATIVE_LOG=str(native_log),
        WOODPECKER_CAPTURE_CAUSALITY_RC_NATIVE_LATEST=str(native_latest),
        WOODPECKER_CAPTURE_CAUSALITY_RC_RAW_DIR=str(raw_dir),
        WOODPECKER_CAPTURE_CAUSALITY_RC_PNG_DIR=str(png_dir),
    )
    app_stderr_handle = (evidence / "app.stderr.log").open("w")
    deck_stderr_handle = (evidence / "deck.stderr.log").open("w")
    binary = WORK / "cargo-target/debug/capture_causality_rc"
    process = subprocess.Popen(
        [str(binary)],
        cwd=app,
        env=env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=app_stderr_handle,
        text=True,
        bufsize=1,
    )
    deck = None
    commands = []
    result = {"status": "running", "evidence": str(evidence), "versions": versions}
    try:
        ready, output = wait_json(process.stdout, "app_ready", timeout=45)
        commands.extend(output)
        deck = subprocess.Popen(
            [str(helper)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=deck_stderr_handle,
            text=True,
            bufsize=1,
        )

        def deck_state(command, expected):
            deck.stdin.write(command + "\n")
            deck.stdin.flush()
            row = None
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                readable, _, _ = select.select([deck.stdout], [], [], 0.25)
                if readable:
                    row = json.loads(deck.stdout.readline())
                    break
            assert row and row["deck_state"] == expected, (command, row)
            commands.append({"deck_command": command, "deck_result": row})
            return row

        last_sample = 0
        captures = []

        def capture(label, native_state, deck_row):
            nonlocal last_sample
            command = f"capture {label}"
            process.stdin.write(command + "\n")
            process.stdin.flush()
            complete, seen = wait_json(process.stdout, "capture_complete", timeout=30)
            commands.append({"app_command": command, "app_events": seen})
            spawned = next(row for row in seen if row.get("event") == "capture_spawned")
            assert complete["label"] == label
            assert complete["screenshot_entity"] == spawned["screenshot_entity"]
            chain = wait_chain(log_path, spawned["screenshot_entity"])
            image = chain_event(chain, "image_returned")
            path = Path(complete["path"])
            assert path.exists()
            raw_path = Path(image["raw_path"])
            assert raw_path.exists()
            last_sample = native_state["last"]["sample_id"]
            item = {
                "label": label,
                "native": native_state,
                "deck": deck_row,
                "spawned": spawned,
                "complete": complete,
                "png": png_stats(path),
                "raw": raw_stats(raw_path),
                "chain": chain,
            }
            captures.append(item)
            return item

        small = deck_state("small", "small")
        baseline_native = wait_stable_native(
            native_log,
            lambda row: row["visible"] and not row["key"] and not row["miniaturized"]
            and row["occlusion_visible"],
            "visible unfocused baseline",
            after_sample=last_sample,
        )
        baseline = capture("visible-unfocused", baseline_native, small)
        target = rect(baseline_native["last"])
        target_args = " ".join(str(value) for value in target)

        partial_deck = deck_state(f"partial {target_args}", "partial")
        partial_area = intersection_area(target, rect(partial_deck, prefix=""))
        target_area = target[2] * target[3]
        assert 0.0 < partial_area < target_area, (target, partial_deck, partial_area)
        partial_native = wait_stable_native(
            native_log,
            lambda row: row["visible"] and not row["key"] and not row["miniaturized"]
            and row["occlusion_visible"] and rect(row) == target,
            "geometrically partial occlusion",
            after_sample=last_sample,
        )
        partial = capture("partially-covered", partial_native, partial_deck)

        cover_deck = deck_state(f"cover {target_args}", "cover")
        cover_area = intersection_area(target, rect(cover_deck, prefix=""))
        assert abs(cover_area - target_area) < 0.01, (target, cover_deck, cover_area)
        covered_native = wait_stable_native(
            native_log,
            lambda row: row["visible"] and not row["miniaturized"]
            and not row["occlusion_visible"] and rect(row) == target,
            "geometrically full occlusion",
            after_sample=last_sample,
        )
        covered = capture("fully-covered", covered_native, cover_deck)

        restored_deck = deck_state("small", "small")
        restored_native = wait_stable_native(
            native_log,
            lambda row: row["visible"] and not row["key"] and not row["miniaturized"]
            and row["occlusion_visible"] and rect(row) == target,
            "restored visible unfocused state",
            after_sample=last_sample,
        )
        restored = capture("restored", restored_native, restored_deck)

        assert [item["complete"]["ordinal"] for item in captures] == [1, 2, 3, 4]
        assert len(read_json_lines(log_path)) > 0
        assert len([row for row in read_json_lines(log_path) if row.get("event") == "screenshot_prepared"]) == 4
        fixture_states = {
            (
                item["spawned"]["fixture"],
                item["spawned"]["physical_width"],
                item["spawned"]["physical_height"],
                item["spawned"]["scale_factor"],
                tuple(item["spawned"]["camera_translation"]),
                tuple(item["spawned"]["camera_rotation"]),
            )
            for item in captures
        }
        assert len(fixture_states) == 1, fixture_states
        assert all(item["png"]["width"] == 1280 and item["png"]["height"] == 720 for item in captures)

        for item in (baseline, partial, restored):
            assert chain_event(item["chain"], "queue_submitted")["copy_encoded"] is True
            assert chain_event(item["chain"], "map_success")["copy_encoded"] is True
            assert chain_event(item["chain"], "image_returned")["nonzero_rgb_bytes"] > 0
            assert any(row["outcome"] in ("success", "suboptimal") for row in item["chain"]["surface_acquire"])
        assert baseline["png"] == partial["png"] == restored["png"]
        assert baseline["raw"] == partial["raw"] == restored["raw"]

        covered_acquire = [row["outcome"] for row in covered["chain"]["surface_acquire"]]
        covered_submit = chain_event(covered["chain"], "queue_submitted")
        covered_map = chain_event(covered["chain"], "map_success")
        covered_image = chain_event(covered["chain"], "image_returned")
        assert covered_submit["copy_encoded"] == covered_map["copy_encoded"]
        assert covered_image["copy_encoded"] == covered_map["copy_encoded"]
        assert covered_image["nonzero_all_bytes"] == covered["raw"]["nonzero_bytes"]
        assert (covered_image["nonzero_rgb_bytes"] == 0) == (
            covered["png"]["nonzero_rgb_bytes"] == 0
        )
        if "occluded" in covered_acquire:
            skipped = chain_event(covered["chain"], "copy_skipped")
            assert skipped["reason"] == "swap_chain_view_missing"
            assert covered_submit["copy_encoded"] is False
            verdict = (
                "reproduced_skip_copy_zero_readback"
                if covered_image["nonzero_rgb_bytes"] == 0
                else "occluded_skip_copy_nonzero_readback"
            )
        elif any(outcome in ("success", "suboptimal") for outcome in covered_acquire):
            assert covered_submit["copy_encoded"] is True
            chain_event(covered["chain"], "copy_texture_to_buffer_encoded")
            verdict = (
                "not_reproduced_copy_nonzero_readback"
                if covered_image["nonzero_rgb_bytes"] > 0
                else "copy_completed_zero_readback"
            )
        else:
            if covered_submit["copy_encoded"]:
                chain_event(covered["chain"], "copy_texture_to_buffer_encoded")
            else:
                chain_event(covered["chain"], "copy_skipped")
            verdict = "different_surface_outcome_" + "_".join(covered_acquire)

        identities = {
            (
                chain_event(item["chain"], "image_returned")["capture_id"],
                chain_event(item["chain"], "image_returned")["buffer_id"],
                chain_event(item["chain"], "image_returned")["capture_texture_id"],
            )
            for item in captures
        }
        assert len(identities) == 4, identities
        for item in captures:
            chain_ids = {
                (row["capture_id"], row["buffer_id"], row["capture_texture_id"])
                for row in item["chain"]["events"]
                if "capture_id" in row
            }
            assert len(chain_ids) == 1, chain_ids

        process.stdin.write("quit\n")
        process.stdin.flush()
        process.wait(timeout=10)
        assert process.returncode == 0
        deck_state("quit", "quit")
        deck.wait(timeout=5)
        assert deck.returncode == 0
        app_stderr_handle.flush()
        stderr = (evidence / "app.stderr.log").read_text()
        assert "DeviceLost" not in stderr and "Validation Error" not in stderr, stderr
        result.update(
            status="passed",
            verdict=verdict,
            scope="standalone_bevy_repro_without_woodpecker_adapter",
            capture_count=4,
            geometry={
                "target": target,
                "partial_intersection_area": partial_area,
                "full_intersection_area": cover_area,
                "target_area": target_area,
            },
            captures=captures,
            visible_png_sha256=baseline["png"]["sha256"],
            restored_equals_baseline=True,
            partial_equals_baseline=True,
        )
        return evidence, result
    except BaseException as error:
        result.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        (evidence / "commands.json").write_text(json.dumps(commands, indent=2) + "\n")
        if process.poll() is None:
            process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        if deck is not None and deck.poll() is None:
            try:
                deck.stdin.write("quit\n")
                deck.stdin.flush()
                deck.wait(timeout=3)
            except Exception:
                deck.terminate()
                deck.wait(timeout=3)
        cleanup = {
            "app_pid": process.pid,
            "app_returncode": process.returncode,
            "deck_pid": deck.pid if deck else None,
            "deck_returncode": deck.returncode if deck else None,
            "app_running": process.poll() is None,
            "deck_running": deck is not None and deck.poll() is None,
        }
        result["cleanup"] = cleanup
        (evidence / "cleanup.json").write_text(json.dumps(cleanup, indent=2) + "\n")
        (evidence / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        app_stderr_handle.close()
        deck_stderr_handle.close()


def main():
    if sys.platform != "darwin":
        raise SystemExit("capture_causality_rc.py requires macOS")
    parser = argparse.ArgumentParser()
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    app, helper, env, versions = prepare()
    print(json.dumps({"event": "build_ready", "work": str(WORK), "versions": versions}, indent=2))
    if not args.prepare_only:
        evidence, result = experiment(app, helper, env, versions)
        print(json.dumps({"event": "gui_slot_released", "evidence": str(evidence), "cleanup": result["cleanup"]}, indent=2))


if __name__ == "__main__":
    main()
