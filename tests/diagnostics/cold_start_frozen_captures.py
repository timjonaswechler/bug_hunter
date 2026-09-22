"""Measure fresh-process and repeated screenshot results without advancing simulation.

Requires an awake desktop and prebuilt woodpecker and blend_modes binaries.
This is a diagnostic measurement, not a pass-until-green acceptance test.
All sessions receive exactly one explicit tick before their captures.
"""

import argparse
import hashlib
import json
from pathlib import Path
import select
import signal
import subprocess
import sys
import tempfile
import time


TESTS = Path(__file__).resolve().parents[1]
ROOT = TESTS.parent
sys.path.insert(0, str(TESTS))

from mesh_picking import rgb_pixels  # noqa: E402
from slice import CLI  # noqa: E402


def run(session_count, capture_count, verify_surface_guard=False):
    directory = Path(tempfile.mkdtemp(prefix="cold-start-", dir=ROOT / "target"))
    print(f"Evidence: {directory}", flush=True)
    measurements_path = directory / "measurements.jsonl"
    command_log_path = directory / "commands.jsonl"
    summaries = []

    with (
        (directory / "server.log").open("w") as server_log,
        command_log_path.open("w") as command_log,
        measurements_path.open("w") as measurements,
    ):
        server = subprocess.Popen(
            [
                str(CLI),
                "--address",
                "127.0.0.1:0",
                "server",
                "start",
                "--artifact-dir",
                str(directory / "artifacts"),
                "--shutdown-seconds",
                "10",
            ],
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=server_log,
            text=True,
        )
        active_session = None
        try:
            assert select.select([server.stdout], [], [], 10)[0], "server did not become Ready"
            address = json.loads(server.stdout.readline())["address"]

            def cli(*args):
                started = time.monotonic()
                result = subprocess.run(
                    [str(CLI), "--address", address, *args],
                    cwd=ROOT,
                    capture_output=True,
                    text=True,
                    timeout=40,
                )
                elapsed_ms = (time.monotonic() - started) * 1000
                command_log.write(
                    json.dumps(
                        {
                            "arguments": args,
                            "elapsed_ms": elapsed_ms,
                            "exit": result.returncode,
                            "stdout": result.stdout,
                            "stderr": result.stderr,
                        }
                    )
                    + "\n"
                )
                command_log.flush()
                assert result.returncode == 0, (args, result.stdout, result.stderr)
                return json.loads(result.stdout), elapsed_ms

            def until(check, description, timeout=20):
                deadline = time.monotonic() + timeout
                while time.monotonic() < deadline:
                    value = check()
                    if value:
                        return value
                    time.sleep(0.025)
                raise AssertionError(description)

            for session_index in range(1, session_count + 1):
                created_at = time.monotonic()
                created, _ = cli(
                    "session",
                    "create",
                    "--config",
                    str(ROOT / "tests/fixtures/blend_modes.toml"),
                )
                active_session = created["id"]

                def ready():
                    detail, _ = cli("session", "inspect", active_session)
                    assert detail["state"] not in ("Failed", "Ended"), detail
                    return detail if detail["state"] == "Ready" else None

                detail = until(
                    ready,
                    f"fresh blend_modes session {session_index} did not become Ready",
                )
                ready_ms = (time.monotonic() - created_at) * 1000
                artifact_dir = Path(detail["artifact_dir"])
                cursor = None

                def command(name, arguments, capture_outcome=False):
                    nonlocal cursor
                    command_started = time.monotonic()
                    pending, _ = cli(
                        "session",
                        "submit",
                        active_session,
                        "--command",
                        json.dumps({"command": name, "arguments": arguments}),
                    )
                    assert (
                        pending["kind"] == "pending" and pending["command"] == name
                    ), pending

                    def completed():
                        nonlocal cursor
                        args = ["session", "poll", active_session, "--wait-ms", "100"]
                        if cursor is not None:
                            args += ["--cursor", json.dumps(cursor)]
                        activity, _ = cli(*args)
                        assert activity["kind"] == "activity", activity
                        cursor = activity["cursor"]
                        for entry in activity["entries"]:
                            event = entry["event"]
                            if (
                                event.get("request_id") == pending["request_id"]
                                and event["kind"] != "pending"
                            ):
                                assert (
                                    event["command"] == name
                                    and (capture_outcome or event["kind"] == "completed")
                                ), event
                                return event
                        return None

                    event = until(
                        completed,
                        f"no outcome for {name} #{pending['request_id']}",
                    )
                    elapsed_ms = (time.monotonic() - command_started) * 1000
                    return (event if capture_outcome else event["output"]), elapsed_ms

                def inspect(handle=None, projection=None):
                    output, _ = command(
                        "inspect.query",
                        {
                            "source": "entities",
                            "entity": handle,
                            "with": [],
                            "without": [],
                            "projection": projection or {"kind": "summary"},
                        },
                    )
                    return output["items"]

                matches = [
                    item["entity"]
                    for item in inspect()
                    if item["result"]["name"] == "camera"
                ]
                assert len(matches) == 1, matches
                camera = matches[0]
                state_paths = [
                    "blend_modes::SceneState",
                    "bevy_camera::camera::Camera",
                    "bevy_transform::components::transform::Transform",
                ]

                def state():
                    items = inspect(
                        camera,
                        {
                            "kind": "components",
                            "selection": {
                                "kind": "listed",
                                "type_paths": state_paths,
                            },
                        },
                    )
                    components = items[0]["result"]["components"]
                    values = {
                        item["component"]["type_path"]: item["value"]["value"]
                        for item in components
                        if item["value"]["status"] == "readable"
                    }
                    assert set(values) == set(state_paths), values
                    camera_value = values["bevy_camera::camera::Camera"]
                    return {
                        "scene": values["blend_modes::SceneState"],
                        "camera_active": camera_value["is_active"],
                        "target_size": camera_value["computed"]["target_info"][
                            "physical_size"
                        ],
                        "transform": values[
                            "bevy_transform::components::transform::Transform"
                        ],
                    }

                before_warp = state()
                assert before_warp["camera_active"] is False, before_warp
                warp_output, warp_ms = command("tick.warp.start", {"ticks": 1})
                assert warp_output == {
                    "requested_ticks": 1,
                    "executed_ticks": 1,
                    "outcome": "completed",
                }, warp_output
                warp_finished_at = time.monotonic()
                capture_states = []
                capture_results = []

                for capture_index in range(1, capture_count + 1):
                    relative_path = (
                        f"screenshots/session-{session_index:02d}-"
                        f"capture-{capture_index:02d}.png"
                    )
                    capture_output, capture_ms = command(
                        "screenshot.capture", {"path": relative_path}, capture_outcome=True
                    )
                    captured_at = time.monotonic()
                    image_path = artifact_dir / relative_path
                    if capture_output["kind"] == "completed":
                        width, height, rows = rgb_pixels(image_path)
                        nonzero_rgb_bytes = sum(value != 0 for row in rows for value in row)
                        digest = hashlib.sha256(image_path.read_bytes()).hexdigest()
                    else:
                        assert capture_output["kind"] == "rejected", capture_output
                        assert capture_output["error"]["code"] == "screenshot_window_unavailable", capture_output
                        assert not image_path.exists(), "rejected capture wrote a file"
                        width = height = nonzero_rgb_bytes = digest = None
                    snapshot = state()
                    capture_states.append(snapshot)
                    result = {
                        "kind": "capture",
                        "session_index": session_index,
                        "capture_index": capture_index,
                        "fresh_process_capture": capture_index == 1,
                        "ticks_before_capture": 1,
                        "elapsed_since_warp_ms": (
                            captured_at - warp_finished_at
                        )
                        * 1000,
                        "capture_command_ms": capture_ms,
                        "all_rgb_zero": nonzero_rgb_bytes == 0 if nonzero_rgb_bytes is not None else None,
                        "nonzero_rgb_bytes": nonzero_rgb_bytes,
                        "rgb_bytes": width * height * 3 if width is not None else None,
                        "width": width,
                        "height": height,
                        "png_sha256": digest,
                        "command_output": capture_output,
                        "state_after_capture": snapshot,
                    }
                    capture_results.append(result)
                    measurements.write(json.dumps(result) + "\n")
                    measurements.flush()
                    if verify_surface_guard:
                        assert result["all_rgb_zero"] is not True, "uncopied black image was accepted"
                    print(
                        json.dumps(
                            {
                                "session": session_index,
                                "capture": capture_index,
                                "outcome": capture_output["kind"],
                                "all_rgb_zero": result["all_rgb_zero"],
                                "nonzero_rgb_bytes": nonzero_rgb_bytes,
                                "elapsed_since_warp_ms": round(
                                    result["elapsed_since_warp_ms"], 1
                                ),
                                "capture_command_ms": round(capture_ms, 1),
                            }
                        ),
                        flush=True,
                    )

                stable = all(item == capture_states[0] for item in capture_states[1:])
                if verify_surface_guard:
                    assert stable, "captures changed the simulation"
                summary = {
                    "kind": "session_summary",
                    "session_index": session_index,
                    "session_id": active_session,
                    "ready_ms": ready_ms,
                    "warp_ms": warp_ms,
                    "before_warp": before_warp,
                    "state_stable_across_captures": stable,
                    "black_results": [
                        item["capture_index"]
                        for item in capture_results
                        if item["all_rgb_zero"]
                    ],
                    "unavailable_results": [
                        item["capture_index"] for item in capture_results
                        if item["command_output"]["kind"] == "rejected"
                    ],
                }
                summaries.append(summary)
                measurements.write(json.dumps(summary) + "\n")
                measurements.flush()

                cli("session", "stop", active_session)

                def ended():
                    stopped, _ = cli("session", "inspect", active_session)
                    return stopped["state"] == "Ended"

                until(ended, f"session {session_index} did not end")
                active_session = None

            cli("server", "stop")
            assert server.wait(timeout=15) == 0
            print(
                json.dumps(
                    {
                        "measurement": "completed",
                        "sessions": session_count,
                        "captures_per_session": capture_count,
                        "black_captures": sum(
                            len(item["black_results"]) for item in summaries
                        ),
                        "unavailable_captures": sum(
                            len(item["unavailable_results"]) for item in summaries
                        ),
                        "evidence": str(directory),
                    }
                ),
                flush=True,
            )
        finally:
            if active_session is not None and server.poll() is None:
                try:
                    cli("session", "stop", active_session)
                except Exception:
                    pass
            if server.poll() is None:
                server.send_signal(signal.SIGINT)
                try:
                    server.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    server.send_signal(signal.SIGINT)
                    server.wait(timeout=10)
            server.stdout.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--sessions", type=int, default=4)
    parser.add_argument("--captures", type=int, default=6)
    parser.add_argument("--verify-surface-guard", action="store_true",
                        help="require valid scene images or explicit unavailable outcomes without files")
    args = parser.parse_args()
    if args.sessions < 1 or args.captures < 2:
        parser.error("--sessions must be >= 1 and --captures must be >= 2")
    run(args.sessions, args.captures, args.verify_surface_guard)
