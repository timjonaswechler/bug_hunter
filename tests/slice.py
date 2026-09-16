"""Real CLI -> HTTP/WebSocket -> session -> Bevy acceptance, without transport adapters.

Run after building both binaries, from the repository root:
    python3 tests/slice.py
"""
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "target/debug/woodpecker"


def until(callback, timeout=90):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = callback()
        if result:
            return result
        time.sleep(0.025)
    raise AssertionError("condition did not become true")


def run():
    with tempfile.TemporaryDirectory(prefix="slice-", dir=ROOT / "target") as directory:
        directory = Path(directory)
        log = (directory / "server.log").open("w+")
        server = subprocess.Popen(
            [str(CLI), "--address", "127.0.0.1:0",
             "server", "start", "--artifact-dir", str(directory / "artifacts"),
             "--shutdown-seconds", "10"],
            cwd=ROOT, stdout=subprocess.PIPE, stderr=log, text=True,
        )
        try:
            assert select.select([server.stdout], [], [], 10)[0], "no server Ready"
            ready = json.loads(server.stdout.readline())
            address = ready["address"]

            def cli(*args, success=True):
                result = subprocess.run(
                    [str(CLI), "--address", address, *args],
                    cwd=ROOT, capture_output=True, text=True, timeout=40,
                )
                if success:
                    assert result.returncode == 0, (args, result.stdout, result.stderr)
                else:
                    assert result.returncode != 0, args
                return json.loads(result.stdout) if result.stdout.strip() else None

            def state(session, expected):
                detail = cli("session", "inspect", session)
                assert detail["state"] != "Failed" or expected == "Failed", detail
                return detail if detail["state"] == expected else None

            def submit(session, name, arguments):
                return cli("session", "submit", session, "--command",
                           json.dumps({"command": name, "arguments": arguments}))

            def outcome(session, request):
                def available():
                    reply = cli("session", "poll", session)
                    assert reply["kind"] == "activity", reply
                    matching = [item["event"] for item in reply["entries"]
                                if item["event"].get("request_id") == request]
                    if len(matching) < 2:
                        return None
                    assert matching[0]["kind"] == "pending", matching
                    assert matching[-1]["kind"] in ("completed", "rejected", "failed"), matching
                    return matching[-1]
                return until(available)

            def inspect(session):
                pending = submit(session, "inspect.query", {
                    "source": "resources", "selector": {"kind": "type", "type_path": "counter::Counter"},
                    "projection": {"kind": "value"},
                })
                value = outcome(session, pending["request_id"])
                assert value["kind"] == "completed", value
                return value["output"]["items"][0]["result"]["value"]["value"]

            assert cli("session", "ls") == []
            with urllib.request.urlopen(f"http://{address}/v1/sessions") as response:
                assert json.load(response)["data"] == []
            try:
                urllib.request.urlopen(urllib.request.Request(
                    f"http://{address}/v1/sessions", headers={"Origin": "http://localhost"},
                ))
                raise AssertionError("browser-origin request succeeded")
            except urllib.error.HTTPError as error:
                assert error.code == 403

            config = ROOT / "tests/fixtures/counter.toml"
            first = cli("session", "create", "--config", str(config))
            assert first["state"] == "Starting", first
            first = first["id"]
            assert len(first) == 32 and first == first.lower()
            until(lambda: state(first, "Ready"))
            initial = inspect(first)
            time.sleep(0.1)
            assert inspect(first)["ticks"] == initial["ticks"] == 0

            warp = submit(first, "tick.warp.start", {"ticks": 13})
            assert warp["kind"] == "pending", warp
            result = outcome(first, warp["request_id"])
            assert result["output"] == {"requested_ticks": 13, "executed_ticks": 13, "outcome": "completed"}, result
            assert inspect(first)["ticks"] == 13

            # Each CLI call disconnects. Accepted work must remain live between calls.
            slow = submit(first, "tick.warp.start", {
                "ticks": 100000, "pace": {"kind": "ticks_per_second", "target": 5.0},
            })
            time.sleep(0.3)
            assert 13 < inspect(first)["ticks"] < 100013
            changed = submit(first, "tick.warp.set_pace", {
                "pace": {"kind": "ticks_per_second", "target": 10.0},
            })
            assert outcome(first, changed["request_id"])["kind"] == "completed"
            stopped = submit(first, "tick.warp.stop", {})
            assert outcome(first, stopped["request_id"])["output"]["was_running"]
            assert outcome(first, slow["request_id"])["output"]["outcome"] == "stopped"

            second = cli("session", "create", "--config", str(config))["id"]
            until(lambda: state(second, "Ready"))
            second_warp = submit(second, "tick.warp.start", {"ticks": 7})
            assert second_warp["request_id"] == 1
            assert outcome(second, 1)["output"]["executed_ticks"] == 7
            assert inspect(second)["ticks"] == 7
            # Request 1 in the first session was Inspect, not this second session's Warp.
            assert outcome(first, 1)["command"] == "inspect.query"
            second_pid = inspect(second)["process_id"]

            cli("session", "stop", first)
            until(lambda: state(first, "Ended"))
            assert cli("session", "inspect", second)["state"] == "Ready"
            assert len(cli("session", "ls")) == 2
            try:
                os.kill(initial["process_id"], 0)
                raise AssertionError("first game process survived shutdown")
            except ProcessLookupError:
                pass

            # Stop during a deliberately delayed Ready.
            delayed = directory / "delayed.toml"
            delayed.write_text(config.read_text().replace("arguments = []", 'arguments = ["3000"]'))
            starting = cli("session", "create", "--config", str(delayed))["id"]
            assert cli("session", "inspect", starting)["state"] == "Starting"
            cli("session", "stop", starting)
            until(lambda: state(starting, "Ended"))

            missing = directory / "missing.toml"
            missing.write_text(config.read_text().replace("bevy_test_apps/Cargo.toml", "missing/Cargo.toml"))
            failed = cli("session", "create", "--config", str(missing))["id"]
            until(lambda: state(failed, "Failed"))

            # A failed entry makes the overall shutdown a failure, while all live sessions still close.
            cli("server", "stop")
            assert server.wait(timeout=15) == 1
            try:
                os.kill(second_pid, 0)
                raise AssertionError("second game process survived server shutdown")
            except ProcessLookupError:
                pass
            assert (directory / "artifacts" / first).is_dir()
            assert (directory / "artifacts" / second).is_dir()
            print(json.dumps({"acceptance": "passed", "sessions": 4, "bevy": "0.19.1"}))
        finally:
            if server.poll() is None:
                server.send_signal(signal.SIGINT)
                try:
                    server.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    server.send_signal(signal.SIGINT)
                    server.wait(timeout=10)
            log.seek(0)
            diagnostics = log.read()
            if server.returncode not in (0, 1):
                print(diagnostics)
            log.close()


if __name__ == "__main__":
    run()
