"""Real CLI acceptance of the existing logical_state scene.

Requires a desktop session. Build woodpecker with cli and logical_state with slice.
Run: python3 tests/logical_state.py
Logs and command evidence are retained under target/logical-state-* on failure too.
"""
import json
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import time

from slice import CLI, ROOT
from mesh_picking import rgb_pixels


def run():
    directory = Path(tempfile.mkdtemp(prefix="logical-state-", dir=ROOT / "target"))
    print(f"Evidence: {directory}", flush=True)
    with (directory / "server.log").open("w") as log, (directory / "commands.jsonl").open("w") as evidence:
        server = subprocess.Popen(
            [str(CLI), "--address", "127.0.0.1:0", "server", "start",
             "--artifact-dir", str(directory / "artifacts"), "--shutdown-seconds", "10"],
            cwd=ROOT, stdout=subprocess.PIPE, stderr=log, text=True,
        )
        try:
            assert select.select([server.stdout], [], [], 10)[0], "server did not become Ready"
            address = json.loads(server.stdout.readline())["address"]

            def cli(*args):
                result = subprocess.run(
                    [str(CLI), "--address", address, *args], cwd=ROOT,
                    capture_output=True, text=True, timeout=40,
                )
                evidence.write(json.dumps({"arguments": args, "exit": result.returncode,
                                           "stdout": result.stdout, "stderr": result.stderr}) + "\n")
                evidence.flush()
                assert result.returncode == 0, (args, result.stdout, result.stderr)
                return json.loads(result.stdout)

            def until(check, description, timeout=15):
                deadline = time.monotonic() + timeout
                while time.monotonic() < deadline:
                    value = check()
                    if value:
                        return value
                    time.sleep(0.025)
                raise AssertionError(description)

            session = cli("session", "create", "--config", str(ROOT / "tests/fixtures/logical_state.toml"))["id"]

            def ready():
                detail = cli("session", "inspect", session)
                assert detail["state"] not in ("Failed", "Ended"), detail
                return detail["state"] == "Ready"

            until(ready, "logical_state did not become Ready; build the slice binary first")
            cursor = None

            def command(name, arguments):
                nonlocal cursor
                pending = cli("session", "submit", session, "--command",
                              json.dumps({"command": name, "arguments": arguments}))
                assert pending["kind"] == "pending" and pending["command"] == name, pending

                def completed():
                    nonlocal cursor
                    args = ["session", "poll", session, "--wait-ms", "100"]
                    if cursor is not None:
                        args += ["--cursor", json.dumps(cursor)]
                    activity = cli(*args)
                    assert activity["kind"] == "activity", activity
                    cursor = activity["cursor"]
                    for entry in activity["entries"]:
                        event = entry["event"]
                        if event.get("request_id") == pending["request_id"] and event["kind"] != "pending":
                            assert event["command"] == name and event["kind"] == "completed", event
                            return event
                    return None

                return until(completed, f"no outcome for {name} #{pending['request_id']}")["output"]

            entities = command("inspect.query", {
                "source": "entities", "entity": None, "with": [], "without": [],
                "projection": {"kind": "summary"},
            })["items"]
            matches = [item["entity"] for item in entities if item["result"]["name"] == "logical-state"]
            assert len(matches) == 1, entities
            handle = matches[0]

            def state():
                items = command("inspect.query", {
                    "source": "entities", "entity": handle, "with": [], "without": [],
                    "projection": {"kind": "components", "selection": {
                        "kind": "listed", "type_paths": ["logical_state::SessionObservation"],
                    }},
                })["items"]
                value = items[0]["result"]["components"][0]["value"]
                assert value["status"] == "readable", items
                return value["value"]

            initial = state()
            assert initial == {
                "updates": 0, "fixed_updates": 0, "timer_finishes": 0,
                "pointer_presses": 0, "key_a_held": False,
                "key_a_presses": 0, "key_a_releases": 0,
            }, initial
            time.sleep(0.1)
            assert state() == initial, "Inspect or real waiting advanced the scene"

            def warp(ticks, pace=None):
                args = {"ticks": ticks}
                if pace is not None:
                    args["pace"] = pace
                assert command("tick.warp.start", args) == {
                    "requested_ticks": ticks, "executed_ticks": ticks, "outcome": "completed",
                }

            # Bevy's first Time update initializes its clock with zero delta.
            # Subsequent fixture ticks must each represent 20 ms, independently
            # of CLI round trips or wall-clock pacing.
            def expect(**changes):
                expected = {**initial, **changes}
                actual = state()
                assert actual == expected, {"expected": expected, "actual": actual}
                return actual

            warp(1)
            expect(updates=1)
            warp(1)
            expect(updates=2, fixed_updates=2)
            warp(1)
            before_press = expect(updates=3, fixed_updates=4, timer_finishes=1)
            assert command("input.keyboard.press", {"key": "a"}) is None
            time.sleep(0.1)
            assert state() == before_press, "accepted key press ran without an explicit tick"
            warp(1)
            expect(updates=4, fixed_updates=6, timer_finishes=1,
                   key_a_held=True, key_a_presses=1)
            # Pacing only limits real execution speed. It must not replace the
            # application's 20 ms time step or repeat the key-press edge.
            warp(3, {"kind": "ticks_per_second", "target": 25})
            held = expect(updates=7, fixed_updates=12, timer_finishes=3,
                          key_a_held=True, key_a_presses=1)
            assert command("input.keyboard.release", {"key": "a"}) is None
            time.sleep(0.1)
            assert state() == held, "accepted key release ran without an explicit tick"
            warp(1, {"kind": "as_fast_as_possible"})
            released = expect(updates=8, fixed_updates=14, timer_finishes=3,
                              key_a_presses=1, key_a_releases=1)
            time.sleep(0.1)
            assert state() == released, "idle control loop advanced fixed steps or timers"
            # The preceding explicit ticks have resolved the button layout.
            buttons = [item["entity"] for item in entities
                       if item["result"]["name"] == "logical-button"]
            assert len(buttons) == 1, entities
            layout = command("inspect.query", {
                "source": "entities", "entity": buttons[0], "with": [], "without": [],
                "projection": {"kind": "components", "selection": {
                    "kind": "listed", "type_paths": [
                        "bevy_ui::ui_transform::UiGlobalTransform",
                    ],
                }},
            })["items"][0]["result"]["components"][0]["value"]
            assert layout["status"] == "readable", layout
            position = layout["value"][-2:]
            assert 0 < position[0] < 640 and 0 < position[1] < 360, position
            assert state() == released, "layout Inspect advanced the scene"
            assert command("input.pointer.move_to", {"position": position}) is None
            assert state() == released, "accepted pointer move ran without a tick"
            warp(1)
            positioned = expect(updates=9, fixed_updates=16, timer_finishes=4,
                                key_a_presses=1, key_a_releases=1)
            artifact_root = Path(cli("session", "inspect", session)["artifact_dir"])
            screenshots = []

            def capture(name, expected):
                assert state() == expected
                path = f"screenshots/{name}.png"
                result = command("screenshot.capture", {"path": path})
                assert result == {"path": path, "width": 640, "height": 360,
                                  "overwritten": False}, result
                width, height, pixels = rgb_pixels(artifact_root / path)
                assert (width, height) == (640, 360)
                x, y = map(round, position)
                # Known scene samples, not a general reference-image comparison.
                assert tuple(pixels[y][x * 3:x * 3 + 3]) == (0, 255, 0)
                assert tuple(pixels[10][30:33]) == (0, 0, 0)
                assert state() == expected, "capture advanced updates, timers or input"
                screenshots.append(str(artifact_root / path))
                return pixels

            baseline = capture("positioned", positioned)
            assert command("input.pointer.press", {"button": "left"}) is None
            assert state() == positioned, "accepted pointer press ran without a tick"
            assert capture("press-pending", positioned) == baseline
            warp(1)
            expect(updates=10, fixed_updates=18, timer_finishes=4,
                   pointer_presses=1, key_a_presses=1, key_a_releases=1)
            warp(3)
            pointer_held = expect(updates=13, fixed_updates=24, timer_finishes=6,
                                  pointer_presses=1, key_a_presses=1, key_a_releases=1)
            assert capture("held", pointer_held) == baseline
            assert command("input.pointer.release", {"button": "left"}) is None
            assert state() == pointer_held, "accepted pointer release ran without a tick"
            assert capture("release-pending", pointer_held) == baseline
            warp(1)
            final = expect(updates=14, fixed_updates=26, timer_finishes=6,
                           pointer_presses=1, key_a_presses=1, key_a_releases=1)
            assert capture("released", final) == baseline
            cli("session", "stop", session)
            until(lambda: cli("session", "inspect", session)["state"] == "Ended", "session did not end")
            cli("server", "stop")
            assert server.wait(timeout=15) == 0
            print(json.dumps({"acceptance": "passed", "scene": "logical_state",
                              "updates": final["updates"], "fixed_updates": final["fixed_updates"],
                              "timer_finishes": final["timer_finishes"],
                              "pointer_presses": final["pointer_presses"],
                              "screenshots": screenshots, "evidence": str(directory)}))
        finally:
            if server.poll() is None:
                server.send_signal(signal.SIGINT)
                try:
                    server.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    server.send_signal(signal.SIGINT)
                    server.wait(timeout=10)
            server.stdout.close()


if __name__ == "__main__":
    run()
