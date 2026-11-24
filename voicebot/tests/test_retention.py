"""Bounded local logs and prompt-history retention. Standard library only."""
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from retention import append_bounded, append_bounded_jsonl, prune_prompt_history


class BoundedAppendTest(unittest.TestCase):
    def test_keeps_newest_complete_lines_under_limit(self):
        with tempfile.TemporaryDirectory() as home:
            path = os.path.join(home, "logs", "debug.jsonl")
            for i in range(40):
                append_bounded(path, f"line-{i:02d}\n".encode(), 100, preserve_lines=True)
            with open(path, "rb") as f:
                content = f.read()
            self.assertLessEqual(len(content), 100)
            self.assertTrue(content.endswith(b"line-39\n"))
            self.assertTrue(all(line.startswith(b"line-") for line in content.splitlines()))
            self.assertNotIn(b"line-00", content)

    def test_jsonl_event_does_not_join_a_crash_truncated_previous_line(self):
        with tempfile.TemporaryDirectory() as home:
            path = os.path.join(home, "debug.jsonl")
            with open(path, "wb") as f:
                f.write(b"{\"partial\":")
            append_bounded_jsonl(path, {"t": 1, "kind": "after-crash"}, 1024)
            with open(path, "rb") as f:
                lines = f.read().splitlines()
            self.assertEqual(lines[0], b'{"partial":')
            self.assertEqual(json.loads(lines[1])["kind"], "after-crash")

    def test_oversized_json_event_is_omitted_without_breaking_jsonl(self):
        with tempfile.TemporaryDirectory() as home:
            path = os.path.join(home, "debug.jsonl")
            append_bounded_jsonl(path, {"t": 1, "kind": "test", "text": "x" * 4096}, 1024)
            append_bounded_jsonl(path, {"t": 2, "kind": "next"}, 1024)
            with open(path) as f:
                events = [json.loads(line) for line in f]
            self.assertEqual(events[0]["kind"], "test")
            self.assertTrue(events[0]["truncated"])
            self.assertEqual(events[-1]["kind"], "next")
            self.assertLessEqual(os.path.getsize(path), 1024)


class PromptHistoryTest(unittest.TestCase):
    def test_prunes_oldest_prompt_versions_and_keeps_unrelated_files(self):
        with tempfile.TemporaryDirectory() as history:
            for i in range(25):
                with open(os.path.join(history, f"system_prompt-20261124-0000{i:02d}.md"), "w") as f:
                    f.write(str(i))
            with open(os.path.join(history, "notes.md"), "w") as f:
                f.write("keep")
            self.assertEqual(prune_prompt_history(history, keep=20), 5)
            names = sorted(name for name in os.listdir(history) if name.startswith("system_prompt-"))
            self.assertEqual(len(names), 20)
            self.assertTrue(names[0].endswith("000005.md"))
            self.assertTrue(os.path.isfile(os.path.join(history, "notes.md")))

    def test_missing_history_is_harmless(self):
        with tempfile.TemporaryDirectory() as home:
            self.assertEqual(prune_prompt_history(os.path.join(home, "missing")), 0)


if __name__ == "__main__":
    unittest.main()
