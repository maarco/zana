"""Track whether one local speech generation has exceeded its health budget."""
import threading
import time
from contextlib import contextmanager


class GenerationHealth:
    """A small, thread-safe clock for the generation currently holding the TTS lock."""

    def __init__(self, timeout_seconds: float = 60.0, clock=None):
        self.timeout_seconds = timeout_seconds
        self._clock = clock or time.monotonic
        self._state_lock = threading.Lock()
        self._started_at = None

    @contextmanager
    def generation(self):
        with self._state_lock:
            self._started_at = self._clock()
        try:
            yield
        finally:
            with self._state_lock:
                self._started_at = None

    def stalled(self) -> bool:
        with self._state_lock:
            started_at = self._started_at
        return started_at is not None and self._clock() - started_at > self.timeout_seconds
