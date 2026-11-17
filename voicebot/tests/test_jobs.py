"""Terminal jobs: output to a file, status, stop. Standard library only (run in CI)."""
import asyncio
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from jobs import Jobs  # noqa: E402


class JobsTest(unittest.TestCase):
    def test_quick_command_finishes_with_output_and_exit_code(self):
        async def run():
            with tempfile.TemporaryDirectory() as home:
                jobs = Jobs(home)
                job = await jobs.start("echo hello-job; exit 3")
                await job.task
                self.assertEqual(job.exit_code, 3)
                self.assertIn("hello-job", job.output())
                self.assertTrue(os.path.isfile(os.path.join(home, "jobs", "1.log")))
                self.assertIn("finished with exit code 3", job.summary())
        asyncio.run(run())

    def test_slow_command_reports_running_and_can_be_stopped(self):
        async def run():
            with tempfile.TemporaryDirectory() as home:
                jobs = Jobs(home)
                job = await jobs.start("echo started; sleep 30 | cat")
                await asyncio.sleep(0.5)
                self.assertIn("still running", job.summary())
                self.assertIn("started", job.output())
                self.assertIs(jobs.get("1"), job)
                job.stop()
                await asyncio.wait_for(job.task, 5)  # the whole pipeline is gone
                self.assertNotEqual(job.exit_code, 0)
        asyncio.run(run())

    def test_timeout_stops_the_job(self):
        async def run():
            with tempfile.TemporaryDirectory() as home:
                job = await Jobs(home, timeout=0.5).start("sleep 30")
                await asyncio.wait_for(job.task, 5)
                self.assertIn("stopped after", job.output())
        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
