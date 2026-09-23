"""Real server signals, cancellation after process spawn, and shared shutdown deadline."""
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "target/debug/woodpecker"


def scenario(mode, interruption, expected, timeout):
    with tempfile.TemporaryDirectory(prefix="shutdown-", dir=ROOT / "target") as directory:
        directory = Path(directory)
        config = directory / "config.toml"
        source = (ROOT / "tests/fixtures/counter.toml").read_text()
        source = source.replace("bevy_test_apps/Cargo.toml", "tests/fixtures/process/Cargo.toml")
        source = source.replace('package = "bevy_test_apps"', 'package = "process_fixture"')
        source = source.replace('name = "counter"', 'name = "process_fixture"')
        source = source.replace('features = ["slice"]', "features = []")
        source = source.replace("arguments = []", f'arguments = ["{mode}"]')
        config.write_text(source)
        with (directory / "log").open("w+") as log:
            server = subprocess.Popen(
                [str(CLI), "--address", "127.0.0.1:0",
                 "server", "start", "--artifact-dir", str(directory / "artifacts"),
                 "--shutdown-seconds", str(timeout)],
                cwd=ROOT, stdout=subprocess.PIPE, stderr=log, text=True,
            )
            try:
                assert select.select([server.stdout], [], [], 10)[0]
                address = json.loads(server.stdout.readline())["address"]

                def cli(*args):
                    result = subprocess.run(
                        [str(CLI), "--address", address, *args],
                        cwd=ROOT, capture_output=True, text=True, timeout=10,
                    )
                    assert result.returncode == 0, result.stderr
                    return json.loads(result.stdout)

                sessions = [cli("session", "create", "--config", str(config)) for _ in range(3)]
                deadline = time.monotonic() + 60
                while True:
                    details = [cli("session", "inspect", s["id"]) for s in sessions]
                    if time.monotonic() >= deadline:
                        processes = subprocess.check_output(
                            ["ps", "-axo", "pid,ppid,pgid,stat,command"], text=True,
                        )
                        print("\n".join(line for line in processes.splitlines()
                                        if any(word in line for word in ("cargo metadata", "cargo run", "process_fixture"))),
                              file=sys.stderr)
                        print([(s["id"], (Path(s["artifact_dir"]) / "pid").exists()) for s in sessions], file=sys.stderr)
                        raise AssertionError(f"sessions did not start: {details}")
                    assert all(d["state"] != "Failed" for d in details), details
                    if mode == "delayed":
                        ready = all((Path(s["artifact_dir"]) / "pid").is_file() for s in sessions)
                    else:
                        ready = all(d["state"] == "Ready" for d in details)
                    if ready:
                        break
                    time.sleep(0.02)
                pids = [int((Path(s["artifact_dir"]) / "pid").read_text()) for s in sessions]
                start = time.monotonic()
                if interruption == "http":
                    cli("server", "stop")
                elif interruption == "term":
                    server.send_signal(signal.SIGTERM)
                else:
                    server.send_signal(signal.SIGINT)
                    deadline = time.monotonic() + 5
                    while not all(cli("session", "inspect", s["id"])["state"] == "Stopping" for s in sessions):
                        assert time.monotonic() < deadline
                        time.sleep(0.01)
                    server.send_signal(signal.SIGINT)
                assert server.wait(timeout=10) == expected
                assert time.monotonic() - start < 3
                for pid in pids:
                    try:
                        os.kill(pid, 0)
                        raise AssertionError(f"process {pid} survived")
                    except ProcessLookupError:
                        pass
            finally:
                if server.poll() is None:
                    server.send_signal(signal.SIGINT)
                    time.sleep(0.1)
                    if server.poll() is None:
                        server.send_signal(signal.SIGINT)
                    server.wait(timeout=10)
                log.seek(0)
                diagnostics = log.read()
                if sys.exc_info()[0] is not None:
                    print(diagnostics, file=sys.stderr)


if __name__ == "__main__":
    scenario("normal", "http", 0, 10)
    scenario("delayed", "http", 0, 10)
    scenario("hang_shutdown", "term", 1, 0.3)
    scenario("hang_shutdown", "interrupt", 1, 30)
    print(json.dumps({"shutdown": "passed", "scenarios": 4, "processes": 12}))
