"""Terminal jobs: output to a file, status, stop. Standard library only (run in CI)."""
import asyncio
import os
import shlex
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import jobs
from jobs import Jobs, conversation_end_warning  # noqa: E402


class JobsTest(unittest.TestCase):
    def test_long_job_warning_names_the_job_and_conversation_end(self):
        warning = conversation_end_warning(12).lower()
        self.assertIn("job 12", warning)
        self.assertIn("stopped", warning)
        self.assertIn("conversation ends", warning)
        self.assertIn("cannot be resumed", warning)

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
                self.assertTrue(Path(job.log).read_bytes().endswith(b"TAIL\n"))
        asyncio.run(run())

    def test_job_output_log_write_failure_is_reported(self):
        async def run():
            with tempfile.TemporaryDirectory() as home:
                command = shlex.join([sys.executable, "-c", "print('JOB OUTPUT')"])
                with patch.object(jobs, "append_bounded", side_effect=PermissionError("denied")):
                    job = await Jobs(home).start(command)
                    await asyncio.wait_for(job.task, 5)
                return job.output(), job.exit_code

        output, exit_code = asyncio.run(run())
        self.assertEqual(exit_code, 0)
        self.assertIn("Could not save terminal output", output)

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

    def test_shell_exit_completes_job_while_process_wait_is_held_by_stdout(self):
        async def run():
            class PipeHeldProcess:
                def __init__(self):
                    self.pid = 2**30
                    self.returncode = None
                    self.stdout = asyncio.StreamReader()
                    self.exited = asyncio.get_running_loop().create_future()

                async def wait(self):
                    await self.exited
                    return self.returncode

            with tempfile.TemporaryDirectory() as home:
                proc = PipeHeldProcess()
                job = jobs.Job(1, "background child", proc, os.path.join(home, "job.log"), 1)

                async def shell_exits():
                    await asyncio.sleep(0.01)
                    proc.returncode = 0

                exit_task = asyncio.create_task(shell_exits())
                try:
                    done, _ = await asyncio.wait({job.task}, timeout=0.5)
                    self.assertIn(job.task, done, "shell exit must not wait for inherited stdout EOF")
                    self.assertEqual(job.exit_code, 0)
                finally:
                    proc.stdout.feed_eof()
                    if not proc.exited.done():
                        proc.exited.set_result(proc.returncode)
                    await asyncio.gather(job.task, exit_task, return_exceptions=True)
        asyncio.run(run())

    def test_shell_exit_closes_output_drain_when_background_child_keeps_pipe_open(self):
        async def run():
            with tempfile.TemporaryDirectory() as home:
                job = await Jobs(home, timeout=1).start("sleep 60 &")
                try:
                    await asyncio.wait_for(job.task, 2)
                    self.assertTrue(
                        job._output_task.done(),
                        "completed shell leaves its inherited stdout drain task pending",
                    )
                finally:
                    job.stop()
                    await asyncio.gather(job.task, job._output_task, return_exceptions=True)
        asyncio.run(run())

    def test_shell_exit_keeps_capturing_late_background_child_output(self):
        async def run():
            with tempfile.TemporaryDirectory() as home:
                job = await Jobs(home, timeout=2).start("printf shell; (sleep 0.4; printf late) &")
                try:
                    await asyncio.wait_for(job.task, 1)
                    await asyncio.sleep(0.55)
                    self.assertIn(b"late", Path(job.log).read_bytes())
                finally:
                    job.stop()
                    tasks = [job.task, job._output_task]
                    late_output_task = getattr(job, "_late_output_task", None)
                    if late_output_task is not None:
                        tasks.append(late_output_task)
                    await asyncio.gather(*tasks, return_exceptions=True)
        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
