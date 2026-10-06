"""Automatic report scenarios sharing the lifecycle acceptance's server.

Not a standalone runner: session_lifecycle owns launch, cleanup and server outcome.
Both tracing and panic cases remain explicit; the rendered investigation is not a
replacement for the headless CLI panic/report path.
"""
import json
from pathlib import Path

from tests.support.runtime import ROOT, until


def check_automatic_reports(cli, directory):
    def state(session, expected):
        detail = cli("session", "inspect", session)
        assert detail["state"] != "Failed", detail
        return detail if detail["state"] == expected else None

    def events(session):
        reply = cli("session", "poll", session)
        assert reply["kind"] == "activity", reply
        return [entry["event"] for entry in reply["entries"]]

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
        reports = until(lambda: [e for e in events(session) if e["kind"] == "report"])
        assert len(reports) == 1, reports
        report_event = reports[0]
        assert report_event["report"]["failure"] == failures[0]
        result = report_event["result"]
        assert result["status"] == "submitted", result
        assert result["outcome"]["kind"] == "created", result
        relative = result["outcome"]["reference"]["reference"]["path"]
        artifact_dir = Path(cli("session", "inspect", session)["artifact_dir"])
        markdown = (artifact_dir / relative).read_text()
        signature = report_event["report"]["signature"]["value"]
        assert f"<!-- bug_hunter-signature: {signature} -->" in markdown
        assert report_event["report"]["title"] in markdown
        snapshot = json.dumps(report_event, sort_keys=True)
        until(lambda: any(e["kind"] == "completed"
                          and e["request_id"] == accepted["request_id"] for e in events(session)))
        assert state(session, "Ready")
        cli("session", "stop", session)
        until(lambda: state(session, "Ended"))
        assert sum(e["kind"] == "event" and e["event"]["kind"] == "failure"
                   for e in events(session)) == 1
        assert [json.dumps(e, sort_keys=True) for e in events(session)
                if e["kind"] == "report"] == [snapshot]
