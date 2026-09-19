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
            cli("session", "stop", session)
            until(lambda: cli("session", "inspect", session)["state"] == "Ended", "session did not end")
            cli("server", "stop")
            assert server.wait(timeout=15) == 0
            print(json.dumps({"acceptance": "passed", "scene": "logical_state",
                              "updates": released["updates"], "fixed_updates": released["fixed_updates"],
                              "timer_finishes": released["timer_finishes"], "evidence": str(directory)}))
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
