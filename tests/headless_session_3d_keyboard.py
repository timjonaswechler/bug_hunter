"""Opt-in GPU acceptance for keyboard movement in the fixed 3D session fixture.

Build first; execute exactly once only after separate GPU approval:
    WOODPECKER_RUN_GPU_3D_KEYBOARD=1 python3 tests/headless_session_3d_keyboard.py
"""
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import traceback

from headless_session import read_rgb_png
from headless_session_3d import assert_f32, assert_fixture_info, assert_initialized_scene
from slice import CLI as DEFAULT_CLI, ROOT, until

CLI = Path(os.environ.get("WOODPECKER_CLI", DEFAULT_CLI))
READY_TIMEOUT = 60
CLI_TIMEOUT = 40
SERVER_READY_TIMEOUT = 10
PROCESS_STOP_TIMEOUT = 15
IMMUTABLE_SCENE_FIELDS = (
    "camera_translation",
    "camera_rotation",
    "camera_vertical_fov_radians",
    "camera_aspect_ratio",
    "camera_near",
    "camera_far",
    "target_physical_size",
    "target_scale_factor",
    "back_translation",
)


def color_fraction(rows, color, left, top, right, bottom):
    pixels = [
        rows[y][x * 3:x * 3 + 3]
        for y in range(top, bottom)
        for x in range(left, right)
    ]
    return sum(pixel == color for pixel in pixels) / len(pixels)


def assert_immutable_scene(reference, actual):
    for field in IMMUTABLE_SCENE_FIELDS:
        assert actual[field] == reference[field], (field, reference[field], actual[field])


def assert_keyboard_images(initial_rows, moved_rows, released_rows):
    red = b"\xff\x00\x00"
    blue = b"\x00\x00\xff"
    green = b"\x00\xff\x00"
    black = b"\x00\x00\x00"

    # The CPU projection test derives these interior regions from the cuboid
    # front faces. They avoid silhouette edges, which are perspective-shaped.
    assert initial_rows[90][170 * 3:170 * 3 + 3] == red
    assert color_fraction(initial_rows, red, 160, 75, 185, 105) > 0.85
    assert color_fraction(initial_rows, blue, 110, 75, 130, 105) > 0.85
    assert color_fraction(moved_rows, blue, 155, 75, 170, 105) > 0.85
    assert color_fraction(moved_rows, red, 205, 75, 225, 105) > 0.85

    for rows in (initial_rows, moved_rows, released_rows):
        assert color_fraction(rows, green, 15, 15, 65, 35) > 0.95
        assert rows[-1][-3:] == black
    assert moved_rows == released_rows


def assert_keyboard_motion(pre_warp, initial, moved, released, info):
    assert_initialized_scene(pre_warp, initial)
    assert moved["ticks"] == 11
    assert_immutable_scene(initial, moved)
    assert_f32(
        moved["front_translation"][0],
        0.4 + 10 * info["front_step_per_tick"],
        "moved.front_translation[0]",
    )
    assert moved["front_translation"][1:] == initial["front_translation"][1:]

    elapsed_for_movement = moved["virtual_millis"] - initial["virtual_millis"]
    assert elapsed_for_movement > 0 and elapsed_for_movement % 10 == 0
    tick_millis = elapsed_for_movement // 10

    assert released["ticks"] == 12
    assert_immutable_scene(initial, released)
    assert released["front_translation"] == moved["front_translation"]
    assert released["virtual_millis"] - moved["virtual_millis"] == tick_millis
    return tick_millis


def run():
    if os.environ.get("WOODPECKER_RUN_GPU_3D_KEYBOARD") != "1":
        raise SystemExit(
            "3D keyboard GPU acceptance is opt-in; set "
            "WOODPECKER_RUN_GPU_3D_KEYBOARD=1 only after approval"
        )
    directory = Path(
        tempfile.mkdtemp(prefix="headless-session-3d-keyboard-", dir=ROOT / "target")
    )
    print(json.dumps({"evidence_dir": str(directory)}, separators=(",", ":")), flush=True)
    session = None
    address = None
    server_stopped = False
    with (directory / "server.log").open("w+") as server_log, (
        directory / "server-stdout.jsonl"
    ).open("w") as server_stdout, (directory / "commands.jsonl").open("w") as commands:
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

        def cli(*args, check=True):
            result = subprocess.run(
                [str(CLI), "--address", address, *args],
                cwd=ROOT,
                capture_output=True,
                text=True,
                timeout=CLI_TIMEOUT,
            )
            commands.write(
                json.dumps(
                    {
                        "arguments": args,
                        "exit": result.returncode,
                        "stdout": result.stdout,
                        "stderr": result.stderr,
                    }
                )
                + "\n"
            )
            commands.flush()
            if check:
                assert result.returncode == 0, (args, result.stdout, result.stderr)
            return json.loads(result.stdout) if result.stdout.strip() else None

        try:
            assert select.select([server.stdout], [], [], SERVER_READY_TIMEOUT)[0], (
                "no server Ready"
            )
            ready_line = server.stdout.readline()
            server_stdout.write(ready_line)
            server_stdout.flush()
            address = json.loads(ready_line)["address"]
            session = cli(
                "session",
                "create",
                "--config",
                str(ROOT / "tests/fixtures/headless_session_3d.toml"),
            )["id"]

            def ready():
                detail = cli("session", "inspect", session)
                (directory / "session-latest.json").write_text(
                    json.dumps(detail, indent=2) + "\n", encoding="utf-8"
                )
                assert detail["state"] not in ("Failed", "Ended"), detail
                return detail if detail["state"] == "Ready" else None

            detail = until(ready, timeout=READY_TIMEOUT)
            (directory / "session-ready.json").write_text(
                json.dumps(detail, indent=2) + "\n", encoding="utf-8"
            )

            def command(name, arguments):
                pending = cli(
                    "session",
                    "submit",
                    session,
                    "--command",
                    json.dumps({"command": name, "arguments": arguments}),
                )

                def completed():
                    activity = cli("session", "poll", session)
                    matches = [
                        entry["event"]
                        for entry in activity["entries"]
                        if entry["event"].get("request_id") == pending["request_id"]
                        and entry["event"]["kind"] != "pending"
                    ]
                    return matches[0] if matches else None

                result = until(completed, timeout=CLI_TIMEOUT)
                assert result["kind"] == "completed", result
                return result["output"]

            def resource(type_path):
                output = command(
                    "inspect.query",
                    {
                        "source": "resources",
                        "selector": {"kind": "type", "type_path": type_path},
                        "projection": {"kind": "value"},
                    },
                )
                return output["items"][0]["result"]["value"]["value"]

            def state():
                return resource("headless_session_3d::SceneState")

            warps = 0
            captures = 0

            def warp(ticks):
                nonlocal warps
                output = command("tick.warp.start", {"ticks": ticks})
                assert output == {
                    "requested_ticks": ticks,
                    "executed_ticks": ticks,
                    "outcome": "completed",
                }, output
                warps += 1

            def capture(path):
                nonlocal captures
                before = state()
                output = command("screenshot.capture", {"path": path})
                assert output == {
                    "path": path,
                    "width": 321,
                    "height": 181,
                    "overwritten": False,
                }, output
                assert state() == before, "capture advanced ticks, time, camera, or cuboids"
                captures += 1
                artifact = Path(detail["artifact_dir"]) / path
                width, height, rows = read_rgb_png(artifact)
                assert (width, height) == (321, 181)
                return rows

            info = resource("headless_session_3d::Headless3dInfo")
            assert_fixture_info(info)
            pre_warp = state()
            assert pre_warp["ticks"] == 0

            warp(1)
            initial = state()
            initial_rows = capture("screenshots/keyboard-3d-initial.png")

            assert command("input.keyboard.press", {"key": "d"}) is None
            assert state() == initial, "accepted key press moved without an explicit warp"

            warp(10)
            moved = state()
            moved_rows = capture("screenshots/keyboard-3d-moved.png")

            assert command("input.keyboard.release", {"key": "d"}) is None
            assert state() == moved, "accepted key release moved without an explicit warp"

            warp(1)
            released = state()
            released_rows = capture("screenshots/keyboard-3d-released.png")

            tick_millis = assert_keyboard_motion(pre_warp, initial, moved, released, info)
            assert_keyboard_images(initial_rows, moved_rows, released_rows)
            assert (warps, captures, released["ticks"]) == (3, 3, 12)

            cli("session", "stop", session)
            session = None
            cli("server", "stop")
            server_stopped = True
            assert server.wait(timeout=PROCESS_STOP_TIMEOUT) == 0
            summary = {
                "acceptance": "passed",
                "warps": warps,
                "ticks": released["ticks"],
                "captures": captures,
                "front_step_per_tick": info["front_step_per_tick"],
                "front_translation_x": released["front_translation"][0],
                "tick_millis": tick_millis,
                "evidence_dir": str(directory),
            }
            (directory / "summary.json").write_text(
                json.dumps(summary, indent=2) + "\n", encoding="utf-8"
            )
            print(json.dumps(summary, separators=(",", ":")))
        except BaseException as error:
            (directory / "failure.json").write_text(
                json.dumps(
                    {
                        "type": type(error).__name__,
                        "message": str(error),
                        "traceback": traceback.format_exc(),
                    },
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
            raise
        finally:
            if address is not None and session is not None:
                try:
                    cli("session", "stop", session, check=False)
                except (subprocess.SubprocessError, OSError, ValueError):
                    pass
            if address is not None and not server_stopped and server.poll() is None:
                try:
                    cli("server", "stop", check=False)
                except (subprocess.SubprocessError, OSError, ValueError):
                    pass
            if server.poll() is None:
                server.send_signal(signal.SIGINT)
                try:
                    server.wait(timeout=PROCESS_STOP_TIMEOUT)
                except subprocess.TimeoutExpired:
                    server.send_signal(signal.SIGINT)
                    server.wait(timeout=10)
            if server.stdout is not None:
                remainder = server.stdout.read()
                if remainder:
                    server_stdout.write(remainder)
                    server_stdout.flush()
                server.stdout.close()


if __name__ == "__main__":
    run()
