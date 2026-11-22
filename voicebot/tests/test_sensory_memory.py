"""Tests for Carter's local multimodal associative sensory memory (pure NumPy, CI-safe)."""

import math
import os
import stat
import sys
import tempfile
import unittest

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from listening import topics  # noqa: E402
from sensory_memory import (  # noqa: E402
    RATE,
    AudioEncoder,
    AudioRingTap,
    SensoryMemory,
    VisionEncoder,
)


def _tone(freq: float, seconds: float = 0.5, noise: float = 0.0, seed: int = 0) -> np.ndarray:
    rng = np.random.default_rng(seed)
    t = np.linspace(0.0, seconds, int(RATE * seconds), dtype=np.float32)
    wav = 0.55 * np.sin(2.0 * math.pi * freq * t) + 0.25 * np.sin(2.0 * math.pi * (freq * 1.5) * t)
    if noise > 0:
        wav = wav + noise * rng.standard_normal(wav.shape).astype(np.float32)
    return wav.astype(np.float32)


def _scene(idx: int, noise: float = 0.0, seed: int = 0) -> np.ndarray:
    rng = np.random.default_rng(seed)
    img = np.full((32, 32, 3), 0.1, dtype=np.float32)
    color = np.array([idx % 3 == 0, (idx + 1) % 3 == 0, (idx + 2) % 3 == 0], dtype=np.float32)
    r0 = (idx // 3) * 6 + 2
    c0 = (idx % 3) * 9 + 2
    img[r0 : r0 + 10, c0 : c0 + 10] = color
    if noise > 0:
        img = np.clip(img + noise * rng.standard_normal(img.shape).astype(np.float32), 0.0, 1.0)
    return img


class AudioRingTapTest(unittest.TestCase):
    def test_caps_buffer_and_triggers_passive_ticks(self):
        tap = AudioRingTap(max_seconds=2.0)
        ticks = 0
        for _ in range(30):  # 3 seconds in 0.1s chunks
            if tap.push(np.ones(RATE // 10, dtype=np.float32)):
                ticks += 1
        self.assertGreaterEqual(ticks, 1)
        self.assertEqual(len(tap.latest(5.0)), 2 * RATE)
        self.assertEqual(len(tap.latest(0.5)), RATE // 2)


class EncoderFloorTest(unittest.TestCase):
    def test_quiet_noise_and_flat_frames_map_stably(self):
        rng = np.random.default_rng(7)
        aenc = AudioEncoder(dim=64)
        q1 = aenc.encode(0.01 * rng.standard_normal(8000).astype(np.float32))
        q2 = aenc.encode(0.01 * rng.standard_normal(8000).astype(np.float32))
        self.assertGreater(float(np.dot(q1, q2)), 0.99)

        venc = VisionEncoder(dim=64)
        f1 = venc.encode(np.clip(0.1 + 0.01 * rng.standard_normal((32, 32, 3)), 0, 1).astype(np.float32))
        f2 = venc.encode(np.clip(0.1 + 0.01 * rng.standard_normal((32, 32, 3)), 0, 1).astype(np.float32))
        self.assertGreater(float(np.dot(f1, f2)), 0.99)

    def test_log_band_audio_separates_distinct_frequencies(self):
        aenc = AudioEncoder(dim=64)
        a1 = aenc.encode(_tone(260.0))
        a1_noisy = aenc.encode(_tone(260.0, noise=0.04, seed=1))
        a2 = aenc.encode(_tone(880.0))
        self.assertGreater(float(np.dot(a1, a1_noisy)), float(np.dot(a1, a2)) + 0.3)


class SensoryMemoryTest(unittest.TestCase):
    def test_remember_survives_boring_background_and_recalls_cross_modally(self):
        with tempfile.TemporaryDirectory() as home:
            mem = SensoryMemory(home, dim=64, seed=0)
            mem.remember("dryer buzzer", audio=_tone(310.0), image=_scene(0))
            mem.remember("kettle whistle", audio=_tone(1180.0), image=_scene(1))
            mem.remember("doorbell chime", audio=_tone(620.0), image=_scene(2))

            # 200 quiet ambient room frames should barely open the surprise gate.
            rng = np.random.default_rng(1)
            gates = [
                mem.observe_ambient(
                    audio=0.01 * rng.standard_normal(4000).astype(np.float32),
                    image=np.clip(0.1 + 0.01 * rng.standard_normal((32, 32, 3)), 0, 1).astype(np.float32),
                )["gate"]
                for _ in range(200)
            ]
            self.assertLess(sum(gates) / len(gates), 0.05)

            # Noisy held-out audio recalls the exact label.
            r_aud = mem.recall(cue="audio", audio=_tone(310.0, noise=0.05, seed=9))
            self.assertTrue(r_aud["familiar"], r_aud)
            self.assertEqual(r_aud["matches"][0]["label"], "dryer buzzer")

            # Noisy held-out screen recalls the exact label.
            r_vis = mem.recall(cue="screen", image=_scene(1, noise=0.06, seed=9))
            self.assertTrue(r_vis["familiar"], r_vis)
            self.assertEqual(r_vis["matches"][0]["label"], "kettle whistle")

            # Unrecognized text query reports familiar=False.
            r_unk = mem.recall(cue="text", query="quantum purple submarine")
            self.assertFalse(r_unk["familiar"], r_unk)

    def test_sleep_consolidates_persists_privately_and_rolls_back_poison(self):
        with tempfile.TemporaryDirectory() as home:
            mem = SensoryMemory(home, dim=64, seed=0)
            concepts = [
                ("dryer buzzer", 260.0, 0),
                ("kettle whistle", 520.0, 1),
                ("doorbell chime", 880.0, 2),
                ("coffee grinder", 1420.0, 3),
            ]
            for name, freq, sid in concepts:
                mem.remember(name, audio=_tone(freq), image=_scene(sid))

            rep = mem.sleep(steps=140)
            self.assertTrue(rep.committed, rep)
            self.assertEqual(mem.status()["buffer_size"], 0)
            self.assertGreaterEqual(mem.status()["consolidated"], 4)

            # Private file permissions (owner read/write only: 0600).
            mode = stat.S_IMODE(os.stat(mem.npz_path).st_mode)
            self.assertEqual(mode, 0o600)

            # Fresh instance loads from disk and recalls purely from SlowCortex.
            mem2 = SensoryMemory(home, dim=64, seed=99)
            r = mem2.recall(cue="audio", audio=_tone(880.0, noise=0.04, seed=5))
            self.assertTrue(r["familiar"], r)
            self.assertEqual(r["matches"][0]["label"], "doorbell chime")

            # Poisoned sleep (contradictory labels without replay) must be rolled back!
            before = mem2.cortex.params()
            rng = np.random.default_rng(123)
            poison = []
            for name, freq, sid in concepts:
                vv = mem2.vision.encode(_scene(sid))
                bad_a = rng.standard_normal(64).astype(np.float32)
                bad_t = rng.standard_normal(64).astype(np.float32)
                poison.append(mem2.fast.pack([vv, bad_a, bad_t]))
            bad_rep = mem2.sleep(steps=160, use_replay=False, force_episodes=poison)
            self.assertFalse(bad_rep.committed, bad_rep)
            self.assertIn("rolled back", bad_rep.reason)
            after = mem2.cortex.params()
            for k in before:
                self.assertTrue(np.array_equal(before[k], after[k]), f"weight {k} not restored")

    def test_time_shuffled_control_stays_at_chance(self):
        rng = np.random.default_rng(42)
        with tempfile.TemporaryDirectory() as home:
            mem = SensoryMemory(home, dim=32, seed=0)
            K = 10
            protos_v = [mem.vision.encode(_scene(i)) for i in range(K)]
            protos_a = [mem.audio.encode(_tone(220.0 * (1.22 ** i))) for i in range(K)]
            protos_t = [mem.text.encode(f"concept number {i} alpha") for i in range(K)]
            for i in range(K):
                mem.labels[f"concept number {i} alpha"] = protos_t[i]

            # Shuffle temporal pairing across modalities across 60 ticks.
            idx_v = rng.integers(0, K, size=60)
            idx_a = rng.integers(0, K, size=60)
            idx_t = rng.integers(0, K, size=60)
            for iv, ia, it in zip(idx_v, idx_a, idx_t):
                mem.fast.observe([protos_v[iv], protos_a[ia], protos_t[it]])

            hits = 0
            for i in range(K):
                _, _, pt = mem._predict_locked([protos_v[i], None, None])
                top = mem._top_labels_locked(pt, k=1)
                if top and top[0]["label"] == f"concept number {i} alpha":
                    hits += 1
            self.assertLessEqual(hits / K, 0.30, f"shuffled control beat chance: {hits}/{K}")

    def test_topics_detect_sensory_memory_phrases(self):
        self.assertIn("sensory", topics("what was that sound"))
        self.assertIn("sensory", topics("remember this sound as the dryer buzzer"))
        self.assertIn("sensory", topics("consolidate your sensory memory"))


if __name__ == "__main__":
    unittest.main()
