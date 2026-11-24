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
from retention import append_bounded

# zsh as a login shell on macOS (the user's PATH and aliases); bash or sh elsewhere, such as CI
SHELL = next(([path, "-lc"] for path in ("/bin/zsh", "/bin/bash") if os.path.exists(path)), ["/bin/sh", "-c"])
MAX_JOB_LOG_BYTES = 5 * 1024 * 1024
LOG_READ_BYTES = 64 * 1024
OUTPUT_DRAIN_GRACE_SECONDS = 0.1


class Job:
    """One terminal command running in its own process group, its output going to a log file."""

    def __init__(self, job_id: int, command: str, proc, log_path: str, timeout: float):
        self.id, self.command, self.proc, self.log = job_id, command, proc, log_path
        self.timeout = timeout
        self.pid = proc.pid
        self.quiet = False  # stopped on purpose: nobody needs to hear that it ended
        self.started = time.time()
        self.ended: float | None = None
        self.exit_code: int | None = None
        self._output_task = asyncio.create_task(self._capture_output())
        self.task = asyncio.ensure_future(self._wait())

    async def _capture_output(self) -> None:
        """Drain the child pipe continuously, keeping only the newest bounded part of its log."""
        if self.proc.stdout is None:
            return
        while chunk := await self.proc.stdout.read(LOG_READ_BYTES):
            try:
                append_bounded(self.log, chunk, MAX_JOB_LOG_BYTES)
            except OSError:
                # Keep draining even if this disk write fails, so a noisy child cannot deadlock.
                pass

    async def _wait(self) -> None:
        timed_out = False
        try:
            loop = asyncio.get_running_loop()
            deadline = loop.time() + self.timeout
            while self.proc.returncode is None:
                remaining = deadline - loop.time()
                if remaining <= 0:
                    self.stop()
                    timed_out = True
                    while self.proc.returncode is None:
                        await asyncio.sleep(0.01)
                    break
                await asyncio.sleep(min(0.01, remaining))
            self.exit_code = self.proc.returncode
        finally:
            # The shell can exit while a background child still holds its stdout pipe.
            # Finish the job on the shell's exit, while allowing a short drain for final output.
            drained, _ = await asyncio.wait({self._output_task}, timeout=OUTPUT_DRAIN_GRACE_SECONDS)
            if self._output_task in drained:
                await self._output_task
            else:
                process_transport = getattr(self.proc, "_transport", None)
                get_pipe_transport = getattr(process_transport, "get_pipe_transport", None)
                pipe_transport = get_pipe_transport(1) if get_pipe_transport else None
                if pipe_transport is not None:
                    pipe_transport.close()
                self._output_task.cancel()
                await asyncio.gather(self._output_task, return_exceptions=True)
            if timed_out:
                try:
                    append_bounded(self.log, f"\n[stopped after {self.timeout:.0f} seconds]\n".encode(),
                                   MAX_JOB_LOG_BYTES)
                except OSError:
                    pass
            self.ended = time.time()

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
        with open(log_path, "wb"):
            pass
        try:
            proc = await asyncio.create_subprocess_exec(
                *SHELL, command, cwd=os.path.expanduser(os.getenv("VOICEBOT_TERMINAL_DIR", "~")),
                stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.STDOUT,
                start_new_session=True)  # its own process group, so stop() takes pipelines down too
        except OSError as exc:
            return f"Could not start the terminal: {exc}"
        job = Job(job_id, command, proc, log_path, self.timeout)
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
