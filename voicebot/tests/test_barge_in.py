"""Only the enrolled voice interrupts Voicebot: check_barge_in and maybe_barge_in against stand-in session, tap
and voice print. Needs the Voicebot packages (skipped without them, like test_compaction)."""
import asyncio
import importlib.util
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

try:
    from livekit.agents import Agent
except ModuleNotFoundError as exc:
    if exc.name != "livekit":
        raise
    DEPENDENCY_ERROR = str(exc)
else:
    TEST_HOME = tempfile.TemporaryDirectory(prefix="zana-barge-in-test-")
    spec = importlib.util.spec_from_file_location(
        "_zana_barge_in_agent", os.path.join(os.path.dirname(os.path.dirname(__file__)), "agent.py"))
    agent_module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = agent_module
    with patch.dict(os.environ, {"VOICEBOT_HOME": TEST_HOME.name}):
        spec.loader.exec_module(agent_module)
    DEPENDENCY_ERROR = None


def tearDownModule():
    if DEPENDENCY_ERROR is None:
        TEST_HOME.cleanup()


class FakeSpeech:
    def __init__(self):
        self.interrupted, self.finished = False, False

    def done(self):
        return self.finished


class FakeSession:
    def __init__(self, replying=True):
        self.user_state, self.interrupts = "speaking", []
        self.current_speech = FakeSpeech() if replying else None

    def interrupt(self, force=False):
        self.interrupts.append(force)
        self.current_speech.interrupted = True


class FakeTap:
    """Each look adds 0.2 s of speech; after `ends_after` looks the speaker stops."""

    def __init__(self, session, ends_after):
        self.session, self.ends_after, self.looks = session, ends_after, 0

    def latest(self):
        self.looks += 1
        if self.looks >= self.ends_after:
            self.session.user_state = "listening"
        return np.zeros(int(self.looks * 0.2 * agent_module.RATE), np.float32)


class FakePrint:
    def __init__(self, score, enrolled=True):
        self.score, self.enrolled = score, enrolled

    def score_and_embedding(self, audio):
        return self.score, None, len(audio) / agent_module.RATE


@unittest.skipIf(DEPENDENCY_ERROR is not None, f"Voicebot dependencies unavailable: {DEPENDENCY_ERROR}")
class BargeInTest(unittest.TestCase):
    def bot(self, score, ends_after=4, enrolled=True, replying=True):
        fake = FakeSession(replying)

        class Bot(agent_module.Voicebot):
            session = property(lambda self: fake)

            def __init__(self):
                Agent.__init__(self, instructions="test")
                self.voice_gated, self.barging, self.updating = True, False, False
                self.threshold, self.short_threshold = 0.4, 0.28
                self.tap, self.voiceprint = FakeTap(fake, ends_after), FakePrint(score, enrolled)
        return Bot(), fake

    def run_check(self, bot):
        bot.barging = True
        asyncio.run(asyncio.wait_for(bot.check_barge_in(), 10))
        self.assertFalse(bot.barging)

    def test_the_enrolled_voice_interrupts(self):
        bot, session = self.bot(score=0.6)
        self.run_check(bot)
        self.assertEqual(session.interrupts, [True])

    def test_another_voice_never_interrupts_and_voicebot_keeps_going(self):
        bot, session = self.bot(score=0.1, ends_after=6)
        self.run_check(bot)
        self.assertEqual(session.interrupts, [])

    def test_a_long_other_voice_is_let_go_while_it_still_talks(self):
        bot, session = self.bot(score=0.1, ends_after=10_000)  # a TV that never stops
        self.run_check(bot)  # returns by itself once SHORT_TURN seconds were checked
        self.assertEqual(session.interrupts, [])
        self.assertEqual(session.user_state, "speaking")

    def test_a_voice_print_cleared_mid_conversation_lets_anyone_interrupt(self):
        bot, session = self.bot(score=0.0, enrolled=False)
        self.run_check(bot)
        self.assertEqual(session.interrupts, [True])

    def test_checks_start_only_while_a_reply_is_under_way(self):
        for replying, gated, updating in ((False, True, False), (True, False, False), (True, True, True)):
            bot, _ = self.bot(score=0.9, replying=replying)
            bot.voice_gated, bot.updating = gated, updating
            bot.maybe_barge_in()
            self.assertFalse(bot.barging, (replying, gated, updating))

    def test_checking_stops_when_the_reply_ends(self):
        bot, session = self.bot(score=0.1, ends_after=10_000)
        session.current_speech.finished = True
        self.run_check(bot)
        self.assertEqual(session.interrupts, [])


if __name__ == "__main__":
    unittest.main()
