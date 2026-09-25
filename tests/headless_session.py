"""Opt-in GPU acceptance for CLI -> Session -> fixed headless 2D PNG.

Build first; execute only after separate GPU approval:
    WOODPECKER_RUN_GPU=1 python3 tests/headless_session.py
"""
import json
import os
from pathlib import Path
import select
import signal
import struct
import subprocess
import tempfile
import zlib

from slice import CLI as DEFAULT_CLI, ROOT, until

CLI = Path(os.environ.get("WOODPECKER_CLI", DEFAULT_CLI))


def read_rgb_png(path):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    offset = 8
    compressed = bytearray()
    width = height = None
    while offset < len(data):
        length = struct.unpack(">I", data[offset:offset + 4])[0]
        kind = data[offset + 4:offset + 8]
        payload = data[offset + 8:offset + 8 + length]
        crc = struct.unpack(">I", data[offset + 8 + length:offset + 12 + length])[0]
        assert zlib.crc32(kind + payload) == crc, (path, kind)
        offset += length + 12
        if kind == b"IHDR":
            width, height, bits, color, compression, filtering, interlace = struct.unpack(
                ">IIBBBBB", payload
            )
            assert (bits, color, compression, filtering, interlace) == (8, 2, 0, 0, 0)
        elif kind == b"IDAT":
            compressed.extend(payload)
        elif kind == b"IEND":
            assert offset == len(data)
            break
    assert width is not None and height is not None
    encoded = zlib.decompress(compressed)
    stride = width * 3
    rows = []
    previous = bytearray(stride)
    cursor = 0
    for _ in range(height):
        filter_kind = encoded[cursor]
        source = encoded[cursor + 1:cursor + 1 + stride]
        cursor += stride + 1
        row = bytearray(stride)
        for index, value in enumerate(source):
            left = row[index - 3] if index >= 3 else 0
            above = previous[index]
            upper_left = previous[index - 3] if index >= 3 else 0
            if filter_kind == 0:
                predictor = 0
            elif filter_kind == 1:
                predictor = left
            elif filter_kind == 2:
                predictor = above
            elif filter_kind == 3:
                predictor = (left + above) // 2
            elif filter_kind == 4:
                estimate = left + above - upper_left
                distances = (abs(estimate - left), abs(estimate - above), abs(estimate - upper_left))
                predictor = (left, above, upper_left)[distances.index(min(distances))]
            else:
                raise AssertionError((path, "unknown PNG filter", filter_kind))
            row[index] = (value + predictor) & 0xFF
        rows.append(bytes(row))
        previous = row
    assert cursor == len(encoded)
    return width, height, rows


def run():
    if os.environ.get("WOODPECKER_RUN_GPU") != "1":
        raise SystemExit("GPU acceptance is opt-in; set WOODPECKER_RUN_GPU=1 after approval")
    directory = Path(tempfile.mkdtemp(prefix="headless-session-", dir=ROOT / "target"))
    print(json.dumps({"evidence_dir": str(directory)}, separators=(",", ":")), flush=True)
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

            session = cli(
                "session", "create", "--config",
                str(ROOT / "tests/fixtures/headless_session.toml"),
            )["id"]

            def ready():
                detail = cli("session", "inspect", session)
                assert detail["state"] not in ("Failed", "Ended"), detail
                return detail if detail["state"] == "Ready" else None

            detail = until(ready, timeout=60)

            def command(name, arguments):
                pending = cli(
                    "session", "submit", session, "--command",
                    json.dumps({"command": name, "arguments": arguments}),
                )

                def completed():
                    activity = cli("session", "poll", session)
                    matches = [
                        entry["event"] for entry in activity["entries"]
                        if entry["event"].get("request_id") == pending["request_id"]
                        and entry["event"]["kind"] != "pending"
                    ]
                    return matches[0] if matches else None

                result = until(completed, timeout=40)
                assert result["kind"] == "completed", result
                return result["output"]

            def state():
                output = command("inspect.query", {
                    "source": "resources",
                    "selector": {"kind": "type", "type_path": "headless_session::SceneState"},
                    "projection": {"kind": "value"},
                })
                return output["items"][0]["result"]["value"]["value"]

            assert state()["ticks"] == 0
            warp = command("tick.warp.start", {"ticks": 1})
            assert warp == {
                "requested_ticks": 1,
                "executed_ticks": 1,
                "outcome": "completed",
            }
            frozen = state()
            assert frozen["ticks"] == 1
            capture = command("screenshot.capture", {"path": "screenshots/fixed-2d.png"})
            assert capture == {
                "path": "screenshots/fixed-2d.png",
                "width": 321,
                "height": 181,
                "overwritten": False,
            }
            assert state() == frozen, "capture advanced simulation, time, or tracked transforms"
            artifact = Path(detail["artifact_dir"]) / capture["path"]
            width, height, rows = read_rgb_png(artifact)
            assert (width, height) == (321, 181)

            def pixel(x, y):
                return rows[y][x * 3:x * 3 + 3]

            assert pixel(width // 2, height // 2) == b"\xff\x00\x00"
            assert pixel(20, 20) == b"\x00\xff\x00"
            assert pixel(width - 1, height - 1) == b"\x00\x00\x00"
            cli("session", "stop", session)
            cli("server", "stop")
            assert server.wait(timeout=15) == 0
            print(json.dumps({
                "acceptance": "passed",
                "warps": 1,
                "captures": 1,
                "size": [width, height],
                "evidence_dir": str(directory),
            }, separators=(",", ":")))
        finally:
            if server.poll() is None:
                server.send_signal(signal.SIGINT)
                try:
                    server.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    server.send_signal(signal.SIGINT)
                    server.wait(timeout=10)


if __name__ == "__main__":
    run()
