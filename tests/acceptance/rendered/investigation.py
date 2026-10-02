"""Combined graphical CLI acceptance. Requires fresh GUI approval before running.

Two sequential windows; confirm each is visible by creating visible-1.json /
visible-2.json in the printed evidence directory. Leave both on their initial
display; no automatic placement, capture retries or hidden ticks.
"""
import hashlib
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import tempfile

from tests.support.runtime import ROOT, until, cli_call, spawn_server
from tests.support.images import rgb_pixels
from tests.support.lifecycle import managed_end


MESSAGE = "recorded B press reproduced the fixture failure"


def recording(path):
    lines = [json.loads(line) for line in path.read_text().splitlines()]
    assert lines[0] == {"type": "recording_started", "format_version": 1}, lines
    commands = lines[1:-1]
    assert lines[-1] == {"type": "recording_ended", "outcome": "stopped",
                         "recorded_commands": len(commands)}, lines[-1]
    assert commands and all(line["type"] == "command" for line in commands), lines
    assert all(line["command"] not in ("recording.start", "recording.stop", "replay.start",
                                       "replay.stop", "session.shutdown") for line in commands)
    assert all(line["outcome"]["status"] in ("completed", "rejected") for line in commands)
    return lines


def pixels(path):
    width, height, rows = rgb_pixels(path)
    assert (width, height) == (640, 360)
    assert tuple(rows[180][320 * 3:320 * 3 + 3]) == (0, 255, 0)
    assert tuple(rows[10][30:33]) == (0, 0, 0)
    return rows


def gone(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return True
    raise AssertionError(f"game process survived stop: {pid}")


def run():
    directory = Path(tempfile.mkdtemp(prefix="investigation-", dir=ROOT / "target"))
    config = ROOT / "tests/fixtures/investigation.toml"
    print(json.dumps({"evidence": str(directory)}), flush=True)
    with (directory / "server.log").open("w") as log, (directory / "cli.jsonl").open("w") as journal:
        server = spawn_server(directory, log)
        try:
            assert select.select([server.stdout], [], [], 10)[0], "no server Ready"
            address = json.loads(server.stdout.readline())["address"]

            def cli(*args):
                return cli_call(address, *args, journal=journal,
                                journal_keys=("args", "status"), allow_empty=True)

            class Session:
                def __init__(self, index):
                    self.id = cli("session", "create", "--config",
                                  str(config))["id"]
                    self.cursor = None
                    self.events = []
                    until(lambda: self.state("Ready"))
                    self.root = Path(cli("session", "inspect", self.id)["artifact_dir"])
                    self.initial = self.probe()
                    assert self.initial["ticks"] == self.initial["failures"] == 0, self.initial
                    notice = {"session": self.id,
                              "instruction": "Confirm visibility without moving the window. Black before first tick is expected.",
                              "confirm_visible": str(directory / f"visible-{index}.json")}
                    (directory / f"await-visible-{index}.json").write_text(json.dumps(notice))
                    print(json.dumps(notice), flush=True)
                    until(lambda: (directory / f"visible-{index}.json").is_file(), timeout=300)
                    assert self.probe() == self.initial, "visibility wait advanced simulation"

                def state(self, expected):
                    detail = cli("session", "inspect", self.id)
                    assert detail["state"] != "Failed", detail
                    return detail["state"] == expected

                def poll(self):
                    args = ["session", "poll", self.id, "--wait-ms", "100"]
                    if self.cursor is not None:
                        args += ["--cursor", json.dumps(self.cursor)]
                    reply = cli(*args)
                    assert reply["kind"] == "activity", reply
                    self.cursor = reply["cursor"]
                    self.events.extend(entry["event"] for entry in reply["entries"])
                    assert not any(e["kind"] == "event" and e["event"]["kind"] in
                                   ("protocol_error", "recording_failed") for e in self.events), self.events
                    return self.events

                def submit(self, name, arguments):
                    # subprocess.run has exited: the submitting client is now disconnected.
                    pending = cli("session", "submit", self.id, "--command",
                                  json.dumps({"command": name, "arguments": arguments}))
                    assert pending["kind"] == "pending" and pending["command"] == name, pending
                    return pending["request_id"]

                def outcome(self, request):
                    def completed():
                        matches = [e for e in self.poll() if e.get("request_id") == request
                                   and e["kind"] != "pending"]
                        assert len(matches) <= 1, matches
                        return matches[0] if matches else None
                    return until(completed)

                def command(self, name, arguments):
                    result = self.outcome(self.submit(name, arguments))
                    assert result["kind"] == "completed", result
                    return result["output"]

                def probe(self):
                    output = self.command("inspect.query", {
                        "source": "resources", "selector": {"kind": "type", "type_path": "logical_state::Investigation"},
                        "projection": {"kind": "value"},
                    })
                    value = output["items"][0]["result"]["value"]
                    assert value["status"] == "readable", output
                    return value["value"]

                def capture(self, name):
                    before = self.probe()
                    path = f"screenshots/{name}.png"
                    assert self.command("screenshot.capture", {"path": path}) == {
                        "path": path, "width": 640, "height": 360, "overwritten": False}
                    assert self.probe() == before, "Capture advanced the fixture"
                    return pixels(self.root / path)

                def report(self):
                    def available():
                        reports = [e for e in self.poll() if e["kind"] == "report"]
                        assert len(reports) <= 1, reports
                        return reports[0] if reports else None
                    event = until(available)
                    failure = event["report"]["failure"]
                    assert failure["message"] == MESSAGE, event
                    assert failure["origin"]["kind"] == "tracing_error", event
                    assert failure["origin"]["target"] == "investigation_fixture", event
                    assert event["result"]["status"] == "submitted", event
                    assert event["result"]["outcome"]["kind"] == "created", event
                    relative = event["result"]["outcome"]["reference"]["reference"]["path"]
                    markdown = (self.root / relative).read_bytes()
                    text = markdown.decode()
                    assert MESSAGE in text and event["report"]["title"] in text
                    context = event["report"]["context"]
                    assert context["application"]["package"] == "bevy_test_apps", context
                    names = {entry["command"]["command"] for entry in context["commands"]}
                    assert {"inspect.query", "input.keyboard.press", "tick.warp.start",
                            "screenshot.capture"}.issubset(names), names
                    assert "input.keyboard.press" in text and "screenshot.capture" in text
                    assert f"<!-- bug_hunter-signature: {event['report']['signature']['value']} -->" in text
                    (self.root / "report-snapshot.json").write_text(json.dumps({
                        "event": event, "path": relative,
                        "markdown_sha256": hashlib.sha256(markdown).hexdigest(),
                    }, indent=2) + "\n")
                    return json.loads(json.dumps(event)), relative, markdown

                def stop(self, saved_report):
                    cli("session", "stop", self.id)
                    until(lambda: self.state("Ended"))
                    event, relative, markdown = saved_report
                    self.poll()
                    assert [e for e in self.events if e["kind"] == "report"] == [event]
                    failures = [e["event"]["failure"] for e in self.events
                                if e["kind"] == "event" and e["event"]["kind"] == "failure"]
                    assert failures == [event["report"]["failure"]], failures
                    managed_end(self.events)
                    assert (self.root / relative).read_bytes() == markdown, "report changed after stop"
                    gone(self.initial["process_id"])

            first = Session(1)
            first.command("recording.start", {"path": "recordings/investigation.jsonl"})
            first.command("tick.warp.start", {"ticks": 1})  # Explicit layout warm-up, recorded.
            before = first.probe()
            baseline = first.capture("baseline")
            first.command("input.keyboard.press", {"key": "b"})
            assert first.probe() == before
            assert first.capture("queued-input") == baseline
            assert not any(e["kind"] == "report" for e in first.poll())
            pending = first.submit("tick.warp.start", {
                "ticks": 3, "pace": {"kind": "ticks_per_second", "target": 5.0}})
            saved_report = first.report()  # Reconnect while accepted work continues independently.
            assert first.outcome(pending)["output"] == {
                "requested_ticks": 3, "executed_ticks": 3, "outcome": "completed"}
            first.command("input.keyboard.release", {"key": "b"})
            first.command("tick.warp.start", {"ticks": 1})
            final = first.probe()
            assert final == {**first.initial, "ticks": 5, "failures": 1}, final
            assert first.capture("after-failure") == baseline
            first.stop(saved_report)  # Recording is deliberately still active.
            source = first.root / "recordings/investigation.jsonl"
            recorded = recording(source)
            names = [line["command"] for line in recorded[1:-1]]
            assert {"inspect.query", "input.keyboard.press", "input.keyboard.release",
                    "tick.warp.start", "screenshot.capture"}.issubset(names), names

            second = Session(2)
            destination = second.root / "recordings/investigation.jsonl"
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(source.read_bytes())
            second.command("recording.start", {"path": "recordings/replayed.jsonl"})
            assert second.command("replay.start", {"path": "recordings/investigation.jsonl"}) == {
                "outcome": {"kind": "completed"}}
            assert second.probe() == {**second.initial, "ticks": 5, "failures": 1}
            second_report = second.report()
            assert second_report[0]["report"]["signature"] == saved_report[0]["report"]["signature"]
            for name in ("baseline", "queued-input", "after-failure"):
                assert pixels(second.root / f"screenshots/{name}.png") == baseline

            second.stop(second_report)
            replayed = recording(second.root / "recordings/replayed.jsonl")
            assert sum(line["command"] == "screenshot.capture" for line in replayed[1:-1]) == 3
            assert sum(line["arguments"]["ticks"] for line in replayed[1:-1]
                       if line["command"] == "tick.warp.start") == 5
            cli("server", "stop")
            assert server.wait(timeout=15) == 0
            result = {"acceptance": "passed", "sessions": [first.id, second.id],
                      "recorded_commands": len(recorded) - 2,
                      "recording_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                      "evidence": str(directory)}
            (directory / "result.json").write_text(json.dumps(result, indent=2))
            print(json.dumps(result), flush=True)
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
