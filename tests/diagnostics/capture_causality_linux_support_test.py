import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from diagnostics.capture_causality_linux_support import classify_capture_result


class CaptureClassificationTest(unittest.TestCase):
    def capture(self, *, covered=False):
        capture_id = 2 if covered else 1
        raw = {"sha256": "scene", "byte_length": 16, "nonzero_all_bytes": 12}
        png = {"sha256": "png", "width": 2, "height": 2}
        return {
            "event": {"kind": "completed"},
            "raw": raw,
            "png": png,
            "chain": {
                "surface_acquire": [{"outcome": "success"}],
                "adapter": [],
                "bevy": [
                    {
                        "event": "screenshot_prepared",
                        "capture_id": capture_id,
                        "buffer_id": capture_id,
                        "capture_texture_id": capture_id,
                    },
                    {"event": "queue_submitted", "copy_encoded": True},
                    {"event": "map_success", "copy_encoded": True},
                    {
                        "event": "image_returned",
                        "nonzero_all_bytes": 12,
                        "nonzero_rgb_bytes": 8,
                    },
                ],
            },
        }

    def test_classifies_successful_covered_capture_without_surface_failure(self):
        captures = [self.capture() for _ in range(4)]
        self.assertEqual(
            classify_capture_result(captures),
            "occlusion_did_not_cause_surface_failure",
        )

    def test_classifies_skip_copy_zero_readback(self):
        captures = [self.capture() for _ in range(4)]
        covered = captures[2]
        covered["event"] = {
            "kind": "rejected",
            "error": {"code": "screenshot_window_unavailable"},
        }
        covered["png"] = None
        covered["raw"] = {
            "sha256": "zero",
            "byte_length": 16,
            "nonzero_all_bytes": 0,
        }
        covered["chain"]["surface_acquire"] = [{"outcome": "occluded"}]
        covered["chain"]["bevy"] = [
            {"event": "screenshot_prepared"},
            {"event": "copy_skipped", "reason": "swap_chain_view_missing"},
            {"event": "queue_submitted", "copy_encoded": False},
            {"event": "map_success", "copy_encoded": False},
            {
                "event": "image_returned",
                "nonzero_all_bytes": 0,
                "nonzero_rgb_bytes": 0,
            },
        ]
        self.assertEqual(
            classify_capture_result(captures),
            "reproduced_skip_copy_zero_readback",
        )

    def test_does_not_infer_the_mac_path_from_a_black_image_alone(self):
        captures = [self.capture() for _ in range(4)]
        covered = captures[2]
        covered["raw"] = {
            "sha256": "zero",
            "byte_length": 16,
            "nonzero_all_bytes": 0,
        }
        image = next(
            row for row in covered["chain"]["bevy"] if row["event"] == "image_returned"
        )
        image["nonzero_all_bytes"] = 0
        image["nonzero_rgb_bytes"] = 0
        self.assertEqual(classify_capture_result(captures), "other_error_path_observed")

    def test_requires_partial_and_restored_to_match_the_baseline(self):
        captures = [self.capture() for _ in range(4)]
        captures[1] = copy.deepcopy(captures[1])
        captures[1]["raw"]["sha256"] = "different"
        self.assertEqual(classify_capture_result(captures), "other_error_path_observed")


if __name__ == "__main__":
    unittest.main()
