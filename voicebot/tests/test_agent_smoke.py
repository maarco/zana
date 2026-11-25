"""Test mode keeps Voicebot construction free of background model warmup."""
import os
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

try:
    from agent import Voicebot
except ModuleNotFoundError as exc:
    if exc.name != "livekit":
        raise
    DEPENDENCY_ERROR = str(exc)
else:
    DEPENDENCY_ERROR = None


@unittest.skipIf(DEPENDENCY_ERROR is not None, f"Voicebot dependencies unavailable: {DEPENDENCY_ERROR}")
class AgentSmokeTest(unittest.TestCase):
    def test_test_mode_does_not_start_voiceprint_warmup(self):
        with patch.dict(os.environ, {"VOICEBOT_TEST_MODE": "1"}), \
                patch("threading.Thread.start") as start:
            Voicebot({"fillers": {"on": False}}, lambda _attributes: None)

        start.assert_not_called()
