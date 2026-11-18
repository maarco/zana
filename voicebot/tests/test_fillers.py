"""Thinking sounds: cache staleness, random picks, silence trimming and the pause after a sound.
Standard library only (run in CI)."""
import array
import math
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from fillers import DEFAULT_PHRASES, Fillers, chunks_with_gap, trim_silence  # noqa: E402

RATE = 24000


def tone(seconds: float, quiet_before: float = 0.0, quiet_after: float = 0.0) -> bytes:
    pad = lambda s: array.array("h", [0] * int(s * RATE))
    t = array.array("h", (int(8000 * math.sin(i * 2 * math.pi * 220 / RATE)) for i in range(int(seconds * RATE))))
    return (pad(quiet_before) + t + pad(quiet_after)).tobytes()


class FillersTest(unittest.TestCase):
    def test_sixteen_default_sounds_including_the_four_asked_for(self):
        self.assertEqual(len(DEFAULT_PHRASES), 16)
        for phrase in ["Ummmm...", "Hmmmm...", "Uhhhhh...", "Ooooookay..."]:
            self.assertIn(phrase, DEFAULT_PHRASES)

    def test_made_once_then_remade_when_old_or_changed(self):
        with tempfile.TemporaryDirectory() as home:
            f = Fillers(home)
            self.assertTrue(f.stale("en-Carter_man", ["Hmm..."], days=7))
            f.save("en-Carter_man", ["Hmm..."], [("Hmm...", tone(0.4), RATE, 1)])
            made = f.manifest()["made_at"]
            self.assertFalse(f.stale("en-Carter_man", ["Hmm..."], days=7, now=made + 86400))
            self.assertTrue(f.stale("en-Carter_man", ["Hmm..."], days=7, now=made + 8 * 86400))
            self.assertTrue(f.stale("en-Emma_woman", ["Hmm..."], days=7, now=made))
            self.assertTrue(f.stale("en-Carter_man", ["Umm..."], days=7, now=made))

    def test_picks_at_random_but_never_the_same_twice_in_a_row(self):
        with tempfile.TemporaryDirectory() as home:
            f = Fillers(home)
            f.save("v", ["a", "b", "c"], [(p, tone(0.2), RATE, 1) for p in "abc"])
            picks = [f.pick()[1] for _ in range(40)]
            self.assertTrue(all(x != y for x, y in zip(picks, picks[1:])))
            self.assertEqual(set(picks), {"a", "b", "c"})

    def test_remaking_replaces_the_old_files(self):
        with tempfile.TemporaryDirectory() as home:
            f = Fillers(home)
            f.save("v", ["a"], [("a", tone(0.2), RATE, 1)])
            f.save("v", ["b"], [("b", tone(0.2), RATE, 1)])
            wavs = [n for n in os.listdir(f.folder) if n.endswith(".wav")]
            self.assertEqual(len(wavs), 1)
            self.assertEqual(f.pick()[1], "b")

    def test_trims_padding_and_adds_the_pause_after(self):
        pcm = tone(0.5, quiet_before=0.3, quiet_after=0.6)
        trimmed = trim_silence(pcm, RATE, 1)
        self.assertAlmostEqual(len(trimmed) / 2 / RATE, 0.5, delta=0.03)
        chunks = chunks_with_gap(trimmed, RATE, 1, gap=0.5)
        self.assertTrue(all(len(c) == RATE // 50 * 2 for c in chunks))
        self.assertAlmostEqual(len(chunks) * 0.02, 1.0, delta=0.03)  # sound plus half a second of pause


if __name__ == "__main__":
    unittest.main()
