"""Window-state control shared by disposable scene-visibility test copies."""

import json
from pathlib import Path
import select
import subprocess
import threading
import time


def read_json_lines(path):
    if not path.exists():
        return []
    text = path.read_text()
    lines = text.splitlines()
    rows = []
    for index, line in enumerate(lines):
        if not line.strip():
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            # The sampler appends while the controller reads. Ignore only a
            # malformed, unterminated tail. Completed malformed lines fail.
            if index == len(lines) - 1 and not text.endswith("\n"):
                break
            raise
    return rows


def read_json_sources(location):
    location = Path(location)
    paths = sorted(location.glob("*.jsonl")) if location.is_dir() else [location]
    rows = []
    for path in paths:
        rows.extend(read_json_lines(path))
    return rows


def correlate_capture_rows(rows, request_id, session_id):
    started = [
        row for row in rows
        if row.get("source") == "woodpecker_adapter"
        and row.get("session_id") == session_id
        and row.get("fields", {}).get("event") == "adapter_capture_started"
        and row["fields"].get("request_id") == request_id
    ]
    if not started:
        return None
    if len(started) != 1:
        raise AssertionError(("duplicate adapter_capture_started", request_id, session_id, started))
    entity = started[0]["fields"]["screenshot_entity"]
    bevy = [
        row for row in rows
        if row.get("source") == "bevy_render"
        and row.get("session_id") == session_id
        and row.get("screenshot_entity") == entity
    ]
    if not any(row.get("event") == "image_returned" for row in bevy):
        return None
    prepared = [row for row in bevy if row.get("event") == "screenshot_prepared"]
    if len(prepared) != 1:
        raise AssertionError(("screenshot_prepared", request_id, session_id, prepared))
    frame_id = prepared[0]["frame_id"]
    acquire = [
        row for row in rows
        if row.get("source") == "bevy_render"
        and row.get("session_id") == session_id
        and row.get("event") == "surface_acquire"
        and row.get("frame_id") == frame_id
    ]
    adapter = [
        row for row in rows
        if row.get("source") == "woodpecker_adapter"
        and row.get("session_id") == session_id
        and row.get("fields", {}).get("request_id") == request_id
    ]
    return {
        "screenshot_entity": entity,
        "bevy": bevy,
        "surface_acquire": acquire,
        "adapter": adapter,
    }


def rect(row, prefix="native_"):
    return tuple(float(row[prefix + key]) for key in ("x", "y", "width", "height"))


def intersection_area(a, b):
    ax, ay, aw, ah = a
    bx, by, bw, bh = b
    width = max(0.0, min(ax + aw, bx + bw) - max(ax, bx))
    height = max(0.0, min(ay + ah, by + bh) - max(ay, by))
    return width * height


class VisibilityController:
    def __init__(self, helper, native_log, evidence_dir):
        self.helper = Path(helper)
        self.native_log = Path(native_log)
        self.evidence_dir = Path(evidence_dir)
        self.evidence_dir.mkdir(parents=True, exist_ok=True)
        self.deck_log = (self.evidence_dir / "deck.stderr.log").open("w")
        self.deck = subprocess.Popen(
            [str(self.helper)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self.deck_log,
            text=True,
            bufsize=1,
        )
        self.rows = []
        self.lock = threading.Lock()
        self.sequence = 0
        self.restored_sessions = set()

    def _deck_state(self, command, expected):
        self.deck.stdin.write(command + "\n")
        self.deck.stdin.flush()
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            ready, _, _ = select.select([self.deck.stdout], [], [], 0.25)
            if ready:
                row = json.loads(self.deck.stdout.readline())
                if row.get("deck_state") == expected:
                    return row
                raise AssertionError((command, row))
        raise AssertionError(f"deck command timed out: {command}")

    def _native(self, session_id):
        return [
            row
            for row in read_json_sources(self.native_log)
            if row.get("source") == "appkit_main" and row.get("session_id") == session_id
        ]

    def _stable(self, session_id, predicate, description, after_sample=0, timeout=12):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            rows = [row for row in self._native(session_id) if row["sample_id"] > after_sample]
            tail = []
            for row in reversed(rows):
                if not predicate(row):
                    break
                tail.append(row)
            tail.reverse()
            if len(tail) >= 8:
                window = tail[-8:]
                stable = {
                    (
                        row["visible"], row["key"], row["miniaturized"],
                        row["occlusion_visible"], rect(row),
                        row["bevy_physical_width"], row["bevy_physical_height"],
                        row["bevy_scale_factor"], json.dumps(row["cameras"], sort_keys=True),
                    )
                    for row in window
                }
                if len(stable) == 1 and window[-1]["unix_time_ns"] - window[0]["unix_time_ns"] >= 100_000_000:
                    return {
                        "description": description,
                        "sample_count": 8,
                        "quiet_span_ns": window[-1]["unix_time_ns"] - window[0]["unix_time_ns"],
                        "first": window[0],
                        "last": window[-1],
                    }
            time.sleep(0.02)
        raise AssertionError(f"native state not confirmed: {description} for {session_id}")

    def place(self, session_id, artifact_dir, state):
        with self.lock:
            self.sequence += 1
            artifact_dir = Path(artifact_dir)
            artifact_dir.mkdir(parents=True, exist_ok=True)
            (artifact_dir / "visibility-front").write_text(f"{self.sequence}\n")
            front = self._stable(
                session_id,
                lambda row: row["visible"] and not row["miniaturized"],
                "owned target window ordered in",
            )
            target = rect(front["last"])
            target_area = target[2] * target[3]
            args = " ".join(str(value) for value in target)
            after = front["last"]["sample_id"]

            transitions = []
            if state == "restored" and session_id in self.restored_sessions:
                state = "visible"
            elif state == "restored":
                covered_deck = self._deck_state(f"cover {args}", "cover")
                cover_area = intersection_area(target, rect(covered_deck, prefix=""))
                assert abs(cover_area - target_area) < 0.01
                covered = self._stable(
                    session_id,
                    lambda row: row["visible"] and not row["miniaturized"]
                    and not row["occlusion_visible"] and rect(row) == target,
                    "pre-restoration full cover",
                    after_sample=after,
                )
                transitions.append({"state": "covered", "deck": covered_deck, "native": covered})
                self.restored_sessions.add(session_id)
                after = covered["last"]["sample_id"]
                state = "visible"

            if state == "visible":
                deck = self._deck_state("small", "small")
                native = self._stable(
                    session_id,
                    lambda row: row["visible"] and not row["key"] and not row["miniaturized"]
                    and row["occlusion_visible"] and rect(row) == target,
                    "visible without focus",
                    after_sample=after,
                )
                classified = "visible" if not transitions else "restored"
                overlap = intersection_area(target, rect(deck, prefix=""))
                assert overlap == 0.0
            elif state == "partial":
                deck = self._deck_state(f"partial {args}", "partial")
                overlap = intersection_area(target, rect(deck, prefix=""))
                assert 0.0 < overlap < target_area
                native = self._stable(
                    session_id,
                    lambda row: row["visible"] and not row["key"] and not row["miniaturized"]
                    and rect(row) == target,
                    "geometrically partial cover",
                    after_sample=after,
                )
                classified = "partial"
            elif state == "covered":
                deck = self._deck_state(f"cover {args}", "cover")
                overlap = intersection_area(target, rect(deck, prefix=""))
                assert abs(overlap - target_area) < 0.01
                native = self._stable(
                    session_id,
                    lambda row: row["visible"] and not row["miniaturized"]
                    and not row["occlusion_visible"] and rect(row) == target,
                    "geometrically full cover",
                    after_sample=after,
                )
                classified = "covered"
            else:
                raise ValueError(state)

            item = {
                "ordinal": len(self.rows) + 1,
                "state": classified,
                "session_id": session_id,
                "artifact_dir": str(artifact_dir),
                "target_rect": target,
                "target_area": target_area,
                "deck": deck,
                "intersection_area": overlap,
                "native": native,
                "transitions": transitions,
            }
            self.rows.append(item)
            (self.evidence_dir / "visibility-states.json").write_text(json.dumps(self.rows, indent=2) + "\n")
            return item

    def close(self):
        if self.deck.poll() is None:
            try:
                self._deck_state("quit", "quit")
                self.deck.wait(timeout=5)
            except Exception:
                self.deck.terminate()
                self.deck.wait(timeout=5)
        cleanup = {
            "deck_pid": self.deck.pid,
            "deck_returncode": self.deck.returncode,
            "deck_running": self.deck.poll() is None,
        }
        (self.evidence_dir / "deck-cleanup.json").write_text(json.dumps(cleanup, indent=2) + "\n")
        self.deck.stdout.close()
        self.deck_log.close()
        return cleanup
