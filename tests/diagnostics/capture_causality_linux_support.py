"""Shared assertions for the bounded Linux capture-causality experiment."""

import hashlib
import json
from pathlib import Path

from diagnostics.capture_scene_visibility_support import (
    correlate_capture_rows,
    intersection_area,
    read_json_sources,
    rect,
)


SUCCESSFUL_ACQUIRE = {"success", "suboptimal"}


def chain_event(chain, name):
    rows = [row for row in chain["bevy"] if row.get("event") == name]
    if len(rows) != 1:
        raise AssertionError((name, rows))
    return rows[0]


def acquire_outcomes(chain):
    return {row.get("outcome") for row in chain["surface_acquire"]}


def raw_stats(path):
    data = Path(path).read_bytes()
    return {
        "byte_length": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "nonzero_all_bytes": sum(value != 0 for value in data),
    }


def classify_capture_result(captures):
    """Classify a complete visible/partial/covered/restored measurement.

    State-control failures are handled by the driver before this function. This
    classifier deliberately does not assume that X11 occlusion makes acquire
    fail.
    """
    if len(captures) != 4:
        raise AssertionError(f"expected four captures, got {len(captures)}")
    baseline, partial, covered, restored = captures

    def raw_content(item):
        raw = item.get("raw")
        return None if raw is None else {key: value for key, value in raw.items() if key != "path"}

    reference = raw_content(baseline)
    outer_expected = all(
        item["event"].get("kind") == "completed"
        and chain_event(item["chain"], "queue_submitted")["copy_encoded"] is True
        and chain_event(item["chain"], "map_success")["copy_encoded"] is True
        and chain_event(item["chain"], "image_returned")["nonzero_rgb_bytes"] > 0
        and acquire_outcomes(item["chain"]) & SUCCESSFUL_ACQUIRE
        and raw_content(item) == reference
        and item.get("png") == baseline.get("png")
        for item in (baseline, partial, restored)
    )
    if not outer_expected:
        return "other_error_path_observed"

    image = chain_event(covered["chain"], "image_returned")
    submitted = chain_event(covered["chain"], "queue_submitted")
    mapped = chain_event(covered["chain"], "map_success")
    skipped = [row for row in covered["chain"]["bevy"] if row.get("event") == "copy_skipped"]
    outcomes = acquire_outcomes(covered["chain"])
    adapter_error = covered["event"].get("error", {}).get("code")

    if (
        covered["event"].get("kind") == "rejected"
        and adapter_error == "screenshot_window_unavailable"
        and len(skipped) == 1
        and skipped[0].get("reason") == "swap_chain_view_missing"
        and submitted["copy_encoded"] is False
        and mapped["copy_encoded"] is False
        and image["nonzero_all_bytes"] == 0
        and image["nonzero_rgb_bytes"] == 0
        and covered.get("png") is None
    ):
        return "reproduced_skip_copy_zero_readback"

    if (
        covered["event"].get("kind") == "completed"
        and outcomes & SUCCESSFUL_ACQUIRE
        and submitted["copy_encoded"] is True
        and mapped["copy_encoded"] is True
        and raw_content(covered) == reference
        and covered.get("png") == baseline.get("png")
    ):
        return "occlusion_did_not_cause_surface_failure"

    return "other_error_path_observed"


def load_complete_chain(log_directory, request_id, session_id):
    return correlate_capture_rows(
        read_json_sources(log_directory),
        request_id,
        session_id,
    )


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


__all__ = [
    "acquire_outcomes",
    "chain_event",
    "classify_capture_result",
    "intersection_area",
    "load_complete_chain",
    "raw_stats",
    "read_json_sources",
    "rect",
    "write_json",
]
