"""CLI REPL acceptance with real Bevy and a Unix pseudoterminal.

Build woodpecker --features cli and bevy_test_apps counter --features slice first.
Run from the repository root: python3 tests/repl.py
"""
import fcntl
import json
import os
from pathlib import Path
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "target/debug/woodpecker"


class Repl:
    def __init__(self, address, session, tty=False):
        self.tty = tty
        self.text = ""
        self.master = self.slave = None
        args = [str(CLI), "--address", address, "session", "repl", session]
        if tty:
            self.master, self.slave = os.openpty()
            fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
            def controlling_terminal():
                os.setsid()
                fcntl.ioctl(0, termios.TIOCSCTTY, 0)

            # Keep the controlling session leader alive while checking restoration.
            # macOS revokes the slave as soon as that leader exits.
            supervisor = """
import signal, subprocess, sys, termios
original = termios.tcgetattr(0)
child = subprocess.Popen(sys.argv[1:])
signal.signal(signal.SIGINT, lambda *_: child.send_signal(signal.SIGINT))
code = child.wait()
assert termios.tcgetattr(0) == original, "terminal mode not restored"
sys.exit(code)
"""
            self.process = subprocess.Popen(
                [sys.executable, "-c", supervisor, *args],
                cwd=ROOT, stdin=self.slave, stdout=self.slave, stderr=self.slave,
                preexec_fn=controlling_terminal,
            )
            self.reader = self.master
        else:
            self.process = subprocess.Popen(
                args, cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
            )
            self.reader = self.process.stdout.fileno()
        self.wait("connected ")

    def send(self, text):
        data = text.encode() if isinstance(text, str) else text
        if self.tty:
            os.write(self.master, data)
        else:
            self.process.stdin.write(data)
            self.process.stdin.flush()

    def wait(self, fragment, timeout=10):
        deadline = time.monotonic() + timeout
        while fragment not in self.text:
            assert time.monotonic() < deadline, (fragment, self.text)
            if select.select([self.reader], [], [], 0.1)[0]:
                data = os.read(self.reader, 65536)
                assert data, (self.process.poll(), fragment, self.text)
                self.text += data.decode(errors="replace")
        return self.text

    def finish(self, action="quit", expected=0):
        if action == "eof":
            self.process.stdin.close()
        elif action == "signal":
            self.process.send_signal(signal.SIGINT)
        elif action == "key":
            self.send(b"\x03")
        elif action == "key_eof":
            self.send(b"\x04")
        elif action == "quit":
            self.send("quit\r" if self.tty else "quit\n")
        # A real terminal continuously consumes output. Keep draining even after
        # sending Ctrl+C; a large Inspect result can exceed the PTY buffer.
        deadline = time.monotonic() + 3
        while self.process.poll() is None:
            assert time.monotonic() < deadline, ("REPL did not exit", self.text)
            if select.select([self.reader], [], [], 0.02)[0]:
                data = os.read(self.reader, 65536)
                self.text += data.decode(errors="replace")
        assert self.process.returncode == expected, self.text
    def close(self):
        if self.process.poll() is None:
            self.finish("signal")
        if self.master is not None:
            os.close(self.master)
            os.close(self.slave)
        else:
            if not self.process.stdin.closed:
                self.process.stdin.close()
            self.process.stdout.close()


def run():
    with tempfile.TemporaryDirectory(prefix="repl-", dir=ROOT / "target") as directory:
        directory = Path(directory)
        with (directory / "server.log").open("w+") as log:
            server = subprocess.Popen(
                [str(CLI), "--address", "127.0.0.1:0", "server", "start",
                 "--artifact-dir", str(directory / "artifacts"), "--shutdown-seconds", "10"],
                cwd=ROOT, stdout=subprocess.PIPE, stderr=log, text=True,
            )
            clients = []
            try:
                assert select.select([server.stdout], [], [], 10)[0]
                address = json.loads(server.stdout.readline())["address"]

                def cli(*args):
                    result = subprocess.run(
                        [str(CLI), "--address", address, *args], cwd=ROOT,
                        capture_output=True, text=True, timeout=40,
                    )
                    assert result.returncode == 0, (args, result.stdout, result.stderr)
                    return json.loads(result.stdout)

                def state(session):
                    return cli("session", "inspect", session)["state"]

                def create():
                    session = cli("session", "create", "--config",
                                  str(ROOT / "tests/fixtures/counter.toml"))["id"]
                    deadline = time.monotonic() + 90
                    while state(session) != "Ready":
                        assert time.monotonic() < deadline
                        time.sleep(0.05)
                    return session

                def submit(session, name, arguments):
                    return cli("session", "submit", session, "--command",
                               json.dumps({"command": name, "arguments": arguments}))

                def attach(session, tty=False):
                    repl = Repl(address, session, tty)
                    clients.append(repl)
                    return repl

                session = create()
                repl = attach(session)
                repl.send("tick warp 100000 pace 10\n")
                repl.wait("[1] tick.warp.start pending")
                repl.send("pending\ninspect entity 1:2:3\nhelp inspect\n")
                repl.wait("server pending: 1")
                repl.wait("input error:")
                repl.wait("component_names")
                repl.send("inspect resources\n")
                repl.wait('"command":"inspect.query"')
                # Bad parse and local commands consume no request IDs.
                repl.wait('"request_id":2')
                repl.send("tick warp stop\n")
                repl.wait('"outcome":"stopped"')
                repl.wait('"was_running":true')
                assert repl.text.count("[1] tick.warp.start pending") == 2, repl.text  # initial + explicit pending
                repl.finish()
                assert state(session) == "Ready"

                # A different client's pending command is visible on initial attach.
                slow = submit(session, "tick.warp.start", {
                    "ticks": 100000, "pace": {"kind": "ticks_per_second", "target": 10},
                })["request_id"]
                tty = attach(session, tty=True)
                tty.wait(f"[{slow}] tick.warp.start pending")
                tty.send("inspect resour")
                time.sleep(0.3)
                external = submit(session, "tick.warp.set_pace", {
                    "pace": {"kind": "ticks_per_second", "target": 20},
                })["request_id"]
                tty.wait(f'"request_id":{external}')
                # Activity output must preserve the partially edited input.
                tty.send("ces\r")
                tty.wait('"command":"inspect.query"')
                tty.finish("key")
                assert state(session) == "Ready"

                for action in ("eof", "signal", "quit"):
                    repl = attach(session)
                    repl.wait(f"[{slow}] tick.warp.start pending")
                    repl.finish(action)
                    assert state(session) == "Ready"
                tty = attach(session, tty=True)
                tty.finish("key_eof")
                assert state(session) == "Ready"
                stopped = submit(session, "tick.warp.stop", {})
                assert stopped["kind"] == "pending"

                # Replay gate applies also when another client starts the replay.
                root = Path(cli("session", "inspect", session)["artifact_dir"])
                (root / "long.jsonl").write_text("\n".join(json.dumps(line) for line in [
                    {"type": "recording_started", "format_version": 1},
                    {"type": "command", "command": "tick.warp.start",
                     "arguments": {"ticks": 100000, "pace": {"kind": "ticks_per_second", "target": 5}},
                     "outcome": {"status": "unanswered"}},
                    {"type": "recording_ended", "outcome": "session_ended", "recorded_commands": 1},
                ]))
                replay = submit(session, "replay.start", {"path": "long.jsonl"})["request_id"]
                repl = attach(session)
                repl.wait(f"[{replay}] replay.start pending")
                repl.send("inspect resources\n")
                repl.wait("only replay stop is allowed")
                repl.send("replay stop\n")
                repl.wait('"was_running":true')
                repl.send("shutdown\n")
                repl.wait('"state":"Ended"')
                repl.finish(action=None)
                assert state(session) == "Ended"

                # An external end is an error, not a silent successful quit.
                second = create()
                repl = attach(second)
                cli("session", "stop", second)
                repl.wait("session ended outside this REPL")
                repl.finish(action=None, expected=1)
                cli("server", "stop")
                assert server.wait(timeout=15) == 0
                print(json.dumps({"acceptance": "passed", "repl": True,
                                  "sessions": 2, "pseudoterminal": True}))
            finally:
                for repl in clients:
                    repl.close()
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
