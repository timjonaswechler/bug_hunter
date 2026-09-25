"""Opt-in GPU acceptance for headless keyboard movement through CLI -> Session.

Build first; execute only after separate GPU approval:
    WOODPECKER_RUN_GPU_KEYBOARD=1 python3 tests/headless_session_keyboard.py
"""
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile

from headless_session import read_rgb_png
from slice import CLI as DEFAULT_CLI, ROOT, until

CLI = Path(os.environ.get("WOODPECKER_CLI", DEFAULT_CLI))


def color_positions(rows, color):
    return {
        (x, y)
        for y, row in enumerate(rows)
        for x in range(len(row) // 3)
        if row[x * 3:x * 3 + 3] == color
    }


def run():
    if os.environ.get("WOODPECKER_RUN_GPU_KEYBOARD") != "1":
        raise SystemExit(
            "keyboard GPU acceptance is opt-in; set "
            "WOODPECKER_RUN_GPU_KEYBOARD=1 after approval"
        )
    directory = Path(
        tempfile.mkdtemp(prefix="headless-session-keyboard-", dir=ROOT / "target")
    )
    print(json.dumps({"evidence_dir": str(directory)}, separators=(",", ":")), flush=True)
    with (directory / "server.log").open("w+") as log, (
        directory / "commands.jsonl"
    ).open("w") as commands:
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
            stderr=log,
            text=True,
        )
        try:
            assert select.select([server.stdout], [], [], 10)[0], "no server Ready"
            address = json.loads(server.stdout.readline())["address"]

            def cli(*args):
                result = subprocess.run(
                    [str(CLI), "--address", address, *args],
                    cwd=ROOT,
                    capture_output=True,
                    text=True,
                    timeout=40,
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
                assert result.returncode == 0, (args, result.stdout, result.stderr)
                return json.loads(result.stdout) if result.stdout.strip() else None

            session = cli(
                "session",
                "create",
                "--config",
                str(ROOT / "tests/fixtures/headless_session.toml"),
            )["id"]

            def ready():
                detail = cli("session", "inspect", session)
                assert detail["state"] not in ("Failed", "Ended"), detail
                return detail if detail["state"] == "Ready" else None

            detail = until(ready, timeout=60)

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

                result = until(completed, timeout=40)
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
                return resource("headless_session::SceneState")

            info = resource("headless_session::ImageTargetInfo")
            assert info == {
                "physical_width": 321,
                "physical_height": 181,
                "scale_factor": 1.5,
                "sprite_step_per_tick": 4.0,
            }, info

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
                assert state() == before, "capture advanced simulation, time, or transforms"
                captures += 1
                artifact = Path(detail["artifact_dir"]) / path
                width, height, rows = read_rgb_png(artifact)
                assert (width, height) == (321, 181)
                return rows

            warp(1)
            initial = state()
            assert initial["ticks"] == 1
            assert initial["sprite_translation"] == [0.0, 0.0, 0.0]
            initial_rows = capture("screenshots/keyboard-initial.png")

            assert command("input.keyboard.press", {"key": "d"}) is None
            assert state() == initial, "accepted key press moved without an explicit warp"

            warp(10)
            moved = state()
            assert moved["ticks"] == 11
            assert moved["sprite_translation"] == [40.0, 0.0, 0.0]
            assert moved["camera_translation"] == initial["camera_translation"]
            elapsed_for_movement = moved["virtual_millis"] - initial["virtual_millis"]
            assert elapsed_for_movement > 0 and elapsed_for_movement % 10 == 0
            tick_millis = elapsed_for_movement // 10
            moved_rows = capture("screenshots/keyboard-moved.png")

            assert command("input.keyboard.release", {"key": "d"}) is None
            assert state() == moved, "accepted key release moved without an explicit warp"

            warp(1)
            released = state()
            assert released["ticks"] == 12
            assert released["sprite_translation"] == moved["sprite_translation"]
            assert released["camera_translation"] == initial["camera_translation"]
            assert released["virtual_millis"] - moved["virtual_millis"] == tick_millis
            released_rows = capture("screenshots/keyboard-released.png")

            red = b"\xff\x00\x00"
            green = b"\x00\xff\x00"
            black = b"\x00\x00\x00"
            initial_red = color_positions(initial_rows, red)
            moved_red = color_positions(moved_rows, red)
            released_red = color_positions(released_rows, red)
            shift = int(10 * info["sprite_step_per_tick"] * info["scale_factor"])
            assert initial_red
            assert moved_red == {(x + shift, y) for x, y in initial_red}
            assert released_red == moved_red
            initial_green = color_positions(initial_rows, green)
            assert initial_green
            assert color_positions(moved_rows, green) == initial_green
            assert color_positions(released_rows, green) == initial_green
            for rows in (initial_rows, moved_rows, released_rows):
                assert rows[-1][-3:] == black

            assert (warps, captures, released["ticks"]) == (3, 3, 12)
            cli("session", "stop", session)
            cli("server", "stop")
            assert server.wait(timeout=15) == 0
            summary = {
                "acceptance": "passed",
                "warps": warps,
                "ticks": released["ticks"],
                "captures": captures,
                "sprite_shift_physical": shift,
                "tick_millis": tick_millis,
                "evidence_dir": str(directory),
            }
            (directory / "summary.json").write_text(
                json.dumps(summary, indent=2) + "\n", encoding="utf-8"
            )
            print(json.dumps(summary, separators=(",", ":")))
        finally:
            if server.poll() is None:
                server.send_signal(signal.SIGINT)
                try:
                    server.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    server.send_signal(signal.SIGINT)
                    server.wait(timeout=10)
            if server.stdout is not None:
                server.stdout.close()


if __name__ == "__main__":
    run()
