"""CLI blend-mode acceptance. Requires an available desktop.

Build woodpecker with cli and blend_modes with slice first.
Retains logs, CLI responses and screenshots under target/blend-modes-*.
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
from mesh_picking import rgb_pixels, rotate


def run():
    directory = Path(tempfile.mkdtemp(prefix="blend-modes-", dir=ROOT / "target"))
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

            session = cli("session", "create", "--config", str(ROOT / "tests/fixtures/blend_modes.toml"))["id"]

            def ready():
                detail = cli("session", "inspect", session)
                assert detail["state"] not in ("Failed", "Ended"), detail
                return detail["state"] == "Ready"

            until(ready, "blend_modes did not become Ready; build the slice binary first")
            artifact_dir = Path(cli("session", "inspect", session)["artifact_dir"])
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

            def component(handle, path):
                values = inspect(handle, {"kind": "components", "selection": {
                    "kind": "listed", "type_paths": [path],
                }})[0]["result"]["components"]
                assert len(values) == 1 and values[0]["value"]["status"] == "readable", values
                return values[0]["value"]["value"]

            camera = named("camera")

            def state():
                return component(camera, "blend_modes::SceneState")

            initial = state()
            assert initial["seed"] == 0x5EEDB1E5 and initial["color_changes"] == 0, initial
            assert not initial["hdr"] and not initial["unlit"] and initial["camera_angle"] == 0, initial
            assert math.isclose(initial["alpha"], 0.9, abs_tol=1e-6), initial
            assert len(initial["colors"]) == 5 and all(
                all(math.isclose(a, b, abs_tol=1e-6) for a, b in zip(color, [0.9, 0.2, 0.3, 0.9]))
                for color in initial["colors"]
            ), initial
            time.sleep(0.1)
            assert state() == initial

            def transform(entity):
                return component(entity, "bevy_transform::components::transform::Transform")

            def snapshot():
                return state(), transform(camera)

            def warp(ticks):
                assert command("tick.warp.start", {"ticks": ticks}) == {
                    "requested_ticks": ticks, "executed_ticks": ticks, "outcome": "completed",
                }

            def key(action, token):
                before = snapshot()
                assert command(f"input.keyboard.{action}", {"key": token}) is None
                assert snapshot() == before, "key was processed without a tick"

            def expect(alpha, angle, hdr=False, unlit=False, changes=0):
                actual = state()
                assert math.isclose(actual["alpha"], alpha, abs_tol=2e-6), actual
                assert math.isclose(actual["camera_angle"], angle, abs_tol=2e-6), actual
                assert (actual["hdr"], actual["unlit"], actual["color_changes"]) == (hdr, unlit, changes), actual
                assert actual["seed"] == 0x5EEDB1E5
                assert all(math.isclose(color[3], alpha, abs_tol=2e-6) for color in actual["colors"]), actual
                position = transform(camera)["translation"]
                expected_position = [10 * math.sin(angle), 2.5, 10 * math.cos(angle)]
                assert all(math.isclose(a, b, rel_tol=0, abs_tol=5e-5)
                           for a, b in zip(position, expected_position)), position
                paths = [item["type_path"] for item in inspect(
                    camera, {"kind": "component_names"},
                )[0]["result"]["components"]]
                assert ("bevy_camera::components::Hdr" in paths) == hdr, paths
                return actual

            spheres = {mode: named(f"sphere-{mode}") for mode in [
                "opaque", "blend", "premultiplied", "add", "multiply",
            ]}
            identities = {
                mode: component(entity, "blend_modes::ObservedMaterialHandle")["asset_id"]
                for mode, entity in spheres.items()
            }
            assert len(set(identities.values())) == 5, identities
            warp(1)
            expect(0.9, 0)
            key("press", "arrow_left")
            key("press", "arrow_down")
            warp(5)
            expect(0.8, 0.1)
            warp(5)
            expect(0.7, 0.2)
            key("release", "arrow_left")
            key("release", "arrow_down")
            warp(1)
            expect(0.7, 0.2)
            key("press", "arrow_right")
            key("press", "arrow_up")
            warp(10)
            expect(0.9, 0)
            key("release", "arrow_right")
            key("release", "arrow_up")
            warp(1)
            expect(0.9, 0)
            key("press", "h")
            warp(1)
            expect(0.9, 0, hdr=True)
            warp(3)
            expect(0.9, 0, hdr=True)
            key("release", "h")
            warp(1)
            key("press", "h")
            warp(1)
            expect(0.9, 0)
            key("release", "h")
            warp(1)
            key("press", "space")
            warp(1)
            expect(0.9, 0, unlit=True)
            warp(3)
            expect(0.9, 0, unlit=True)
            key("release", "space")
            warp(1)
            key("press", "arrow_up")
            warp(10)
            expect(1.0, 0, unlit=True)
            key("release", "arrow_up")
            warp(1)

            view = component(camera, "bevy_camera::camera::Camera")["computed"]["target_info"]
            pixel_width, pixel_height = view["physical_size"]
            scale = view["scale_factor"]
            perspective = component(camera, "bevy_camera::projection::Projection")["Perspective"]
            camera_transform = transform(camera)

            def pixel_position(entity):
                delta = [a - b for a, b in zip(transform(entity)["translation"], camera_transform["translation"])]
                x, y, z, w = camera_transform["rotation"]
                vx, vy, vz = rotate([-x, -y, -z, w], delta)
                assert vz < 0
                tangent = math.tan(perspective["fov"] / 2)
                return [pixel_width / 2 * (1 + vx / (-vz * tangent * perspective["aspect_ratio"])),
                        pixel_height / 2 * (1 - vy / (-vz * tangent))]

            positions = {mode: pixel_position(entity) for mode, entity in spheres.items()}
            assert pixel_width > 0 and pixel_height > 0 and scale > 0, view

            def capture(name):
                before = snapshot()
                path = f"screenshots/{name}.png"
                result = command("screenshot.capture", {"path": path})
                assert result == {"path": path, "width": pixel_width, "height": pixel_height,
                                  "overwritten": False}, result
                assert snapshot() == before, "screenshot advanced simulation"
                w, h, pixels = rgb_pixels(artifact_dir / path)
                assert (w, h) == (pixel_width, pixel_height)
                assert any(any(row) for row in pixels), "all-black screenshot"
                return pixels

            def patch(image, mode):
                x, y = map(round, positions[mode])
                assert 4 <= x < pixel_width - 4 and 4 <= y < pixel_height - 4
                return [image[row][col * 3:col * 3 + 3]
                        for row in range(y - 4, y + 5) for col in range(x - 4, x + 5)]

            def mean_color(image, mode):
                pixels = patch(image, mode)
                return [sum(pixel[c] for pixel in pixels) / len(pixels) for c in range(3)]

            full = capture("unlit-alpha-one")
            for mode in ["opaque", "blend", "premultiplied"]:
                color = mean_color(full, mode)
                assert color[0] > color[1] + 50 and color[0] > color[2] + 50, (mode, color)
            key("press", "arrow_down")
            warp(60)
            expect(0.0, 0, unlit=True)
            key("release", "arrow_down")
            warp(1)
            zero = capture("unlit-alpha-zero")
            # Opaque ignores alpha. Blend/Add/Multiply at alpha zero expose the
            # achromatic checkerboard; Premultiplied keeps this fixture's RGB,
            # since it deliberately supplies the same non-premultiplied color.
            assert patch(full, "opaque") == patch(zero, "opaque")
            for mode in ["blend", "add", "multiply"]:
                color = mean_color(zero, mode)
                assert max(color) - min(color) < 10, (mode, color)
                assert patch(full, mode) != patch(zero, mode), mode
            color = mean_color(zero, "premultiplied")
            assert color[0] > color[1] + 30, color
            key("press", "arrow_up")
            warp(60)
            expect(1.0, 0, unlit=True)
            key("release", "arrow_up")
            warp(1)
            restored = capture("unlit-restored")
            for mode in spheres:
                assert patch(full, mode) == patch(restored, mode), mode

            key("press", "space")
            warp(1)
            expect(1.0, 0)
            lit = capture("lit")
            assert patch(lit, "opaque") != patch(full, "opaque")
            warp(3)
            expect(1.0, 0)
            key("release", "space")
            warp(1)
            key("press", "space")
            warp(1)
            expect(1.0, 0, unlit=True)
            key("release", "space")
            warp(1)
            key("press", "h")
            warp(1)
            expect(1.0, 0, hdr=True, unlit=True)
            hdr_image = capture("hdr")
            # An unlit color in the LDR range need not change under HDR. Check
            # that it still renders, while expect() verifies the actual Hdr component.
            hdr_color = mean_color(hdr_image, "opaque")
            assert hdr_color[0] > hdr_color[1] + 50 and hdr_color[0] > hdr_color[2] + 50, hdr_color
            key("release", "h")
            warp(1)
            key("press", "h")
            warp(1)
            expect(1.0, 0, unlit=True)
            key("release", "h")
            warp(1)

            sequence = []
            for change in [1, 2]:
                key("press", "c")
                warp(1)
                colored = expect(1.0, 0, unlit=True, changes=change)
                sequence.append(colored["colors"])
                assert all(0 <= value <= 1 for color in colored["colors"] for value in color)
                assert len({tuple(color) for color in colored["colors"]}) == 5
                if change == 2:
                    assert sequence[0] != sequence[1]
                warp(3)
                assert expect(1.0, 0, unlit=True, changes=change)["colors"] == sequence[-1]
                key("release", "c")
                warp(1)
            assert {
                mode: component(entity, "blend_modes::ObservedMaterialHandle")["asset_id"]
                for mode, entity in spheres.items()
            } == identities, "color updates replaced material identities"
            changed_image = capture("changed-colors")
            assert patch(changed_image, "opaque") != patch(full, "opaque")
            before = snapshot()
            time.sleep(0.1)
            assert snapshot() == before
            cli("session", "stop", session)
            until(lambda: cli("session", "inspect", session)["state"] == "Ended", "first session did not end")

            # A fresh process with the same seed must reproduce both color
            # changes, despite a different prior input/tick history. Compare
            # through the CLI, not by duplicating the application's RNG.
            session = cli("session", "create", "--config", str(ROOT / "tests/fixtures/blend_modes.toml"))["id"]
            until(ready, "second blend_modes session did not become Ready")
            cursor = None
            camera = named("camera")
            assert state() == initial
            key("press", "arrow_up")
            warp(11)
            expect(1.0, 0)
            key("release", "arrow_up")
            warp(1)
            for change in [1, 2]:
                key("press", "c")
                warp(1)
                assert expect(1.0, 0, changes=change)["colors"] == sequence[change - 1]
                key("release", "c")
                warp(1)

            cli("session", "stop", session)
            until(lambda: cli("session", "inspect", session)["state"] == "Ended", "session did not end")
            cli("server", "stop")
            assert server.wait(timeout=15) == 0
            print(json.dumps({"acceptance": "passed", "scene": "blend_modes",
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
