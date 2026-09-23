"""Rendered context-menu acceptance through CLI -> server -> session -> Bevy.

Requires a desktop session and the CLI and context_menu binaries built with `slice`.
Run from the repository root: python3 tests/ui.py
"""
import json
import argparse
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import select
import signal
import struct
import subprocess
import sys
import tempfile
import zlib

from slice import CLI, ROOT, until


def read_png(path):
    """Check the complete PNG, including CRCs and decompressible RGB scanlines."""
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    offset, compressed = 8, bytearray()
    width = height = None
    while offset < len(data):
        length = struct.unpack(">I", data[offset:offset + 4])[0]
        kind = data[offset + 4:offset + 8]
        payload = data[offset + 8:offset + 8 + length]
        crc = struct.unpack(">I", data[offset + 8 + length:offset + 12 + length])[0]
        assert zlib.crc32(kind + payload) == crc, (path, kind)
        offset += length + 12
        if kind == b"IHDR":
            width, height, bits, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", payload)
            assert (bits, color, compression, filtering, interlace) == (8, 2, 0, 0, 0)
        elif kind == b"IDAT":
            compressed.extend(payload)
        elif kind == b"IEND":
            assert offset == len(data)
            break
    else:
        raise AssertionError("missing PNG end")
    pixels = zlib.decompress(compressed)
    assert len(pixels) == height * (1 + width * 3)
    return width, height, pixels


def run(capture_dir=None):
    with tempfile.TemporaryDirectory(prefix="ui-", dir=ROOT / "target") as directory:
        directory = Path(directory)
        with (directory / "server.log").open("w+") as log:
            server = subprocess.Popen(
                [str(CLI), "--address", "127.0.0.1:0", "server", "start",
                 "--artifact-dir", str(directory / "artifacts"), "--shutdown-seconds", "10"],
                cwd=ROOT, stdout=subprocess.PIPE, stderr=log, text=True,
            )
            try:
                assert select.select([server.stdout], [], [], 10)[0], "no server Ready"
                address = json.loads(server.stdout.readline())["address"]

                def cli(*args):
                    result = subprocess.run(
                        [str(CLI), "--address", address, *args],
                        cwd=ROOT, capture_output=True, text=True, timeout=40,
                    )
                    assert result.returncode == 0, (args, result.stdout, result.stderr)
                    return json.loads(result.stdout) if result.stdout.strip() else None

                session = cli("session", "create", "--config",
                              str(ROOT / "tests/fixtures/context_menu.toml"))["id"]

                def ready(target=session):
                    detail = cli("session", "inspect", target)
                    assert detail["state"] not in ("Failed", "Ended"), detail
                    return detail["state"] == "Ready"

                until(ready)

                def command(name, arguments, rejection=None, target=session):
                    pending = cli("session", "submit", target, "--command",
                                  json.dumps({"command": name, "arguments": arguments}))

                    def completed():
                        activity = cli("session", "poll", target)
                        assert activity["kind"] == "activity", activity
                        outcomes = [entry["event"] for entry in activity["entries"]
                                    if entry["event"].get("request_id") == pending["request_id"]
                                    and entry["event"]["kind"] != "pending"]
                        return outcomes[0] if outcomes else None

                    result = until(completed)
                    if rejection:
                        assert result["kind"] == "rejected", result
                        assert result["error"]["code"] == rejection, result
                        return
                    assert result["kind"] == "completed", result
                    return result["output"]

                def inspect(handle=None, projection=None, target=session):
                    return command("inspect.query", {
                        "source": "entities", "entity": handle, "with": [], "without": [],
                        "projection": projection or {"kind": "summary"},
                    }, target=target)["items"]

                def named(name, target=session):
                    found = [item for item in inspect(target=target) if item["result"]["name"] == name]
                    assert len(found) == 1, (name, found)
                    return found[0]["entity"]

                def components(handle, paths, target=session):
                    values = inspect(handle, {"kind": "components", "selection": {
                        "kind": "listed", "type_paths": paths,
                    }}, target=target)[0]["result"]["components"]
                    assert all(item["value"]["status"] == "readable" for item in values), values
                    return [item["value"]["value"] for item in values]

                background = named("background")
                button = named("button")
                backgrounds = {session: background}

                def state(target=session):
                    return components(backgrounds[target], ["context_menu::SessionState"], target)[0]

                def warp(ticks, target=session):
                    result = command("tick.warp.start", {"ticks": ticks}, target=target)
                    assert result == {"requested_ticks": ticks, "executed_ticks": ticks,
                                      "outcome": "completed"}, result

                def capture(path, overwritten=False, target=session):
                    before = state(target)
                    result = command("screenshot.capture", {"path": path}, target=target)
                    assert result == {"path": path, "width": 640, "height": 360,
                                      "overwritten": overwritten}, result
                    root = Path(cli("session", "inspect", target)["artifact_dir"])
                    width, height, pixels = read_png(root / path)
                    assert (width, height) == (640, 360)
                    assert state(target) == before, "screenshot advanced simulation or time"
                    if capture_dir is not None:
                        destination = capture_dir / target / path
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        destination.write_bytes((root / path).read_bytes())
                    return pixels

                # Layout runs only in explicit ticks. Inspect the real layout, not fixture coordinates.
                recording_path = "recordings/ui.jsonl"
                assert command("recording.start", {"path": recording_path}) == {"path": recording_path}
                warp(3)
                closed_pixels = capture("screenshots/current.png")
                assert capture("screenshots/current.png", overwritten=True) == closed_pixels
                frozen = state()
                with ThreadPoolExecutor(max_workers=3) as pool:
                    results = list(pool.map(
                        lambda _: command("screenshot.capture", {"path": "screenshots/concurrent.png"}),
                        range(3)))
                assert sorted(result["overwritten"] for result in results) == [False, True, True]
                assert state() == frozen
                for path in ["../escape.png", "/absolute.png", "a//b.png", "a/./b.png", "a\\b.png", "a.jpg"]:
                    command("screenshot.capture", {"path": path}, rejection="invalid_screenshot_path")
                artifact_root = Path(cli("session", "inspect", session)["artifact_dir"])
                (artifact_root / "escape").symlink_to(directory, target_is_directory=True)
                command("screenshot.capture", {"path": "escape/outside.png"},
                        rejection="invalid_screenshot_path")
                assert not (directory / "outside.png").exists()
                before = state()
                command("input.text.input", {"text": "not focused"},
                        rejection="text_focus_unavailable")
                command("input.text.input", {"text": "🦜" * 4097},
                        rejection="text_too_large")
                command("input.keyboard.press", {"key": "KeyA"}, rejection="invalid_key")
                command("input.keyboard.release", {"key": "a"}, rejection="key_not_pressed")
                assert command("input.keyboard.press", {"key": "a"}) is None
                command("input.keyboard.press", {"key": "a"}, rejection="key_already_pressed")
                assert state() == before, "keyboard input was processed without a tick"
                warp(1)
                assert state()["key_a_held"] and state()["key_a_presses"] == 1
                warp(3)
                assert state()["key_a_held"] and state()["key_a_presses"] == 1
                assert state()["key_a_releases"] == 0 and state()["text"] == ""
                transform = components(button, ["bevy_ui::ui_transform::UiGlobalTransform"])[0]
                # Bevy's reflected Affine2 is serialized as its six column-major scalars.
                position = transform[-2:]
                assert not state()["menu_open"]
                command("input.pointer.move_by", {"delta": [0.0, 0.0]},
                        rejection="pointer_location_unavailable")
                command("input.pointer.press", {"button": "other"},
                        rejection="invalid_pointer_button")
                assert command("input.pointer.move_to", {"position": position}) is None
                command("input.pointer.move_to", {"position": [-1.0, 0.0]},
                        rejection="pointer_position_out_of_bounds")
                assert command("input.pointer.move_by", {"delta": [1.0, 0.0]}) is None
                assert command("input.pointer.move_by", {"delta": [-1.0, 0.0]}) is None
                assert command("input.pointer.scroll", {"delta": [0.0, 0.0]}) is None
                assert command("input.pointer.press", {"button": "left"}) is None
                command("input.pointer.press", {"button": "left"},
                        rejection="pointer_button_already_pressed")
                assert not state()["menu_open"], "input was processed without a tick"
                warp(1)
                assert state()["menu_open"], {
                    "message": "pointer press did not open the context menu",
                    "position": position, "transform": transform,
                    "state": state(),
                }
                menu = named("context-menu")
                assert components(button, ["bevy_ui::focus::Interaction"])[0] == "Pressed"
                assert capture("screenshots/menu.png") != closed_pixels

                # A second real process can press the same button independently,
                # at another position, while the first session keeps its button held.
                second = cli("session", "create", "--config",
                             str(ROOT / "tests/fixtures/context_menu.toml"))["id"]
                until(lambda: ready(second))
                backgrounds[second] = named("background", second)
                assert not state(second)["key_a_held"]
                command("input.keyboard.press", {"key": "a"}, target=second)
                assert not state(second)["key_a_held"]
                warp(3, second)
                assert capture("screenshots/current.png", target=second) == closed_pixels
                assert state(second)["key_a_held"] and state(second)["key_a_presses"] == 1
                assert state()["key_a_held"] and state()["key_a_presses"] == 1
                second_button = named("button", second)
                second_position = components(
                    second_button, ["bevy_ui::ui_transform::UiGlobalTransform"], second
                )[0][-2:]
                second_position[0] += 10.0
                command("input.pointer.move_to", {"position": second_position}, target=second)
                command("input.pointer.press", {"button": "left"}, target=second)
                assert not state(second)["menu_open"]
                warp(1, second)
                assert state(second)["menu_open"]
                assert state()["menu_open"]
                command("input.pointer.press", {"button": "left"},
                        rejection="pointer_button_already_pressed")
                command("input.pointer.release", {"button": "left"}, target=second)
                warp(1, second)
                command("input.pointer.press", {"button": "left"},
                        rejection="pointer_button_already_pressed")
                assert command("input.pointer.release", {"button": "left"}) is None
                warp(1)
                assert state()["menu_open"]

                command("input.keyboard.release", {"key": "a"})
                assert state()["key_a_held"]
                warp(1)
                assert not state()["key_a_held"] and state()["key_a_releases"] == 1
                assert state(second)["key_a_held"] and state(second)["key_a_releases"] == 0
                command("input.keyboard.release", {"key": "a"}, target=second)
                warp(2, second)
                assert not state(second)["key_a_held"] and state(second)["key_a_releases"] == 1

                # Close through the real background observer, then verify the old handle is dead.
                command("input.pointer.move_to", {"position": [1.0, 1.0]})
                command("input.pointer.press", {"button": "left"})
                assert state()["menu_open"]
                warp(1)
                assert not state()["menu_open"]
                command("inspect.query", {
                    "source": "entities", "entity": menu, "with": [], "without": [],
                    "projection": {"kind": "summary"},
                }, rejection="entity_not_found")
                command("input.pointer.release", {"button": "left"})
                warp(1)

                # Focus real EditableText widgets with each session's virtual pointer.
                # Choose a point left of the open menu in the second session.
                for target in [session, second]:
                    field = named("text-input", target)
                    position = components(
                        field, ["bevy_ui::ui_transform::UiGlobalTransform"], target
                    )[0][-2:]
                    position[0] -= 80.0
                    command("input.pointer.move_to", {"position": position}, target=target)
                    command("input.pointer.press", {"button": "left"}, target=target)
                    warp(1, target)
                    command("input.pointer.release", {"button": "left"}, target=target)
                    warp(1, target)

                # Verify visible text, not just the mirrored application state.
                empty_field_pixels = capture("screenshots/text-empty.png")
                assert command("input.text.input", {"text": "hallo"}) is None
                assert state()["text"] == ""
                assert capture("screenshots/text-pending.png") == empty_field_pixels
                warp(1)
                assert state()["text"] == "hallo", state()
                assert state(second)["text"] == ""
                assert capture("screenshots/text-hallo.png") != empty_field_pixels

                assert command("input.text.input", {"text": "Grüße "}) is None
                assert command("input.text.input", {"text": "🦜"}) is None
                assert command("input.text.input", {"text": ""}) is None
                command("input.text.input", {"text": "東京"}, target=second)
                assert state()["text"] == "hallo" and state(second)["text"] == ""
                # Keyboard letters must not generate a second, implicit text input.
                command("input.keyboard.press", {"key": "a"})
                command("input.keyboard.release", {"key": "a"})
                warp(1)
                assert state()["text"] == "halloGrüße 🦜", state()
                capture("screenshots/text-unicode.png")
                assert state(second)["text"] == ""
                warp(1, second)
                assert state(second)["text"] == "東京", state(second)
                capture("screenshots/text-japanese.png", target=second)
                warp(1)
                assert state()["text"] == "halloGrüße 🦜"
                frozen = state()
                recorded = command("recording.stop", {})
                assert recorded["path"] == recording_path
                assert state() == frozen, "recording stop advanced simulation or time"
                lines = [json.loads(line) for line in (artifact_root / recording_path).read_text().splitlines()]
                assert lines[0] == {"type": "recording_started", "format_version": 1}
                assert lines[-1] == {"type": "recording_ended", "outcome": "stopped",
                                     "recorded_commands": recorded["recorded_commands"]}
                entries = lines[1:-1]
                assert len(entries) == recorded["recorded_commands"]
                assert all(entry["type"] == "command" and "request_id" not in entry for entry in entries)
                assert all(not entry["command"].startswith("recording.") for entry in entries)
                assert any(entry["command"] == "input.text.input" and entry["arguments"]["text"] == "🦜"
                           for entry in entries)
                assert not any(entry["command"] == "input.text.input" and entry["arguments"]["text"] == "東京"
                               for entry in entries), "recording mixed sessions"
                assert any(entry["command"] == "screenshot.capture"
                           and entry["outcome"]["status"] == "completed" for entry in entries)
                assert any(entry["outcome"]["status"] == "rejected" for entry in entries)
                assert any(entry["command"] == "tick.warp.start" for entry in entries)
                # Replay the actual UI recording in the current world. Old Inspect results,
                # input rejections and screenshot overwrite flags are not expectations.
                second_before = state(second)
                ticks = sum(entry["outcome"]["output"]["executed_ticks"] for entry in entries
                            if entry["command"] == "tick.warp.start"
                            and entry["outcome"]["status"] == "completed")
                assert command("replay.start", {"path": recording_path}) == {
                    "outcome": {"kind": "completed"}}
                assert state()["ticks"] == frozen["ticks"] + ticks
                assert state(second) == second_before, "replay affected the other session"
                for entry in entries:
                    if entry["command"] == "screenshot.capture" and entry["outcome"]["status"] == "completed":
                        width, height, _ = read_png(artifact_root / entry["arguments"]["path"])
                        assert (width, height) == (640, 360)
                cli("session", "stop", session)
                assert state(second)["menu_open"]
                cli("session", "stop", second)
                cli("server", "stop")
                assert server.wait(timeout=15) == 0
                print(json.dumps({"acceptance": "passed", "scene": "context_menu", "sessions": 2,
                                  "bevy": "0.19.1"}))
            finally:
                if server.poll() is None:
                    server.send_signal(signal.SIGINT)
                    try:
                        server.wait(timeout=15)
                    except subprocess.TimeoutExpired:
                        server.send_signal(signal.SIGINT)
                        server.wait(timeout=10)
                if sys.exc_info()[0] is not None:
                    log.seek(0)
                    print(log.read(), file=sys.stderr)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--capture-dir", type=Path, help="retain PNGs for visual inspection")
    run(parser.parse_args().capture_dir)
