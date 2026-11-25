"""Thinking sounds: cache staleness, random picks, silence trimming and the pause after a sound.
Standard library only (run in CI)."""
import array
import json
import math
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from fillers import DEFAULT_PHRASES, Fillers, chunks_with_gap, trim_silence  # noqa: E402

RATE = 24000


def tone(seconds: float, quiet_before: float = 0.0, quiet_after: float = 0.0) -> bytes:
    def pad(s):
        return array.array("h", [0] * int(s * RATE))

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

    def test_quiet_ending_and_internal_pause_survive_cache_and_playback(self):
        frame = RATE // 50
        silence = array.array("h", [0] * frame)
        loud = array.array("h", [10000] * frame)
        # The voiced ending is far below the old relative -35 dB cutoff and
        # resumes after an internal pause. It must remain byte-for-byte intact.
        ending = (array.array("h", [90] * frame * 6) + silence * 3
                  + array.array("h", [-11] * frame * 6))
        source = (silence * 4 + loud + ending + silence * 8).tobytes()
        expected = (loud + ending).tobytes()
        self.assertEqual(trim_silence(source, RATE, 1), expected)

        with tempfile.TemporaryDirectory() as home:
            f = Fillers(home)
            f.save("v", ["hmm"], [("hmm", source, RATE, 1)])
            path, _ = f.pick()
            pcm, rate, channels = f.read(path)
            self.assertEqual((pcm, rate, channels), (expected, RATE, 1))
            played = b"".join(chunks_with_gap(pcm, rate, channels))
            self.assertEqual(played[:len(expected)], expected)
            self.assertEqual(played[len(expected):], bytes(int(RATE * 0.5) * 2))

    def test_trim_keeps_quiet_tail_in_either_stereo_channel(self):
        frame = RATE // 50
        silence = array.array("h", [0, 0] * frame)
        loud = array.array("h", [10000, -10000] * frame)
        quiet_tail = array.array("h", [0, 1] * (frame + 13))
        source = (silence * 3 + loud + quiet_tail + silence * 4).tobytes()
        expected = (loud + quiet_tail).tobytes()
        trimmed = trim_silence(source, RATE, 2)
        self.assertEqual(trimmed[:len(expected)], expected)
        self.assertEqual(trimmed[len(expected):], bytes(len(trimmed) - len(expected)))
        self.assertLess(len(trimmed) - len(expected), frame * 2 * 2)

    def test_stereo_partial_last_chunk_keeps_every_sample_before_the_gap(self):
        channels = 2
        samples = array.array("h", [400, -400] * (RATE // 50 + 13))
        samples[-2:] = array.array("h", [7, -9])
        pcm = samples.tobytes()
        chunks = chunks_with_gap(pcm, RATE, channels, gap=0.5)
        size = RATE // 50 * channels * 2
        self.assertTrue(all(len(chunk) == size for chunk in chunks))
        output = b"".join(chunks)
        self.assertEqual(output[:len(pcm)], pcm)
        self.assertEqual(output[len(pcm):], bytes(len(output) - len(pcm)))
        self.assertGreaterEqual(len(output) - len(pcm), int(RATE * 0.5) * channels * 2)
        self.assertLess(len(output) - len(pcm), int(RATE * 0.5) * channels * 2 + size)

    def test_empty_and_silent_input_is_unchanged(self):
        for pcm in (b"", bytes(RATE * 2 // 10)):
            with self.subTest(length=len(pcm)):
                self.assertEqual(trim_silence(pcm, RATE, 1), pcm)

    def test_legacy_cache_is_regenerated_even_when_fresh(self):
        with tempfile.TemporaryDirectory() as home:
            f = Fillers(home)
            f.save("v", ["hmm"], [("hmm", tone(0.2), RATE, 1)])
            manifest = f.manifest()
            manifest.pop("audio_version", None)  # cache written before the tail fix
            with open(f.manifest_path, "w") as out:
                json.dump(manifest, out)
            made = manifest["made_at"]
            self.assertTrue(f.stale("v", ["hmm"], days=7, now=made))
            self.assertIsNotNone(f.pick())  # usable until the replacement is ready
            f.save("v", ["hmm"], [])  # a failed refresh must not discard the fallback
            self.assertEqual(f.manifest(), manifest)
            self.assertIsNotNone(f.pick())
            self.assertTrue(f.stale("v", ["hmm"], days=7, now=made))
            f.save("v", ["hmm"], [("hmm", tone(0.3), RATE, 1)])
            self.assertIn("audio_version", f.manifest())
            self.assertFalse(f.stale("v", ["hmm"], days=7))


if __name__ == "__main__":
    unittest.main()
