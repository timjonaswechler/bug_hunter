"""CLI acceptance of game_menu. Requires a desktop, not native focus.

Build woodpecker with cli and game_menu with slice before running this file.
Each run retains server.log and full CLI responses under target/game-menu-*.
"""
import json
import math
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import time

from slice import CLI, ROOT


def run():
    directory = Path(tempfile.mkdtemp(prefix="game-menu-", dir=ROOT / "target"))
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

            session = cli("session", "create", "--config", str(ROOT / "tests/fixtures/game_menu.toml"))["id"]

            def ready():
                detail = cli("session", "inspect", session)
                assert detail["state"] not in ("Failed", "Ended"), detail
                return detail["state"] == "Ready"

            until(ready, "game_menu did not become Ready; build the slice binary first")
            cursor = None

            def command(name, arguments, rejection=None):
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
                            assert event["command"] == name, event
                            return event
                    return None

                event = until(completed, f"no outcome for {name} #{pending['request_id']}")
                if rejection:
                    assert event["kind"] == "rejected" and event["error"]["code"] == rejection, event
                    return None
                assert event["kind"] == "completed", event
                return event["output"]

            def inspect(handle=None, projection=None):
                return command("inspect.query", {
                    "source": "entities", "entity": handle, "with": [], "without": [],
                    "projection": projection or {"kind": "summary"},
                })["items"]

            def named(name):
                matches = [item["entity"] for item in inspect() if item["result"]["name"] == name]
                assert len(matches) == 1, (name, matches)
                return matches[0]

            observation = named("game-menu-state")

            def components(handle, paths):
                values = inspect(handle, {"kind": "components", "selection": {
                    "kind": "listed", "type_paths": paths,
                }})[0]["result"]["components"]
                assert all(item["value"]["status"] == "readable" for item in values), values
                return [item["value"]["value"] for item in values]

            def state():
                return components(observation, ["game_menu::SessionObservation"])[0]

            initial = state()
            assert initial == {
                "game_state": "Splash", "menu_state": "Disabled",
                "display_quality": "Medium", "volume": 7,
                "splash_elapsed_seconds": 0.0, "splash_duration_seconds": 1.0,
                "game_elapsed_seconds": 0.0, "game_duration_seconds": 5.0,
            }, initial
            time.sleep(0.1)
            assert state() == initial, "Inspect or wall time advanced the scene"

            def warp(ticks):
                result = command("tick.warp.start", {"ticks": ticks})
                assert result == {"requested_ticks": ticks, "executed_ticks": ticks,
                                  "outcome": "completed"}, result

            def expect(**changes):
                expected = {**initial, **changes}
                actual = state()
                assert actual.keys() == expected.keys(), actual
                for key, value in expected.items():
                    # Reflected f32 seconds are represented as JSON f64 numbers.
                    matches = (math.isclose(actual[key], value, rel_tol=0, abs_tol=1e-6)
                               if isinstance(value, float) else actual[key] == value)
                    assert matches, {"expected": expected, "actual": actual}

            # First Time update initializes delta to zero. Timer completion in
            # Update requests a transition; it does not apply it in that tick.
            warp(1)
            expect()
            splash = named("splash-screen")
            warp(9)
            expect(splash_elapsed_seconds=0.9)
            warp(1)
            expect(splash_elapsed_seconds=1.0)
            warp(1)
            expect(game_state="Menu", splash_elapsed_seconds=1.0)
            # OnEnter(ScreenState::Menu) queues MenuState::Main for the next
            # StateTransition schedule, rather than recursively entering it.
            warp(1)
            expect(game_state="Menu", menu_state="Main", splash_elapsed_seconds=1.0)

            def dead(handle):
                command("inspect.query", {
                    "source": "entities", "entity": handle, "with": [], "without": [],
                    "projection": {"kind": "summary"},
                }, rejection="entity_not_found")

            def hierarchy(name, children):
                handle = named(name)
                root = inspect(handle, {"kind": "hierarchy", "depth": 2})[0]["result"]["root"]
                assert root["entity"] == handle and root["name"] == name, root
                assert [child["name"] for child in root["children"]] == children, root
                return handle

            dead(splash)
            main = hierarchy("main-menu-screen", [
                "main-menu-title", "new-game-button", "settings-button", "quit-button",
            ])

            def press(name):
                button = named(name)
                transform = components(button, ["bevy_ui::ui_transform::UiGlobalTransform"])[0]
                position = transform[-2:]
                assert 0 < position[0] < 800 and 0 < position[1] < 600, transform
                frozen = state()
                assert command("input.pointer.move_to", {"position": position}) is None
                assert command("input.pointer.press", {"button": "left"}) is None
                assert state() == frozen, "queued click advanced the scene"
                return button

            def release(button):
                assert components(button, ["bevy_ui::focus::Interaction"])[0] == "Pressed"
                assert command("input.pointer.release", {"button": "left"}) is None

            old_button = press("settings-button")
            warp(1)
            release(old_button)
            expect(game_state="Menu", menu_state="Main", splash_elapsed_seconds=1.0)
            warp(1)
            dead(main)
            dead(old_button)
            hierarchy("settings-menu-screen", [
                "settings-title", "display-settings-button", "sound-settings-button", "settings-back-button",
            ])
            expect(game_state="Menu", menu_state="Settings", splash_elapsed_seconds=1.0)

            button = press("display-settings-button")
            warp(1)
            release(button)
            warp(1)
            expect(game_state="Menu", menu_state="SettingsDisplay", splash_elapsed_seconds=1.0)
            hierarchy("display-settings-screen", [
                "display-settings-title", "quality-low-button", "quality-medium-button",
                "quality-high-button", "display-back-button",
            ])
            button = press("quality-high-button")
            warp(1)
            release(button)
            expect(game_state="Menu", menu_state="SettingsDisplay",
                   splash_elapsed_seconds=1.0, display_quality="High")
            warp(1)
            button = press("display-back-button")
            warp(1)
            release(button)
            warp(1)
            expect(game_state="Menu", menu_state="Settings",
                   splash_elapsed_seconds=1.0, display_quality="High")
            button = press("sound-settings-button")
            warp(1)
            release(button)
            warp(1)
            expect(game_state="Menu", menu_state="SettingsSound",
                   splash_elapsed_seconds=1.0, display_quality="High")
            hierarchy("sound-settings-screen", [
                "sound-settings-title", "volume-options", "sound-back-button",
            ])
            hierarchy("volume-options", [f"volume-{i}-button" for i in range(10)])
            button = press("volume-3-button")
            warp(1)
            release(button)
            expect(game_state="Menu", menu_state="SettingsSound",
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            warp(1)
            button = press("sound-back-button")
            warp(1)
            release(button)
            warp(1)
            expect(game_state="Menu", menu_state="Settings",
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            button = press("settings-back-button")
            warp(1)
            release(button)
            warp(1)
            expect(game_state="Menu", menu_state="Main",
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            new_main = named("main-menu-screen")
            assert new_main != main
            dead(main)
            button = press("new-game-button")
            warp(1)
            release(button)
            expect(game_state="Menu", menu_state="Main",
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            warp(1)
            dead(new_main)
            dead(button)
            game = hierarchy("game-screen", ["game-title", "game-settings-summary"])
            expect(game_state="Game", game_elapsed_seconds=0.1,
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            frozen = state()
            time.sleep(0.1)
            assert state() == frozen, "wall time advanced the game timer"
            warp(48)
            expect(game_state="Game", game_elapsed_seconds=4.9,
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            warp(1)
            expect(game_state="Game", game_elapsed_seconds=5.0,
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            # Timer expiry queues the screen transition. Entering Menu then
            # queues its independent menu-state transition for one tick later.
            warp(1)
            expect(game_state="Menu", game_elapsed_seconds=5.0,
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            dead(game)
            warp(1)
            expect(game_state="Menu", menu_state="Main", game_elapsed_seconds=5.0,
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            returned = hierarchy("main-menu-screen", [
                "main-menu-title", "new-game-button", "settings-button", "quit-button",
            ])
            assert returned != new_main
            frozen = state()
            time.sleep(0.1)
            assert state() == frozen

            cli("session", "stop", session)
            until(lambda: cli("session", "inspect", session)["state"] == "Ended", "session did not end")
            cli("server", "stop")
            assert server.wait(timeout=15) == 0
            print(json.dumps({"acceptance": "passed", "scene": "game_menu",
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
