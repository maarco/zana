"""The local speech server's generation health budget."""
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from generation_health import GenerationHealth  # noqa: E402


class Clock:
    def __init__(self):
        self.now = 0.0

    def __call__(self):
        return self.now


class GenerationHealthTest(unittest.TestCase):
    def test_generation_is_healthy_at_sixty_seconds_and_stalls_after(self):
        clock = Clock()
        health = GenerationHealth(timeout_seconds=60, clock=clock)
        with health.generation():
            clock.now = 60
            self.assertFalse(health.stalled())
            clock.now = 60.001
            self.assertTrue(health.stalled())
        self.assertFalse(health.stalled())

    def test_no_generation_is_healthy(self):
        self.assertFalse(GenerationHealth().stalled())


if __name__ == "__main__":
    unittest.main()
