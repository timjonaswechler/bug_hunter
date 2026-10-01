"""CLI mesh-picking and image acceptance. Requires an awake desktop.

Build woodpecker with cli and mesh_picking with slice first.
Evidence, including screenshots, remains under target/mesh-picking-*.
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
from ui import read_png


def rotate(q, v):
    """Apply an xyzw unit quaternion to a vector."""
    x, y, z, w = q
    vx, vy, vz = v
    tx, ty, tz = 2 * (y * vz - z * vy), 2 * (z * vx - x * vz), 2 * (x * vy - y * vx)
    return [vx + w * tx + y * tz - z * ty,
            vy + w * ty + z * tx - x * tz,
            vz + w * tz + x * ty - y * tx]


def tick_rotation(q, delta=(0.0, 0.0)):
    """Fixture oracle: world Y/X drag in PreUpdate, then 20 ms of Y spin."""
    def product(a, b):
        x, y, z, w = a
        vx, vy, vz, vw = b
        return [w * vx + x * vw + y * vz - z * vy,
                w * vy - x * vz + y * vw + z * vx,
                w * vz + x * vy - y * vx + z * vw,
                w * vw - x * vx - y * vy - z * vz]

    dx, dy = delta
    yaw = [0.0, math.sin(dx * 0.01), 0.0, math.cos(dx * 0.01)]
    pitch = [math.sin(dy * 0.01), 0.0, 0.0, math.cos(dy * 0.01)]
    spin = [0.0, math.sin(0.005), 0.0, math.cos(0.005)]
    return product(spin, product(pitch, product(yaw, q)))


def rgb_pixels(path):
    width, height, scanlines = read_png(path)
    stride = width * 3
    previous = bytearray(stride)
    image = []
    for row in range(height):
        offset = row * (stride + 1)
        kind = scanlines[offset]
        assert 0 <= kind <= 4, kind
        decoded = bytearray(scanlines[offset + 1:offset + 1 + stride])
        for i in range(stride):
            left = decoded[i - 3] if i >= 3 else 0
            up = previous[i]
            corner = previous[i - 3] if i >= 3 else 0
            if kind == 0:
                predictor = 0
            elif kind == 1:
                predictor = left
            elif kind == 2:
                predictor = up
            elif kind == 3:
                predictor = (left + up) // 2
            else:
                p = left + up - corner
                predictor = min((left, up, corner), key=lambda value: abs(p - value))
            decoded[i] = (decoded[i] + predictor) & 255
        image.append(decoded)
        previous = decoded
    return width, height, image


def run():
    directory = Path(tempfile.mkdtemp(prefix="mesh-picking-", dir=ROOT / "target"))
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

            session = cli("session", "create", "--config", str(ROOT / "tests/fixtures/mesh_picking.toml"))["id"]

            def ready():
                detail = cli("session", "inspect", session)
                assert detail["state"] not in ("Failed", "Ended"), detail
                return detail["state"] == "Ready"

            until(ready, "mesh_picking did not become Ready; build the slice binary first")
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
                        if event["kind"] == "event":
                            assert event["event"]["kind"] not in ("failure", "protocol_error", "ended"), event
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

            meshes = {name: named(name) for name in ["center-cube", "left-sphere", "right-cylinder"]}

            def state(mesh):
                return component(mesh, "mesh_picking::MeshInteractionState")

            def transform(entity):
                return component(entity, "bevy_transform::components::transform::Transform")

            initial = {name: transform(mesh) for name, mesh in meshes.items()}
            for mesh in meshes.values():
                assert state(mesh) == {"last_interaction": "Idle", "drag_events": 0}
            time.sleep(0.1)
            assert {name: transform(mesh) for name, mesh in meshes.items()} == initial

            camera = named("scene-camera")
            assert component(camera, "bevy_camera::camera::Camera")["is_active"] is False

            total_ticks = 0

            def warp(ticks):
                nonlocal total_ticks
                result = command("tick.warp.start", {"ticks": ticks})
                assert result == {"requested_ticks": ticks, "executed_ticks": ticks,
                                  "outcome": "completed"}, result
                total_ticks += ticks

            def expect_yaw(mesh, angle):
                actual = transform(mesh)
                expected = [0.0, math.sin(angle / 2), 0.0, math.cos(angle / 2)]
                assert all(math.isclose(a, b, rel_tol=0, abs_tol=1e-6)
                           for a, b in zip(actual["rotation"], expected)), (actual, expected)
                assert actual["translation"] == initial[next(n for n, e in meshes.items() if e == mesh)]["translation"]
                assert actual["scale"] == [1.0, 1.0, 1.0]

            warp(1)
            assert component(camera, "bevy_camera::camera::Camera")["is_active"] is True
            for mesh in meshes.values():
                expect_yaw(mesh, 0.0)  # First Time update has zero delta.
            warp(10)
            for mesh in meshes.values():
                expect_yaw(mesh, 0.1)  # 200 ms at 0.5 rad/s.

            camera_transform = transform(camera)
            projection = component(camera, "bevy_camera::projection::Projection")
            assert "Perspective" in projection, projection
            perspective = projection["Perspective"]
            target = component(camera, "bevy_camera::camera::Camera")["computed"]["target_info"]
            pixel_width, pixel_height = target["physical_size"]
            scale = target["scale_factor"]
            width, height = pixel_width / scale, pixel_height / scale
            assert width > 0 and height > 0 and scale > 0, target

            def screen_position(mesh):
                world_position = transform(mesh)["translation"]
                delta = [v - c for v, c in zip(world_position, camera_transform["translation"])]
                q = camera_transform["rotation"]
                x, y, z = rotate([-q[0], -q[1], -q[2], q[3]], delta)
                assert z < 0
                tangent = math.tan(perspective["fov"] / 2)
                position = [width / 2 * (1 + x / (-z * tangent * perspective["aspect_ratio"])),
                            height / 2 * (1 - y / (-z * tangent))]
                assert 0 < position[0] < width and 0 < position[1] < height, position
                return position

            positions = {name: screen_position(mesh) for name, mesh in meshes.items()}

            def snapshot():
                return {name: (state(mesh), transform(mesh)) for name, mesh in meshes.items()}

            def input_command(name, arguments):
                frozen = snapshot()
                assert command(name, arguments) is None
                assert snapshot() == frozen, "input ran before an explicit tick"

            screenshots = []

            def capture(name):
                frozen = snapshot()
                path = f"screenshots/{name}.png"
                assert command("screenshot.capture", {"path": path}) == {
                    "path": path, "width": pixel_width, "height": pixel_height, "overwritten": False,
                }
                assert snapshot() == frozen, "screenshot advanced simulation"
                width, height, pixels = rgb_pixels(artifact_dir / path)
                assert (width, height) == (pixel_width, pixel_height)
                screenshots.append(str(artifact_dir / path))
                return pixels

            def patch_color(image, point):
                x, y = (round(value * scale) for value in point)
                colors = [image[row][col * 3:col * 3 + 3]
                          for row in range(y - 4, y + 5) for col in range(x - 4, x + 5)]
                return [sum(color[c] for color in colors) / len(colors) for c in range(3)]

            idle_image = capture("idle")
            for point in positions.values():
                color = patch_color(idle_image, point)
                assert min(color) > 40 and max(color) - min(color) < 35, color
            cube = meshes["center-cube"]
            input_command("input.pointer.move_to", {"position": positions["center-cube"]})
            warp(1)
            assert state(cube) == {"last_interaction": "Hover", "drag_events": 0}
            hover_image = capture("hover")
            hover = patch_color(hover_image, positions["center-cube"])
            assert hover[1] > hover[0] + 15 and hover[2] > hover[0] + 15, hover
            input_command("input.pointer.press", {"button": "left"})
            warp(1)
            assert state(cube) == {"last_interaction": "Press", "drag_events": 0}
            pressed_image = capture("pressed")
            pressed = patch_color(pressed_image, positions["center-cube"])
            assert pressed[0] > pressed[2] + 15 and pressed[1] > pressed[2] + 15, pressed
            input_command("input.pointer.move_by", {"delta": [12.0, 0.0]})
            warp(1)
            assert state(cube) == {"last_interaction": "Drag", "drag_events": 1}
            expect_yaw(cube, 0.37)  # 0.13 timed rotation plus 12 px * 0.02.
            for name in ["left-sphere", "right-cylinder"]:
                expect_yaw(meshes[name], 0.13)
                assert state(meshes[name]) == {"last_interaction": "Idle", "drag_events": 0}
            input_command("input.pointer.release", {"button": "left"})
            warp(1)
            assert state(cube) == {"last_interaction": "Release", "drag_events": 1}
            released_image = capture("released")
            released = patch_color(released_image, positions["center-cube"])
            assert released[1] > released[0] + 15 and released[2] > released[0] + 15, released
            input_command("input.pointer.move_to", {"position": [width - 10, height - 10]})
            warp(1)
            assert state(cube) == {"last_interaction": "Out", "drag_events": 1}
            out_image = capture("out")
            color = patch_color(out_image, positions["center-cube"])
            assert min(color) > 40 and max(color) - min(color) < 35, color
            for name in ["left-sphere", "right-cylinder"]:
                mesh = meshes[name]
                input_command("input.pointer.move_to", {"position": positions[name]})
                warp(1)
                assert state(mesh) == {"last_interaction": "Hover", "drag_events": 0}
                image = capture(f"{name}-hover")
                color = patch_color(image, positions[name])
                assert color[1] > color[0] + 15 and color[2] > color[0] + 15, (name, color)
                # The other meshes must not inherit this object's material.
                for other, point in positions.items():
                    if other != name:
                        neutral = patch_color(image, point)
                        assert min(neutral) > 40 and max(neutral) - min(neutral) < 35, (other, neutral)
                input_command("input.pointer.press", {"button": "left"})
                warp(1)
                assert state(mesh) == {"last_interaction": "Press", "drag_events": 0}
                color = patch_color(capture(f"{name}-pressed"), positions[name])
                assert color[0] > color[2] + 15 and color[1] > color[2] + 15, (name, color)
                input_command("input.pointer.release", {"button": "left"})
                warp(1)
                assert state(mesh) == {"last_interaction": "Release", "drag_events": 0}
                color = patch_color(capture(f"{name}-released"), positions[name])
                assert color[1] > color[0] + 15 and color[2] > color[0] + 15, (name, color)
                input_command("input.pointer.move_to", {"position": [width - 10, height - 10]})
                warp(1)
                assert state(mesh) == {"last_interaction": "Out", "drag_events": 0}
                color = patch_color(capture(f"{name}-out"), positions[name])
                assert min(color) > 40 and max(color) - min(color) < 35, (name, color)
            expect_yaw(cube, 0.47)  # 23 timed increments and the earlier drag.
            for name in ["left-sphere", "right-cylinder"]:
                expect_yaw(meshes[name], 0.23)
            frozen = snapshot()
            time.sleep(0.1)
            assert snapshot() == frozen

            # Extend the existing horizontal cube case with noncommuting X/Y
            # rotations. Picking observers run in PreUpdate, before timed spin
            # in Update; Transform.rotate_* premultiplies world-axis rotations.
            def checked_tick(dragged=None, delta=(0.0, 0.0)):
                before = snapshot()
                warp(1)
                after = snapshot()
                for name in meshes:
                    prior_state, prior_transform = before[name]
                    actual_state, actual_transform = after[name]
                    expected = tick_rotation(prior_transform["rotation"],
                                             delta if name == dragged else (0.0, 0.0))
                    assert all(math.isclose(a, b, rel_tol=0, abs_tol=1e-6)
                               for a, b in zip(actual_transform["rotation"], expected)), {
                                   "mesh": name, "expected": expected, "actual": actual_transform,
                               }
                    assert actual_transform["translation"] == initial[name]["translation"]
                    assert actual_transform["scale"] == initial[name]["scale"]
                    assert actual_state["drag_events"] == prior_state["drag_events"] + int(name == dragged), after
                if dragged:
                    assert after[dragged][0]["last_interaction"] == "Drag", after

            for name, delta in [
                ("center-cube", [0.0, 12.0]),
                ("left-sphere", [12.0, 12.0]),
                ("right-cylinder", [-12.0, 12.0]),
            ]:
                mesh = meshes[name]
                point = screen_position(mesh)
                count = state(mesh)["drag_events"]
                input_command("input.pointer.move_to", {"position": point})
                checked_tick()
                assert state(mesh) == {"last_interaction": "Hover", "drag_events": count}
                input_command("input.pointer.press", {"button": "left"})
                checked_tick()
                assert state(mesh) == {"last_interaction": "Press", "drag_events": count}
                input_command("input.pointer.move_by", {"delta": delta})
                # Capture may neither consume the pending delta nor spin meshes.
                pending_image = capture(f"{name}-drag-pending")
                color = patch_color(pending_image, point)
                assert color[0] > color[2] + 15 and color[1] > color[2] + 15, (name, color)
                checked_tick(name, delta)
                image = capture(f"{name}-dragged")
                color = patch_color(image, point)
                assert color[0] > color[2] + 15 and color[1] > color[2] + 15, (name, color)
                for other, other_point in positions.items():
                    if other != name:
                        neutral = patch_color(image, other_point)
                        assert min(neutral) > 40 and max(neutral) - min(neutral) < 35, (other, neutral)
                checked_tick()  # Holding still must not repeat the drag delta.
                assert state(mesh)["drag_events"] == count + 1
                input_command("input.pointer.release", {"button": "left"})
                checked_tick()
                assert state(mesh) == {"last_interaction": "Release", "drag_events": count + 1}
                input_command("input.pointer.move_to", {"position": [width - 10, height - 10]})
                checked_tick()
                assert state(mesh) == {"last_interaction": "Out", "drag_events": count + 1}
                input_command("input.pointer.move_to", {"position": screen_position(mesh)})
                checked_tick()
                assert state(mesh) == {"last_interaction": "Hover", "drag_events": count + 1}
                input_command("input.pointer.move_to", {"position": [width - 10, height - 10]})
                checked_tick()
                assert state(mesh) == {"last_interaction": "Out", "drag_events": count + 1}

            assert {name: state(mesh)["drag_events"] for name, mesh in meshes.items()} == {
                "center-cube": 2, "left-sphere": 1, "right-cylinder": 1,
            }
            cli("session", "stop", session)
            until(lambda: cli("session", "inspect", session)["state"] == "Ended", "session did not end")
            cli("server", "stop")
            assert server.wait(timeout=15) == 0
            print(json.dumps({"acceptance": "passed", "scene": "mesh_picking",
                              "ticks": total_ticks, "screenshots": screenshots,
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
