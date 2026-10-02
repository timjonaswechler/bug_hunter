"""Outcome assertions distinct from managed lifecycle completion."""


def managed_end(events):
    # A successful shutdown is not the unexpected session::Event::Ended.
    shutdown = [e for e in events if e["kind"] == "completed" and e["command"] == "shutdown"]
    ended = [e for e in events if e["kind"] == "lifecycle" and e["state"] == "Ended"]
    assert len(shutdown) == 1 and shutdown[0]["output"] is None, shutdown
    assert len(ended) == 1 and ended[0]["error"] is None, ended
    assert not any(e["kind"] == "event" and e["event"]["kind"] == "ended" for e in events), events
