"""The chat model asks OpenRouter for a bounded reply, so a low balance still covers it (#50)."""
import importlib.util
import json
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

try:
    from livekit.agents import Agent  # noqa: F401 - only to see whether the Voicebot venv is in use
except ModuleNotFoundError as exc:
    if exc.name != "livekit":
        raise
    DEPENDENCY_ERROR = str(exc)
else:
    TEST_HOME = tempfile.TemporaryDirectory(prefix="zana-llm-test-")
    spec = importlib.util.spec_from_file_location(
        "_zana_llm_agent", os.path.join(os.path.dirname(os.path.dirname(__file__)), "agent.py"))
    agent_module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = agent_module
    with patch.dict(os.environ, {"VOICEBOT_HOME": TEST_HOME.name}):
        spec.loader.exec_module(agent_module)
    DEPENDENCY_ERROR = None


def tearDownModule():
    if DEPENDENCY_ERROR is None:
        TEST_HOME.cleanup()


@unittest.skipIf(DEPENDENCY_ERROR, f"needs the Voicebot venv: {DEPENDENCY_ERROR}")
class ReplyCapTest(unittest.TestCase):
    def cap(self, base_url):
        with open(os.path.join(TEST_HOME.name, "config.json"), "w") as f:
            json.dump({"llm": {"model": "some/model", "base_url": base_url, "api_key": "k"}}, f)
        return agent_module.make_llm()._opts.max_completion_tokens

    def test_openrouter_replies_are_capped(self):
        self.assertEqual(self.cap("https://openrouter.ai/api/v1"), agent_module.MAX_REPLY_TOKENS)

    def test_other_providers_get_no_cap(self):
        self.assertNotIsInstance(self.cap("https://api.openai.com/v1"), int)


if __name__ == "__main__":
    unittest.main()
