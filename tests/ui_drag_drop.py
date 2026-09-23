"""Real CLI drag-and-drop acceptance; requires a desktop, not native focus.

Build woodpecker with cli and ui_drag_drop with slice first.
Full CLI evidence and server logs remain under target/ui-drag-drop-*.
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
    directory = Path(tempfile.mkdtemp(prefix="ui-drag-drop-", dir=ROOT / "target"))
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

            session = cli("session", "create", "--config", str(ROOT / "tests/fixtures/ui_drag_drop.toml"))["id"]

            def ready():
                detail = cli("session", "inspect", session)
                assert detail["state"] not in ("Failed", "Ended"), detail
                return detail["state"] == "Ready"

            until(ready, "ui_drag_drop did not become Ready; build the slice binary first")
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

            def inspect(handle=None, projection=None):
                return command("inspect.query", {
                    "source": "entities", "entity": handle, "with": [], "without": [],
                    "projection": projection or {"kind": "summary"},
                })["items"]

            def named(name):
                matches = [item["entity"] for item in inspect() if item["result"]["name"] == name]
                assert len(matches) == 1, (name, matches)
                return matches[0]

            grid = named("drag-grid")

            def component(handle, path):
                values = inspect(handle, {"kind": "components", "selection": {
                    "kind": "listed", "type_paths": [path],
                }})[0]["result"]["components"]
                assert len(values) == 1 and values[0]["value"]["status"] == "readable", values
                return values[0]["value"]["value"]

            def state():
                return component(grid, "ui_drag_drop::SceneState")

            initial = state()
            assert initial == {
                "active_tile": None, "hover_events": 0, "drag_start_events": 0,
                "drag_events": 0, "drag_drop_events": 0, "drag_end_events": 0,
                "drag_sequence": [], "occupancy": ["Amber", "Blue", "Green", "Rose"],
            }, initial
            time.sleep(0.1)
            assert state() == initial, "Inspect or wall time advanced the scene"

            def warp(ticks):
                result = command("tick.warp.start", {"ticks": ticks})
                assert result == {"requested_ticks": ticks, "executed_ticks": ticks,
                                  "outcome": "completed"}, result

            def position(tile):
                return component(tile, "bevy_ui::ui_transform::UiGlobalTransform")[-2:]

            def move(point):
                frozen = state()
                assert command("input.pointer.move_to", {"position": point}) is None
                assert state() == frozen, "queued movement advanced the scene"

            def button(action):
                frozen = state()
                assert command(f"input.pointer.{action}", {"button": "left"}) is None
                assert state() == frozen, "queued button input advanced the scene"

            def expect(**changes):
                actual = state()
                # Hover counts depend on traversed geometry; drag phases and
                # occupancy have a literal oracle independent of those counts.
                expected = {key: value for key, value in initial.items() if key != "hover_events"}
                expected.update(changes)
                assert {key: actual[key] for key in expected} == expected, actual
                return actual

            warp(1)  # Explicit initial layout.
            tiles = {name: named(f"tile-{name.lower()}") for name in initial["occupancy"]}
            original = {name: position(tile) for name, tile in tiles.items()}
            amber, blue = tiles["Amber"], tiles["Blue"]
            root = inspect(grid, {"kind": "hierarchy", "depth": 1})[0]["result"]["root"]
            assert [child["entity"] for child in root["children"]] == list(tiles.values()), root

            def appearance(tile):
                return {
                    path: component(tile, path) for path in [
                        "bevy_ui::ui_transform::UiTransform",
                        "bevy_ui::ui_node::GlobalZIndex",
                        "bevy_ui::ui_node::Outline",
                    ]
                }

            resting = appearance(amber)
            ax, ay = original["Amber"]
            bx, by = original["Blue"]
            assert ax < bx and ay == by, original
            move(original["Amber"])
            warp(1)
            assert expect()["hover_events"] > 0
            button("press")
            warp(1)
            expect()
            midpoint = [(ax + bx) / 2, ay]
            move(midpoint)
            warp(1)
            expect(active_tile="Amber", drag_start_events=1, drag_events=1,
                   drag_sequence=["DragStart", "Drag"])
            assert position(amber) == midpoint
            dragging = appearance(amber)
            assert dragging["bevy_ui::ui_node::GlobalZIndex"] == 1, dragging
            assert all(dragging[path] != resting[path] for path in resting), dragging
            frozen = state()
            time.sleep(0.1)
            assert state() == frozen and position(amber) == midpoint
            move(original["Blue"])
            warp(1)
            expect(active_tile="Amber", drag_start_events=1, drag_events=2,
                   drag_sequence=["DragStart", "Drag", "Drag"])
            assert position(amber) == original["Blue"]
            button("release")
            warp(1)
            expect(drag_start_events=1, drag_events=2, drag_drop_events=1, drag_end_events=1,
                   drag_sequence=["DragStart", "Drag", "Drag", "DragDrop", "DragEnd"],
                   occupancy=["Blue", "Amber", "Green", "Rose"])
            assert position(amber) == original["Blue"]
            assert position(blue) == original["Amber"]
            assert appearance(amber) == resting
            swapped = {**original, "Amber": original["Blue"], "Blue": original["Amber"]}
            assert {name: position(tile) for name, tile in tiles.items()} == swapped

            # A second drag leaves the grid and releases over empty background.
            # Read the actual size to prove that the destination is outside all
            # resting tiles, rather than relying on hard-coded fixture pixels.
            outside = [ax / 4, ay]
            for name, tile in tiles.items():
                size = component(tile, "bevy_ui::ui_node::ComputedNode")["size"]
                assert abs(outside[0] - swapped[name][0]) > size[0] / 2
            move(swapped["Amber"])
            warp(1)
            button("press")
            warp(1)
            move(outside)
            warp(1)
            expect(active_tile="Amber", drag_start_events=2, drag_events=3,
                   drag_drop_events=1, drag_end_events=1,
                   drag_sequence=["DragStart", "Drag", "Drag", "DragDrop", "DragEnd",
                                  "DragStart", "Drag"],
                   occupancy=["Blue", "Amber", "Green", "Rose"])
            assert position(amber) == outside
            button("release")
            warp(1)
            ended = expect(drag_start_events=2, drag_events=3,
                           drag_drop_events=1, drag_end_events=2,
                           drag_sequence=["DragStart", "Drag", "Drag", "DragDrop", "DragEnd",
                                          "DragStart", "Drag", "DragEnd"],
                           occupancy=["Blue", "Amber", "Green", "Rose"])
            assert appearance(amber) == resting
            assert {name: position(tile) for name, tile in tiles.items()} == swapped
            time.sleep(0.1)
            assert state() == ended
            warp(2)
            assert state() == ended, "idle ticks repeated drag/drop events"
            assert {name: position(tile) for name, tile in tiles.items()} == swapped

            cli("session", "stop", session)
            until(lambda: cli("session", "inspect", session)["state"] == "Ended", "session did not end")
            cli("server", "stop")
            assert server.wait(timeout=15) == 0
            print(json.dumps({"acceptance": "passed", "scene": "ui_drag_drop",
                              "evidence": str(directory)}))
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
