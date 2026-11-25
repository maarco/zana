"""Bounded retry for local model downloads during Voicebot startup."""

import time


RETRY_DELAYS_SECONDS = (1, 2, 4, 8, 15, 30)


def retry_startup(operation, *, sleep=time.sleep, on_retry=None):
    """Retry a startup operation after transient failures with capped backoff."""
    for attempt in range(len(RETRY_DELAYS_SECONDS) + 1):
        try:
            return operation()
        except Exception as error:
            if attempt == len(RETRY_DELAYS_SECONDS):
                raise
            delay = RETRY_DELAYS_SECONDS[attempt]
            if on_retry is not None:
                on_retry(delay, attempt + 1, error)
            sleep(delay)

    raise AssertionError("startup retry loop always returns")
