"""Prevalidated scripts through real CLI, server, Session and Bevy.

Build woodpecker --features cli and bevy_test_apps counter --features slice first.
Run from the repository root: python3 tests/script.py
"""
import json
from pathlib import Path
import select
import signal
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "target/debug/woodpecker"


def command(name, **arguments):
    return {"command": name, "arguments": arguments}


INSPECT = command("inspect.query", source="resources",
                  selector={"kind": "type", "type_path": "counter::Counter"},
                  projection={"kind": "value"})


def run():
    with tempfile.TemporaryDirectory(prefix="script-", dir=ROOT / "target") as directory:
        directory = Path(directory)
        with (directory / "server.log").open("w+") as log:
            server = subprocess.Popen(
                [str(CLI), "--address", "127.0.0.1:0", "server", "start",
                 "--artifact-dir", str(directory / "artifacts"), "--shutdown-seconds", "10"],
                cwd=ROOT, stdout=subprocess.PIPE, stderr=log, text=True,
            )
            processes = []
            try:
                assert select.select([server.stdout], [], [], 10)[0]
                address = json.loads(server.stdout.readline())["address"]

                def cli(*args, success=True):
                    result = subprocess.run(
                        [str(CLI), "--address", address, *args], cwd=ROOT,
                        capture_output=True, text=True, timeout=40,
                    )
                    assert (result.returncode == 0) == success, (args, result.stdout, result.stderr)
                    return json.loads(result.stdout) if result.stdout else None

                def until(callback):
                    deadline = time.monotonic() + 90
                    while time.monotonic() < deadline:
                        value = callback()
                        if value:
                            return value
                        time.sleep(0.025)
                    raise AssertionError("condition did not become true")

                def detail(session):
                    return cli("session", "inspect", session)

                def create():
                    session = cli("session", "create", "--config",
                                  str(ROOT / "tests/fixtures/counter.toml"))["id"]
                    until(lambda: detail(session)["state"] == "Ready")
                    return session

                def write(commands, name="run.json"):
                    path = directory / name
                    path.write_text(json.dumps({"version": 1, "commands": commands}))
                    return path

                def script(session, commands, success=True):
                    return cli("session", "script", session, "--file", str(write(commands)), success=success)

                def activity(session):
                    return [entry["event"] for entry in cli("session", "poll", session)["entries"]]

                session = create()
                # No valid prefix may run before a later invalid command is discovered.
                script(session, [command("tick.warp.start", ticks=13),
                                 command("inspect.query", source="unknown")], success=False)
                script(session, [command("shutdown"), INSPECT], success=False)
                assert not any(event["kind"] == "pending" for event in activity(session))
                assert script(session, []) == {"kind": "passed", "completed": []}
                bad_utf8 = directory / "invalid-utf8.json"
                bad_utf8.write_bytes(b'{"version":1,"commands":[]}\xff')
                cli("session", "script", session, "--file", str(bad_utf8), success=False)
                assert not any(event["kind"] == "pending" for event in activity(session))

                # If ordinary commands waited for Warp completion, this would time out.
                pipeline = script(session, [
                    command("tick.warp.start", ticks=100000,
                            pace={"kind": "ticks_per_second", "target": 20}),
                    INSPECT, command("tick.warp.stop"),
                ])
                assert pipeline["kind"] == "passed", pipeline
                assert [item["command_index"] for item in pipeline["completed"]] == [0, 1, 2]
                assert [item["request_id"] for item in pipeline["completed"]] == [1, 2, 3]
                assert pipeline["completed"][0]["output"]["outcome"] == "stopped", pipeline
                assert pipeline["completed"][2]["output"]["was_running"], pipeline

                # A semantic rejection is collected, not confused with invalid syntax.
                failed = script(session, [
                    command("tick.warp.set_pace", pace={"kind": "ticks_per_second", "target": -1}),
                    INSPECT,
                ], success=False)
                assert failed["kind"] == "failed", failed
                assert failed["failures"][0]["command_index"] == 0
                assert failed["failures"][0]["request_id"] == 4
                assert failed["failures"][0]["reason"]["kind"] == "rejected"
                assert failed["completed"][0]["command_index"] == 1
                assert detail(session)["state"] == "Ready"

                # Real header/footer barriers: only the enclosed Warp is recorded.
                recorded = script(session, [
                    command("tick.warp.start", ticks=3),
                    command("recording.start", path="recordings/script.jsonl"),
                    command("tick.warp.start", ticks=7),
                    command("recording.stop"), INSPECT, command("shutdown"),
                ])
                assert recorded["kind"] == "passed", recorded
                assert recorded["completed"][-1]["request_id"] == 2**64 - 1
                root = Path(detail(session)["artifact_dir"])
                lines = [json.loads(line) for line in
                         (root / "recordings/script.jsonl").read_text().splitlines()]
                assert len(lines) == 3, lines
                assert lines[1]["command"] == "tick.warp.start"
                assert lines[1]["arguments"]["ticks"] == 7
                assert lines[-1]["recorded_commands"] == 1
                until(lambda: detail(session)["state"] == "Ended")

                # Interrupting the client while it waits must not stop the accepted Warp.
                second = create()
                path = write([
                    command("tick.warp.start", ticks=100000,
                            pace={"kind": "ticks_per_second", "target": 10}),
                    command("recording.start", path="must-not-start.jsonl"),
                ], "interrupt.json")
                child = subprocess.Popen(
                    [str(CLI), "--address", address, "session", "script", second, "--file", str(path)],
                    cwd=ROOT, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE, text=True,
                )
                processes.append(child)
                until(lambda: any(e["kind"] == "pending" for e in activity(second)))
                # stdin is already EOF. Script execution does not treat that as Shutdown.
                assert child.poll() is None
                child.send_signal(signal.SIGINT)
                stdout, stderr = child.communicate(timeout=3)
                assert child.returncode != 0, (stdout, stderr)
                interrupted = json.loads(stdout)
                assert interrupted["failures"][0]["reason"]["kind"] == "unknown", interrupted
                assert interrupted["failures"][1]["reason"]["kind"] == "not_submitted", interrupted
                assert detail(second)["state"] == "Ready"
                assert not (Path(detail(second)["artifact_dir"]) / "must-not-start.jsonl").exists()
                # A separate script can still inspect and stop that live Warp.
                stopped = script(second, [INSPECT, command("tick.warp.stop")])
                assert stopped["kind"] == "passed", stopped
                assert stopped["completed"][1]["output"]["was_running"]
                script(second, [command("shutdown")])
                cli("server", "stop")
                assert server.wait(timeout=15) == 0
                print(json.dumps({"acceptance": "passed", "script": True,
                                  "sessions": 2, "barriers": ["recording.start", "recording.stop", "shutdown"]}))
            finally:
                for child in processes:
                    if child.poll() is None:
                        child.send_signal(signal.SIGINT)
                        child.communicate(timeout=3)
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
