"""Terminal jobs for Voicebot's run_terminal tool. Standard library only.

A command runs in its own process group with its output written to a unique file
under <home>/jobs. Quick commands answer directly; slow ones keep running as a
job the agent can check or stop, and the agent is told when it finishes.
"""
import asyncio
import os
import re
import signal
import time
import uuid

from listening import clean
from retention import append_bounded

# zsh as a login shell on macOS (the user's PATH and aliases); bash or sh elsewhere, such as CI
SHELL = next(([path, "-lc"] for path in ("/bin/zsh", "/bin/bash") if os.path.exists(path)), ["/bin/sh", "-c"])
MAX_JOB_LOG_BYTES = 5 * 1024 * 1024
LOG_READ_BYTES = 64 * 1024
OUTPUT_DRAIN_GRACE_SECONDS = 0.1
OUTPUT_LATE_DRAIN_SECONDS = 0.75
_SAFE_CONVERSATION_ID = re.compile(r"^[A-Za-z0-9_-]{1,64}$")


def conversation_end_warning(job_id: int) -> str:
    """Disclose exactly which long-running job is cancelled when its session ends."""
    return (
        f"Job {job_id} will be stopped if this conversation ends, including when "
        "you close the orb or change Voicebot settings. It cannot be resumed in a "
        "later conversation; keep this conversation open if you want it to finish."
    )


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
        self._output_error = False
        self._output_pending = False
        self._output_task = asyncio.create_task(self._capture_output())
        self._late_output_task: asyncio.Task | None = None
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
                self._output_error = True

    def _close_output_pipe(self) -> None:
        transport = getattr(self.proc, "_transport", None)
        pipe = transport.get_pipe_transport(1) if transport is not None else None
        if pipe is not None:
            pipe.close()

    async def _capture_late_output(self) -> None:
        """Capture a short tail from descendants, then release an inherited stdout pipe."""
        try:
            await asyncio.wait_for(self._capture_output(), timeout=OUTPUT_LATE_DRAIN_SECONDS)
        except TimeoutError:
            self._output_error = True
            self._close_output_pipe()
        except asyncio.CancelledError:
            self._output_error = True
            raise
        finally:
            self._output_pending = False
            self._close_output_pipe()

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
                self._close_output_pipe()
            else:
                self._output_task.cancel()
                await asyncio.gather(self._output_task, return_exceptions=True)
                # The shell is done, but a background child may still own stdout.
                # Keep a bounded tail window without holding a pipe open for the child's lifetime.
                self._output_pending = True
                self._late_output_task = asyncio.create_task(self._capture_late_output())
            if timed_out:
                try:
                    append_bounded(self.log, f"\n[stopped after {self.timeout:.0f} seconds]\n".encode(),
                                   MAX_JOB_LOG_BYTES)
                except OSError:
                    self._output_error = True
            self.ended = time.time()

    def stop(self, quiet: bool = False) -> None:
        """Kill the job's whole process group."""
        self.quiet = self.quiet or quiet
        try:
            os.killpg(self.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
        if self._late_output_task is not None:
            self._late_output_task.cancel()

    def output(self, lines: int = 80) -> str:
        try:
            with open(self.log, errors="replace") as f:
                text = f.read()
        except OSError:
            text = ""
            self._output_error = True
        if self._output_pending:
            warning = "[Background process output may be missing while collection is still active.]"
            text = f"{text}\n{warning}" if text else warning
        elif self._output_error:
            warning = "[Could not save terminal output fully; some output may be missing.]"
            text = f"{text}\n{warning}" if text else warning
        return clean("\n".join(text.splitlines()[-lines:])) if lines else ""

    def summary(self, lines: int = 40) -> str:
        took = (self.ended or time.time()) - self.started
        state = (f"finished with exit code {self.exit_code}" if self.ended else "still running")
        head = f"job {self.id} (process {self.pid}) {state} after {took:.0f} s: {self.command}\noutput file: {self.log}"
        out = self.output(lines)
        return head + (f"\nlatest output:\n{out}" if out else "")


class Jobs:
    """Terminal jobs of one conversation; numbers are local, log names are unique."""

    def __init__(self, home: str, timeout: float = 900, conversation_id: str | None = None):
        self.home, self.timeout = home, timeout
        self.conversation_id = (conversation_id if conversation_id and _SAFE_CONVERSATION_ID.fullmatch(conversation_id)
                                else uuid.uuid4().hex)
        self.jobs: dict[int, Job] = {}
        self.next_id = 1
        self.log_paths: set[str] = set()

    async def start(self, command: str):
        folder = os.path.join(self.home, "jobs")
        os.makedirs(folder, exist_ok=True)
        job_id = self.next_id
        self.next_id += 1
        log_path = os.path.join(folder, f"{self.conversation_id}-job-{job_id}.log")
        log_fd = os.open(log_path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        os.close(log_fd)
        self.log_paths.add(log_path)
        try:
            proc = await asyncio.create_subprocess_exec(
                *SHELL, command, cwd=os.path.expanduser(os.getenv("VOICEBOT_TERMINAL_DIR", "~")),
                stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.STDOUT,
                start_new_session=True)  # its own process group, so stop() takes pipelines down too
        except OSError as exc:
            self.log_paths.discard(log_path)
            try:
                os.remove(log_path)
            except FileNotFoundError:
                pass
            return f"Could not start the terminal: {exc}"
        job = Job(job_id, command, proc, log_path, self.timeout)
        self.jobs[job_id] = job
        return job

    def stop_all(self) -> None:
        """Stop every running job (the conversation is closing)."""
        for job in self.jobs.values():
            if job.ended is None:
                job.stop(quiet=True)

    async def cleanup(self) -> None:
        """Stop and drain this conversation, then remove only its tracked artifacts."""
        self.stop_all()
        await asyncio.gather(*(job.task for job in self.jobs.values()), return_exceptions=True)
        # _wait can create the late drain only after the shell exits. Await it
        # after the main tasks so no descendant can recreate a removed log.
        late_tasks = [job._late_output_task for job in self.jobs.values()
                      if job._late_output_task is not None]
        if late_tasks:
            await asyncio.gather(*late_tasks, return_exceptions=True)
        for job in self.jobs.values():
            job._close_output_pipe()
        for path in self.log_paths:
            for artifact in (path, path + ".tmp"):
                try:
                    os.remove(artifact)
                except FileNotFoundError:
                    pass
        self.log_paths.clear()

    def get(self, job_id) -> "Job | None":
        try:
            return self.jobs.get(int(job_id))
        except (TypeError, ValueError):
            return None
