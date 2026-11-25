"""Voice print audio plumbing (numpy and scipy; the speaker model itself is exercised live)."""
import os
import sys
import unittest

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from voiceprint import RATE, VoiceTap, to_mono_16k  # noqa: E402


class ToMono16kTest(unittest.TestCase):
    def test_resamples_and_mixes_down(self):
        tone = (np.sin(np.arange(48000) / 10) * 8000).astype(np.int16)
        self.assertEqual(len(to_mono_16k(tone.tobytes(), 48000, 1)), RATE)
        stereo = np.repeat(tone[:24000], 2)  # 24,000 interleaved L/R frames at 24 kHz: one second
        out = to_mono_16k(stereo.tobytes(), 24000, 2)
        self.assertEqual(len(out), RATE)
        self.assertLessEqual(float(np.abs(out).max()), 1.0)


class SpeechSecondsTest(unittest.TestCase):
    def test_counts_speech_not_the_silence_around_it(self):
        from voiceprint import RATE, speech_seconds

        def quiet(s):
            return np.full(int(s * RATE), 1e-5, dtype=np.float32)

        tone = 0.3 * np.sin(np.arange(int(0.6 * RATE)) * 2 * np.pi * 220 / RATE).astype(np.float32)
        audio = np.concatenate([quiet(0.3), tone, quiet(0.9)])  # preroll, "go ahead", end-of-turn wait
        self.assertAlmostEqual(speech_seconds(audio), 0.6, delta=0.05)
        self.assertEqual(speech_seconds(np.zeros(0, dtype=np.float32)), 0.0)


class AdaptTest(unittest.TestCase):
    def test_adapt_moves_the_print_toward_the_voice_and_saves_it(self):
        import tempfile
        from voiceprint import VoicePrint
        with tempfile.TemporaryDirectory() as home:
            vp = VoicePrint(home)
            vp.reference = np.array([1.0, 0.0, 0.0])
            vp.adapt(np.array([0.0, 1.0, 0.0]), weight=0.1)
            self.assertAlmostEqual(float(np.linalg.norm(vp.reference)), 1.0, places=6)
            self.assertGreater(vp.reference[1], 0.0)
            self.assertGreater(vp.reference[0], vp.reference[1])
            self.assertTrue(np.allclose(np.load(os.path.join(home, "voiceprint.npy")), vp.reference))


class WindowsTest(unittest.TestCase):
    def test_always_fixed_three_second_windows(self):
        from voiceprint import RATE, WINDOW, windows
        for seconds, count in [(0.0, 1), (0.5, 1), (3.0, 1), (4.6, 2), (8.5, 3), (20.0, 3)]:
            w = windows(np.ones(int(seconds * RATE), dtype=np.float32))
            self.assertEqual(w.shape, (count, WINDOW), seconds)
        audio = np.arange(int(4.6 * RATE), dtype=np.float32)
        w = windows(audio)
        self.assertEqual(w[0][0], 0)              # the first window starts at the start
        self.assertEqual(w[-1][-1], audio[-1])    # the last one ends at the end


class TapCapTest(unittest.TestCase):
    def test_endless_speech_keeps_only_the_newest_seconds(self):
        from voiceprint import RATE, VoiceTap
        tap = VoiceTap(max_seconds=12)
        tap.start()
        for _ in range(60 * 100):  # a minute of 10 ms chunks without a pause
            tap.push(np.zeros(160, dtype=np.float32))
        self.assertLessEqual(tap.segment_len, 12 * RATE)
        self.assertEqual(len(tap.take()), 12 * RATE)


class VoiceTapTest(unittest.TestCase):
    def test_keeps_each_segment_with_preroll_and_clears_on_take(self):
        tap = VoiceTap(preroll_seconds=0.1)
        for _ in range(20):
            tap.push(np.full(160, 0.1, np.float32))  # silence before speech: only the pre-roll is kept
        tap.start()
        tap.push(np.full(RATE, 0.5, np.float32))
        tap.end()
        audio = tap.take()
        self.assertGreater(len(audio), RATE)
        self.assertLess(len(audio), RATE + int(0.2 * RATE))
        self.assertEqual(len(tap.take()), 0)

    def test_latest_is_the_segment_being_spoken_or_the_one_that_just_ended(self):
        tap = VoiceTap(preroll_seconds=0.01)
        self.assertEqual(len(tap.latest()), 0)
        tap.start()
        tap.push(np.ones(RATE // 2, np.float32))
        self.assertGreaterEqual(len(tap.latest()), RATE // 2)
        tap.end()
        self.assertGreaterEqual(len(tap.latest()), RATE // 2)  # still there after the speaker stops

    def test_pending_until_the_turn_takes_the_audio(self):
        tap = VoiceTap(preroll_seconds=0.01)
        self.assertFalse(tap.pending())
        tap.start()
        self.assertTrue(tap.pending())
        tap.push(np.ones(RATE // 10, np.float32))
        tap.end()
        self.assertTrue(tap.pending())  # heard, turn not taken yet
        tap.take()
        self.assertFalse(tap.pending())

    def test_a_segment_nobody_took_is_dropped_when_new_speech_starts_later(self):
        now = [0.0]
        tap = VoiceTap(preroll_seconds=0.01, stale_seconds=4, clock=lambda: now[0])
        tap.start()
        tap.push(np.ones(RATE, np.float32))  # an utterance whose turn was dropped (Voicebot kept talking)
        tap.end()
        now[0] = 2.0
        tap.start()  # within the same turn: kept
        tap.end()
        self.assertGreaterEqual(len(tap.take()), RATE)
        tap.start()
        tap.push(np.ones(RATE, np.float32))
        tap.end()
        now[0] = 9.0
        tap.start()  # a new turn, long after: the old audio doesn't blur its voice check
        tap.push(np.ones(RATE // 4, np.float32))
        self.assertLess(len(tap.take()), RATE // 2)

    def test_take_includes_a_segment_still_in_progress_and_caps_length(self):
        tap = VoiceTap(max_seconds=2)
        tap.start()
        for _ in range(5):
            tap.push(np.ones(RATE, np.float32))
        self.assertEqual(len(tap.take()), 2 * RATE)


if __name__ == "__main__":
    unittest.main()
