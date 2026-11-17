"""Terminal jobs for Voicebot's run_terminal tool. Standard library only.

A command runs in its own process group with its output written to
<home>/jobs/<n>.log. Quick commands answer directly; slow ones keep running as a
job the agent can check or stop, and the agent is told when it finishes.
"""
import asyncio
import os
import signal
import time

from listening import clean


class Job:
    """One terminal command running in its own process group, its output going to a log file."""

    def __init__(self, job_id: int, command: str, proc, log_path: str, log_file, timeout: float):
        self.id, self.command, self.proc, self.log, self._log_file = job_id, command, proc, log_path, log_file
        self.timeout = timeout
        self.pid = proc.pid
        self.quiet = False  # stopped on purpose: nobody needs to hear that it ended
        self.started = time.time()
        self.ended: float | None = None
        self.exit_code: int | None = None
        self.task = asyncio.ensure_future(self._wait())

    async def _wait(self) -> None:
        try:
            self.exit_code = await asyncio.wait_for(self.proc.wait(), self.timeout)
        except asyncio.TimeoutError:
            self.stop()
            self.exit_code = await self.proc.wait()
            with open(self.log, "a") as f:
                f.write(f"\n[stopped after {self.timeout:.0f} seconds]\n")
        finally:
            self.ended = time.time()
            self._log_file.close()

    def stop(self, quiet: bool = False) -> None:
        """Kill the job's whole process group."""
        self.quiet = self.quiet or quiet
        try:
            os.killpg(self.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass

    def output(self, lines: int = 80) -> str:
        try:
            with open(self.log, errors="replace") as f:
                text = f.read()
        except OSError:
            return ""
        return clean("\n".join(text.splitlines()[-lines:])) if lines else ""

    def summary(self, lines: int = 40) -> str:
        took = (self.ended or time.time()) - self.started
        state = (f"finished with exit code {self.exit_code}" if self.ended else "still running")
        head = f"job {self.id} (process {self.pid}) {state} after {took:.0f} s: {self.command}\noutput file: {self.log}"
        out = self.output(lines)
        return head + (f"\nlatest output:\n{out}" if out else "")


class Jobs:
    """Terminal jobs of one agent process; numbers start at 1."""

    def __init__(self, home: str, timeout: float = 900):
        self.home, self.timeout = home, timeout
        self.jobs: dict[int, Job] = {}
        self.next_id = 1

    async def start(self, command: str):
        folder = os.path.join(self.home, "jobs")
        os.makedirs(folder, exist_ok=True)
        job_id = self.next_id
        self.next_id += 1
        log_path = os.path.join(folder, f"{job_id}.log")
        log_file = open(log_path, "wb")
        try:
            proc = await asyncio.create_subprocess_exec(
                "/bin/zsh", "-lc", command, cwd=os.path.expanduser(os.getenv("VOICEBOT_TERMINAL_DIR", "~")),
                stdout=log_file, stderr=asyncio.subprocess.STDOUT,
                start_new_session=True)  # its own process group, so stop() takes pipelines down too
        except OSError as exc:
            log_file.close()
            return f"Could not start the terminal: {exc}"
        job = Job(job_id, command, proc, log_path, log_file, self.timeout)
        self.jobs[job_id] = job
        return job

    def stop_all(self) -> None:
        """Stop every running job (the conversation is closing)."""
        for job in self.jobs.values():
            if job.ended is None:
                job.stop(quiet=True)

    def get(self, job_id) -> "Job | None":
        try:
            return self.jobs.get(int(job_id))
        except (TypeError, ValueError):
            return None
