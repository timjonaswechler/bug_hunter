"""Real CLI/server acceptance for observation events, without report publication."""
import json
import select
import signal
import subprocess
import tempfile
from pathlib import Path

from slice import CLI, ROOT, until


def run():
    with tempfile.TemporaryDirectory(prefix="observation-", dir=ROOT / "target") as temp:
        directory = Path(temp)
        with (directory / "server.log").open("w+") as log:
            server = subprocess.Popen(
                [str(CLI), "--address", "127.0.0.1:0", "server", "start",
                 "--artifact-dir", str(directory / "artifacts"), "--shutdown-seconds", "10"],
                cwd=ROOT, stdout=subprocess.PIPE, stderr=log, text=True,
            )
            try:
                assert select.select([server.stdout], [], [], 10)[0]
                address = json.loads(server.stdout.readline())["address"]

                def cli(*args):
                    result = subprocess.run([str(CLI), "--address", address, *args],
                                            cwd=ROOT, capture_output=True, text=True, timeout=40)
                    assert result.returncode == 0, (args, result.stdout, result.stderr)
                    return json.loads(result.stdout) if result.stdout.strip() else None

                def state(session, expected):
                    detail = cli("session", "inspect", session)
                    assert detail["state"] != "Failed", detail
                    return detail if detail["state"] == expected else None

                def events(session):
                    return [entry["event"] for entry in cli("session", "poll", session)["entries"]]

                for mode, origin in [("trace", "tracing_error"), ("caught", "panic")]:
                    config = directory / f"{mode}.toml"
                    config.write_text(
                        'version = 1\n\n'
                        f'[launch]\nmanifest_path = {json.dumps(str(ROOT / "Cargo.toml"))}\n'
                        f'package = "woodpecker"\nfeatures = []\narguments = ["{mode}"]\n'
                        '[launch.target]\nkind = "example"\nname = "observation_fixture"\n'
                        '[tick.pace]\nkind = "as_fast_as_possible"\n'
                        '[report]\ntracing_errors = true\noutput = "reports"\n'
                        '[report.provider]\nkind = "local"\n'
                    )
                    session = cli("session", "create", "--config", str(config))["id"]
                    until(lambda: state(session, "Ready"))
                    accepted = cli("session", "submit", session, "--command", json.dumps({
                        "command": "tick.warp.start", "arguments": {"ticks": 1}}))
                    # Each call disconnects. Observation and command progress remain independent.
                    failures = until(lambda: [e["event"]["failure"] for e in events(session)
                                             if e["kind"] == "event" and e["event"]["kind"] == "failure"])
                    assert len(failures) == 1 and failures[0]["origin"]["kind"] == origin, failures
                    until(lambda: any(e["kind"] == "completed"
                                      and e["request_id"] == accepted["request_id"] for e in events(session)))
                    assert state(session, "Ready")
                    cli("session", "stop", session)
                    until(lambda: state(session, "Ended"))
                    assert sum(e["kind"] == "event" and e["event"]["kind"] == "failure"
                               for e in events(session)) == 1
                cli("server", "stop")
                assert server.wait(timeout=15) == 0
                print(json.dumps({"acceptance": "passed", "observation": True, "sessions": 2}))
            finally:
                if server.poll() is None:
                    server.send_signal(signal.SIGINT)
                    server.wait(timeout=15)


if __name__ == "__main__":
    run()
