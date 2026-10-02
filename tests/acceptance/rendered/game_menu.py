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

from tests.support.runtime import ROOT, cli_call, spawn_server, wait_until as until
from tests.support.images import rgb_pixels


def run():
    directory = Path(tempfile.mkdtemp(prefix="game-menu-", dir=ROOT / "target"))
    print(f"Evidence: {directory}", flush=True)
    with (directory / "server.log").open("w") as log, (directory / "commands.jsonl").open("w") as evidence:
        server = spawn_server(directory, log)
        try:
            assert select.select([server.stdout], [], [], 10)[0], "server did not become Ready"
            address = json.loads(server.stdout.readline())["address"]

            def cli(*args):
                return cli_call(address, *args, journal=evidence)

            session = cli("session", "create", "--config", str(ROOT / "tests/fixtures/game_menu.toml"))["id"]

            def ready(target=None):
                detail = cli("session", "inspect", target or session)
                assert detail["state"] not in ("Failed", "Ended"), detail
                return detail["state"] == "Ready"

            until(ready, "game_menu did not become Ready; build the slice binary first")
            cursors = {}

            def command(name, arguments, rejection=None, target=None):
                target = target or session
                pending = cli("session", "submit", target, "--command",
                              json.dumps({"command": name, "arguments": arguments}))
                assert pending["kind"] == "pending" and pending["command"] == name, pending

                def completed():
                    args = ["session", "poll", target, "--wait-ms", "100"]
                    if target in cursors:
                        args += ["--cursor", json.dumps(cursors[target])]
                    activity = cli(*args)
                    assert activity["kind"] == "activity", activity
                    cursors[target] = activity["cursor"]
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

            artifact_root = Path(cli("session", "inspect", session)["artifact_dir"])
            screenshots = []

            def capture(name, background):
                before = state()
                path = f"screenshots/{name}.png"
                result = command("screenshot.capture", {"path": path})
                assert result == {"path": path, "width": 800, "height": 600,
                                  "overwritten": False}, result
                width, height, pixels = rgb_pixels(artifact_root / path)
                assert (width, height) == (800, 600)
                # Known full-screen fixture background, away from text/buttons.
                sample = tuple(pixels[10][30:33])
                assert all(abs(a - b) <= 2 for a, b in zip(sample, background)), sample
                assert state() == before, "capture advanced scene state or timers"
                screenshots.append(str(artifact_root / path))
                return pixels

            # First Time update initializes delta to zero. Timer completion in
            # Update requests a transition; it does not apply it in that tick.
            warp(1)
            expect()
            splash = named("splash-screen")
            capture("splash", (0, 0, 128))
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

            capture("main", (220, 20, 60))
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
            capture("pointer-game", (139, 0, 0))
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

            # Keyboard edges queue a transition in Update; the next explicit
            # tick applies it. Capture while pending must not consume the edge.
            def shortcut(key, name, background):
                before = state()
                assert command("input.keyboard.press", {"key": key}) is None
                assert state() == before, "queued shortcut advanced the scene"
                capture(f"{name}-pending", background)
                warp(1)
                assert state() == before, "shortcut applied a transition too early"
                assert command("input.keyboard.release", {"key": key}) is None
                assert state() == before, "queued key release advanced the scene"
                warp(1)

            old_settings_button = named("settings-button")
            shortcut("s", "settings", (220, 20, 60))
            expect(game_state="Menu", menu_state="Settings", game_elapsed_seconds=5.0,
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            dead(returned)
            dead(old_settings_button)
            settings = hierarchy("settings-menu-screen", [
                "settings-title", "display-settings-button", "sound-settings-button", "settings-back-button",
            ])
            back_button = named("settings-back-button")
            settings_pixels = capture("keyboard-settings", (220, 20, 60))
            shortcut("escape", "main", (220, 20, 60))
            expect(game_state="Menu", menu_state="Main", game_elapsed_seconds=5.0,
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            dead(settings)
            dead(back_button)
            keyboard_main = hierarchy("main-menu-screen", [
                "main-menu-title", "new-game-button", "settings-button", "quit-button",
            ])
            assert capture("keyboard-main", (220, 20, 60)) != settings_pixels
            play_button = named("new-game-button")
            shortcut("n", "game", (220, 20, 60))
            expect(game_state="Game", game_elapsed_seconds=0.1,
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            dead(keyboard_main)
            dead(play_button)
            keyboard_game = hierarchy("game-screen", ["game-title", "game-settings-summary"])
            capture("keyboard-game", (139, 0, 0))
            warp(51)
            expect(game_state="Menu", menu_state="Main", game_elapsed_seconds=5.0,
                   splash_elapsed_seconds=1.0, display_quality="High", volume=3)
            dead(keyboard_game)

            # Start the isolation witness only after the first session's
            # captures; a second window may obscure its render surface.
            second = cli("session", "create", "--config",
                         str(ROOT / "tests/fixtures/game_menu.toml"))["id"]
            until(lambda: ready(second), "second game_menu did not become Ready")

            def second_state():
                items = command("inspect.query", {
                    "source": "entities", "entity": None,
                    "with": ["game_menu::SessionObservation"], "without": [],
                    "projection": {"kind": "components", "selection": {
                        "kind": "listed", "type_paths": ["game_menu::SessionObservation"],
                    }},
                }, target=second)["items"]
                assert len(items) == 1, items
                value = items[0]["result"]["components"][0]["value"]
                assert value["status"] == "readable", value
                return value["value"]

            assert second_state() == initial
            # Quit is a natural application exit, not session management stop.
            press("quit-button")
            quit_pending = cli("session", "submit", session, "--command", json.dumps({
                "command": "tick.warp.start", "arguments": {"ticks": 1},
            }))
            assert quit_pending["kind"] == "pending", quit_pending
            until(lambda: cli("session", "inspect", session)["state"] == "Failed",
                  "Quit did not end the session as an unexpected process exit")
            assert cli("session", "inspect", session)["error"]["code"] == "ended"
            quit_events = []

            def quit_observed():
                activity = cli("session", "poll", session, "--wait-ms", "100",
                               "--cursor", json.dumps(cursors[session]))
                assert activity["kind"] == "activity", activity
                cursors[session] = activity["cursor"]
                quit_events.extend(entry["event"] for entry in activity["entries"])
                return (any(event["kind"] == "report" for event in quit_events)
                        and any(event["kind"] == "event"
                                and event["event"]["kind"] == "ended" for event in quit_events)
                        and any(event.get("request_id") == quit_pending["request_id"]
                                and event["kind"] != "pending" for event in quit_events))

            until(quit_observed, "missing Quit outcome, process-end event or local report")
            outcomes = [event for event in quit_events
                        if event.get("request_id") == quit_pending["request_id"]
                        and event["kind"] != "pending"]
            # This single-tick warp finishes before the runner handles AppExit.
            assert len(outcomes) == 1, outcomes
            assert outcomes[0]["kind"] == "completed", outcomes
            assert outcomes[0]["command"] == "tick.warp.start", outcomes
            assert outcomes[0]["output"] == {
                "requested_ticks": 1, "executed_ticks": 1, "outcome": "completed",
            }, outcomes
            failures = [event["event"]["failure"] for event in quit_events
                        if event["kind"] == "event" and event["event"]["kind"] == "failure"]
            assert len(failures) == 1, failures
            assert failures[0]["origin"] == {"kind": "process_exit", "status": "exit status: 0"}, failures
            ended = [event["event"]["reason"] for event in quit_events
                     if event["kind"] == "event" and event["event"]["kind"] == "ended"]
            assert ended == [failures[0]["origin"]], ended
            reports = [event for event in quit_events if event["kind"] == "report"]
            assert len(reports) == 1, reports
            report = reports[0]
            assert report["report"]["failure"] == failures[0], report
            assert report["result"]["status"] == "submitted", report
            outcome = report["result"]["outcome"]
            assert outcome["kind"] == "created" and outcome["reference"]["kind"] == "file", outcome
            report_path = artifact_root / outcome["reference"]["reference"]["path"]
            assert "process exited unexpectedly" in report_path.read_text(), report_path
            assert cli("session", "inspect", second)["state"] == "Ready"
            assert second_state() == initial, "Quit in another session advanced the witness"
            assert command("tick.warp.start", {"ticks": 2}, target=second) == {
                "requested_ticks": 2, "executed_ticks": 2, "outcome": "completed",
            }
            witness = second_state()
            assert math.isclose(witness["splash_elapsed_seconds"], 0.1, abs_tol=1e-6), witness
            assert {**witness, "splash_elapsed_seconds": 0.0} == initial, witness
            cli("session", "stop", second)
            until(lambda: cli("session", "inspect", second)["state"] == "Ended",
                  "second session did not end")
            cli("server", "stop")
            # The server remains usable, but reports its failed session on exit.
            assert server.wait(timeout=15) == 1
            assert "shutdown_incomplete" in (directory / "server.log").read_text()
            print(json.dumps({"acceptance": "passed", "scene": "game_menu",
                              "screenshots": screenshots, "quit_report": str(report_path),
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
