"""Opt-in GPU acceptance for CLI -> Session -> fixed procedural 3D PNG.

Build first; execute exactly once only after separate GPU approval:
    WOODPECKER_RUN_GPU_3D=1 python3 tests/headless_session_3d.py
"""
import json
import math
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import traceback

from headless_session import read_rgb_png
from slice import CLI as DEFAULT_CLI, ROOT, until

CLI = Path(os.environ.get("WOODPECKER_CLI", DEFAULT_CLI))
READY_TIMEOUT = 60
CLI_TIMEOUT = 40
SERVER_READY_TIMEOUT = 10
PROCESS_STOP_TIMEOUT = 15
# Reflected f32 values are serialized as JSON numbers and parsed by Python as
# f64. This tolerance is approximately one f32 ULP around the fixture's values.
F32_REL_TOLERANCE = 1e-7
F32_ABS_TOLERANCE = 1e-8


def assert_f32(actual, expected, field):
    assert isinstance(actual, (int, float)) and not isinstance(actual, bool), field
    assert math.isfinite(actual), (field, actual)
    assert math.isclose(
        actual,
        expected,
        rel_tol=F32_REL_TOLERANCE,
        abs_tol=F32_ABS_TOLERANCE,
    ), (field, actual, expected)


def assert_f32_vector(actual, expected, field):
    assert isinstance(actual, list) and len(actual) == len(expected), (field, actual)
    for index, (actual_component, expected_component) in enumerate(zip(actual, expected)):
        assert_f32(actual_component, expected_component, f"{field}[{index}]")


def assert_fixture_info(info):
    assert info["physical_width"] == 321
    assert info["physical_height"] == 181
    assert_f32(info["scale_factor"], 1.5, "scale_factor")
    assert_f32(info["vertical_fov_radians"], math.pi / 4, "vertical_fov_radians")
    assert_f32(info["near"], 0.1, "near")
    assert_f32(info["far"], 100.0, "far")
    assert_f32(info["front_step_per_tick"], 0.08, "front_step_per_tick")


def assert_initialized_scene(initial, frozen):
    assert frozen["ticks"] == 1
    assert frozen["virtual_millis"] >= initial["virtual_millis"]
    assert_f32_vector(frozen["camera_translation"], [0.0, 0.0, 8.0], "camera_translation")
    assert_f32_vector(frozen["camera_rotation"], [0.0, 0.0, 0.0, 1.0], "camera_rotation")
    assert_f32(
        frozen["camera_vertical_fov_radians"],
        math.pi / 4,
        "camera_vertical_fov_radians",
    )
    assert_f32(frozen["camera_aspect_ratio"], 321 / 181, "camera_aspect_ratio")
    assert_f32(frozen["camera_near"], 0.1, "camera_near")
    assert_f32(frozen["camera_far"], 100.0, "camera_far")
    assert frozen["target_physical_size"] == [321, 181]
    assert_f32(frozen["target_scale_factor"], 1.5, "target_scale_factor")
    assert_f32_vector(frozen["back_translation"], [-0.4, 0.0, 0.0], "back_translation")
    assert_f32_vector(frozen["front_translation"], [0.4, 0.0, 2.0], "front_translation")


def color_fraction(rows, color, left, top, right, bottom):
    pixels = [
        rows[y][x * 3:x * 3 + 3]
        for y in range(top, bottom)
        for x in range(left, right)
    ]
    return sum(pixel == color for pixel in pixels) / len(pixels)


def run():
    if os.environ.get("WOODPECKER_RUN_GPU_3D") != "1":
        raise SystemExit(
            "3D GPU acceptance is opt-in; set WOODPECKER_RUN_GPU_3D=1 "
            "only after approval"
        )
    directory = Path(
        tempfile.mkdtemp(prefix="headless-session-3d-", dir=ROOT / "target")
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

            info = resource("headless_session_3d::Headless3dInfo")
            assert_fixture_info(info)

            initial = state()
            assert initial["ticks"] == 0
            warp = command("tick.warp.start", {"ticks": 1})
            assert warp == {
                "requested_ticks": 1,
                "executed_ticks": 1,
                "outcome": "completed",
            }
            frozen = state()
            assert_initialized_scene(initial, frozen)

            capture = command(
                "screenshot.capture", {"path": "screenshots/fixed-3d.png"}
            )
            assert capture == {
                "path": "screenshots/fixed-3d.png",
                "width": 321,
                "height": 181,
                "overwritten": False,
            }
            after = state()
            assert after == frozen, "capture advanced time, ticks, camera, or cuboids"

            artifact = Path(detail["artifact_dir"]) / capture["path"]
            width, height, rows = read_rgb_png(artifact)
            assert (width, height) == (321, 181)
            red = b"\xff\x00\x00"
            blue = b"\x00\x00\xff"
            green = b"\x00\xff\x00"
            black = b"\x00\x00\x00"

            # CPU projection test establishes that (170, 90) lies inside both
            # front faces. Red there therefore proves the nearer cuboid won depth.
            assert rows[90][170 * 3:170 * 3 + 3] == red
            assert color_fraction(rows, red, 160, 75, 185, 105) > 0.85
            assert color_fraction(rows, blue, 110, 75, 130, 105) > 0.85
            assert color_fraction(rows, green, 15, 15, 65, 35) > 0.95
            assert rows[height - 1][(width - 1) * 3:width * 3] == black

            summary = {
                "acceptance": "passed",
                "warps": 1,
                "ticks": 1,
                "captures": 1,
                "size": [width, height],
                "evidence_dir": str(directory),
            }
            (directory / "summary.json").write_text(
                json.dumps(summary, indent=2) + "\n", encoding="utf-8"
            )
            cli("session", "stop", session)
            session = None
            cli("server", "stop")
            server_stopped = True
            assert server.wait(timeout=PROCESS_STOP_TIMEOUT) == 0
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
