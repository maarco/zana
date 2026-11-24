"""Terminal jobs: output to a file, status, stop. Standard library only (run in CI)."""
import asyncio
import os
import shlex
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import jobs  # noqa: E402
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
                for _ in range(100):  # a login shell can take a moment to start
                    if "started" in job.output():
                        break
                    await asyncio.sleep(0.1)
                self.assertIn("still running", job.summary())
                self.assertIn("started", job.output())
                self.assertIs(jobs.get("1"), job)
                job.stop()
                await asyncio.wait_for(job.task, 5)  # the whole pipeline is gone
                self.assertNotEqual(job.exit_code, 0)
        asyncio.run(run())

    def test_stop_all_stops_running_jobs_quietly(self):
        async def run():
            with tempfile.TemporaryDirectory() as home:
                jobs = Jobs(home)
                job = await jobs.start("sleep 30")
                jobs.stop_all()
                await asyncio.wait_for(job.task, 5)
                self.assertTrue(job.quiet)
        asyncio.run(run())

    def test_timeout_stops_the_job(self):
        async def run():
            with tempfile.TemporaryDirectory() as home:
                job = await Jobs(home, timeout=0.5).start("sleep 30")
                await asyncio.wait_for(job.task, 5)
                self.assertIn("stopped after", job.output())
        asyncio.run(run())

    def test_noisy_job_log_is_capped_and_keeps_its_tail(self):
        async def run():
            with tempfile.TemporaryDirectory() as home:
                script = 'import sys; sys.stdout.write("x" * 20000); sys.stdout.write("TAIL\\n")'
                command = shlex.join([sys.executable, "-c", script])
                with patch.object(jobs, "MAX_JOB_LOG_BYTES", 1024):
                    job = await Jobs(home).start(command)
                    await asyncio.wait_for(job.task, 5)
                self.assertLessEqual(os.path.getsize(job.log), 1024)
                with open(job.log, "rb") as f:
                    self.assertTrue(f.read().endswith(b"TAIL\n"))
        asyncio.run(run())

    def test_shell_completion_is_not_held_open_by_background_child_pipe(self):
        async def run():
            with tempfile.TemporaryDirectory() as home:
                job = await Jobs(home, timeout=1).start("sleep 5 &")
                try:
                    done, _ = await asyncio.wait({job.task}, timeout=0.5)
                    self.assertIn(job.task, done, "exited shell job waits for background child's stdout EOF")
                finally:
                    job.stop()
                    await asyncio.gather(job.task, return_exceptions=True)
        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
