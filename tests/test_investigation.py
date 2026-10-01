"""Acceptance-oracle regression: managed shutdown is not an unexpected endevent."""
import unittest

from investigation import managed_end


class ManagedEndTests(unittest.TestCase):
    def setUp(self):
        # Minimal terminal activity from investigation-rhyjnmux, positions 53/54.
        self.events = [
            {"command": "shutdown", "kind": "completed", "output": None,
             "request_id": 18446744073709551615},
            {"error": None, "kind": "lifecycle", "state": "Ended"},
        ]

    def test_managed_shutdown_needs_no_unexpected_end_event(self):
        managed_end(self.events)

    def test_missing_lifecycle_is_not_success(self):
        with self.assertRaises(AssertionError):
            managed_end(self.events[:1])

    def test_missing_shutdown_completion_is_not_success(self):
        with self.assertRaises(AssertionError):
            managed_end(self.events[1:])

    def test_unexpected_end_event_is_not_managed_success(self):
        with self.assertRaises(AssertionError):
            managed_end(self.events + [{"kind": "event", "event": {"kind": "ended"}}])


if __name__ == "__main__":
    unittest.main()
