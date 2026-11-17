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

    def test_take_includes_a_segment_still_in_progress_and_caps_length(self):
        tap = VoiceTap(max_seconds=2)
        tap.start()
        for _ in range(5):
            tap.push(np.ones(RATE, np.float32))
        self.assertEqual(len(tap.take()), 2 * RATE)


if __name__ == "__main__":
    unittest.main()
