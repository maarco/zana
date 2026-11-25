"""Speech-model startup retries are bounded and injectable for fixture tests."""

import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from startup_retry import RETRY_DELAYS_SECONDS, retry_startup  # noqa: E402


class StartupRetryTest(unittest.TestCase):
    def test_transient_failure_retries_until_network_returns(self):
        attempts = 0
        sleeps = []
        reports = []

        def operation():
            nonlocal attempts
            attempts += 1
            if attempts < 3:
                raise OSError("network unavailable")
            return "ready"

        result = retry_startup(
            operation,
            sleep=sleeps.append,
            on_retry=lambda delay, attempt, error: reports.append((delay, attempt, str(error))),
        )

        self.assertEqual(result, "ready")
        self.assertEqual(sleeps, [1, 2])
        self.assertEqual(reports[0][:2], (1, 1))
        self.assertEqual(attempts, 3)

    def test_failure_stops_after_the_capped_schedule(self):
        attempts = 0
        sleeps = []

        def operation():
            nonlocal attempts
            attempts += 1
            raise OSError("network unavailable")

        with self.assertRaisesRegex(OSError, "network unavailable"):
            retry_startup(operation, sleep=sleeps.append)

        self.assertEqual(sleeps, list(RETRY_DELAYS_SECONDS))
        self.assertEqual(attempts, len(RETRY_DELAYS_SECONDS) + 1)
