import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from capture_scene_visibility_support import (
    correlate_capture_rows,
    read_json_lines,
    read_json_sources,
)


class ReadJsonLinesTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.path = Path(self.temporary.name) / "events.jsonl"

    def tearDown(self):
        self.temporary.cleanup()

    def test_reads_complete_lines(self):
        self.path.write_text('{"id":1}\n{"id":2}\n')
        self.assertEqual(read_json_lines(self.path), [{"id": 1}, {"id": 2}])

        self.path.write_text('{"id":3}')
        self.assertEqual(read_json_lines(self.path), [{"id": 3}])

    def test_temporarily_ignores_an_incomplete_last_line(self):
        self.path.write_text('{"id":1}\n{"id":')
        self.assertEqual(read_json_lines(self.path), [{"id": 1}])

    def test_reads_the_last_line_after_it_is_completed(self):
        self.path.write_text('{"id":1}\n{"id":')
        self.assertEqual(read_json_lines(self.path), [{"id": 1}])

        with self.path.open("a") as file:
            file.write('2}\n')
        self.assertEqual(read_json_lines(self.path), [{"id": 1}, {"id": 2}])

    def test_rejects_a_completed_invalid_line(self):
        self.path.write_text('{"id":1}\n{"id":}\n')
        with self.assertRaises(json.JSONDecodeError):
            read_json_lines(self.path)


class DiagnosticJsonlWriterTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.build = tempfile.TemporaryDirectory()
        directory = Path(__file__).resolve().parent
        cls.harness = Path(cls.build.name) / "visibility-log-harness"
        subprocess.run(
            [
                "rustc",
                "--edition=2024",
                str(directory / "capture_scene_visibility_log_harness.rs"),
                "-o",
                str(cls.harness),
            ],
            check=True,
        )

    @classmethod
    def tearDownClass(cls):
        cls.build.cleanup()

    def test_concurrent_threads_sources_and_processes_preserve_every_event(self):
        with tempfile.TemporaryDirectory() as temporary:
            log_dir = Path(temporary) / "logs"
            env = os.environ.copy()
            env["VISIBILITY_LOG_DIR"] = str(log_dir)
            process_count = 3
            threads = 6
            entries = 80
            processes = [
                subprocess.Popen(
                    [str(self.harness), f"process-{process}", str(threads), str(entries), "8192"],
                    env=env,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                )
                for process in range(process_count)
            ]
            pids = {process.pid for process in processes}
            for process in processes:
                stdout, stderr = process.communicate(timeout=60)
                self.assertEqual(process.returncode, 0, (stdout, stderr))

            paths = sorted(log_dir.glob("*.jsonl"))
            self.assertEqual(len(paths), process_count * 2)
            self.assertEqual(
                {int(path.stem.rsplit("-", 1)[1]) for path in paths},
                pids,
            )
            expected = {
                f"{source}-process-{process}-{thread}-{entry}"
                for process in range(process_count)
                for thread in range(threads)
                for entry in range(entries)
                for source in ["bevy_render" if thread % 2 == 0 else "woodpecker_adapter"]
            }
            rows = read_json_sources(log_dir)
            actual = [row["event_id"] for row in rows]
            self.assertEqual(len(actual), len(expected))
            self.assertEqual(len(actual), len(set(actual)))
            self.assertEqual(set(actual), expected)
            for path in paths:
                source = path.name.rsplit("-", 1)[0]
                for line in path.read_text().splitlines():
                    row = json.loads(line)
                    self.assertEqual(row["source"], source)
                    self.assertEqual(len(row["payload"]), 8192)

    def test_write_failure_terminates_the_diagnostic_writer(self):
        with tempfile.TemporaryDirectory() as temporary:
            not_a_directory = Path(temporary) / "file"
            not_a_directory.write_text("occupied")
            env = os.environ.copy()
            env["VISIBILITY_LOG_DIR"] = str(not_a_directory)
            completed = subprocess.run(
                [str(self.harness), "failure", "2", "1", "64"],
                env=env,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("diagnostic write failed", completed.stderr)


class CaptureCorrelationTest(unittest.TestCase):
    def test_correlates_sharded_sources_independent_of_arrival_order(self):
        session = "session-a"
        request = 41
        entity = "393v0"
        adapter = [
            {
                "source": "woodpecker_adapter",
                "session_id": session,
                "fields": {
                    "event": "adapter_response",
                    "request_id": request,
                    "outcome": "completed",
                },
            },
            {
                "source": "woodpecker_adapter",
                "session_id": session,
                "fields": {
                    "event": "adapter_capture_started",
                    "request_id": request,
                    "screenshot_entity": entity,
                },
            },
        ]
        bevy = [
            {
                "source": "bevy_render",
                "session_id": session,
                "event": "image_returned",
                "frame_id": 19,
                "capture_id": 7,
                "buffer_id": 8,
                "capture_texture_id": 9,
                "screenshot_entity": entity,
            },
            {
                "source": "bevy_render",
                "session_id": session,
                "event": "surface_acquire",
                "frame_id": 19,
                "outcome": "success",
            },
            {
                "source": "bevy_render",
                "session_id": session,
                "event": "screenshot_prepared",
                "frame_id": 19,
                "capture_id": 7,
                "buffer_id": 8,
                "capture_texture_id": 9,
                "screenshot_entity": entity,
            },
        ]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            (directory / "woodpecker_adapter-20.jsonl").write_text(
                "".join(json.dumps(row) + "\n" for row in adapter)
            )
            (directory / "bevy_render-20.jsonl").write_text(
                "".join(json.dumps(row) + "\n" for row in bevy)
            )
            rows = read_json_sources(directory)
            chain = correlate_capture_rows(rows, request, session)
        self.assertEqual(chain["screenshot_entity"], entity)
        self.assertEqual({row["capture_id"] for row in chain["bevy"] if "capture_id" in row}, {7})
        self.assertEqual([row["frame_id"] for row in chain["surface_acquire"]], [19])
        self.assertEqual(
            {row["fields"]["event"] for row in chain["adapter"]},
            {"adapter_capture_started", "adapter_response"},
        )


if __name__ == "__main__":
    unittest.main()
