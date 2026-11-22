"""Local multimodal associative sensory memory for Voicebot (Carter).

Pairs an online surprise-gated fast-weight matrix (hippocampus: instant binding
of live room audio + screen snapshots + spoken labels, ignoring boring ambient
background) with a nonlinear SlowCortex MLP (neocortex: consolidated during
offline 'sleep' with replay and a regression-quiz rollback gate).

Privacy: ONLY compact 64-number embeddings and memory weights are stored in
`<VOICEBOT_HOME>/sensory_memory.npz` (mode 0600). Raw audio and screenshots are
never saved by this module.

Pure NumPy + stdlib (optional Pillow for decoding screenshot files), so CI tests
run with zero heavy installs and runtime overhead stays under a few milliseconds.
"""

from __future__ import annotations

from collections import deque
from dataclasses import dataclass
import hashlib
import json
import math
import os
import re
import threading
import time
from typing import Any

import numpy as np

RATE = 16000


def _unit(v: np.ndarray) -> np.ndarray:
    n = float(np.linalg.norm(v))
    return (v / (n + 1e-8)).astype(np.float32)


def _sigmoid(x: float) -> float:
    if x >= 0:
        z = math.exp(-x)
        return 1.0 / (1.0 + z)
    z = math.exp(x)
    return z / (1.0 + z)


def _fixed_proj(in_dim: int, out_dim: int, seed: int) -> np.ndarray:
    rng = np.random.default_rng(seed)
    return (rng.standard_normal((in_dim, out_dim)) / math.sqrt(in_dim)).astype(np.float32)


def _write_private_bytes(path: str, data: bytes) -> None:
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    tmp = path + ".tmp"
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    try:
        with os.fdopen(fd, "wb") as f:
            f.write(data)
    except Exception:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise
    os.replace(tmp, path)


# ---------- continuous room audio ring buffer ----------


class AudioRingTap:
    """Rolling 16 kHz mono buffer of live microphone audio (captures ambient
    non-speech sounds like buzzers, alarms, or hums, not only VAD speech)."""

    def __init__(self, max_seconds: float = 6.0, rate: int = RATE):
        self.rate = rate
        self.max_samples = int(max_seconds * rate)
        self._chunks: deque[np.ndarray] = deque()
        self._length = 0
        self._lock = threading.Lock()
        self._ambient_acc = 0

    def push(self, pcm: np.ndarray) -> bool:
        """Append mono float32 samples. Returns True every ~2s for passive ticks."""
        arr = np.asarray(pcm, dtype=np.float32).reshape(-1)
        if arr.size == 0:
            return False
        with self._lock:
            self._chunks.append(arr)
            self._length += arr.size
            while self._length > self.max_samples and len(self._chunks) > 1:
                self._length -= self._chunks.popleft().size
            self._ambient_acc += arr.size
            if self._ambient_acc >= 2 * self.rate:
                self._ambient_acc = 0
                return True
        return False

    def latest(self, seconds: float = 3.0) -> np.ndarray:
        """Return up to the newest `seconds` of mono 16 kHz audio."""
        want = int(seconds * self.rate)
        with self._lock:
            if not self._chunks:
                return np.zeros(0, dtype=np.float32)
            full = np.concatenate(list(self._chunks))
        return full[-want:] if full.size > want else full


# ---------- lightweight frozen encoders (pure NumPy + optional Pillow) ----------


class AudioEncoder:
    """48-band log-frequency FFT audio encoder (80-4000 Hz) with RMS silence floor."""

    def __init__(
        self,
        dim: int = 64,
        n_bands: int = 48,
        n_frames: int = 4,
        sample_rate: int = RATE,
        silence_rms: float = 0.03,
        seed: int = 202,
    ):
        self.dim = dim
        self.n_bands = n_bands
        self.n_frames = n_frames
        self.sample_rate = sample_rate
        self.silence_rms = silence_rms
        self.proj = _fixed_proj(n_bands * n_frames, dim, seed=seed)
        self.silence_vec = _unit(np.random.default_rng(seed + 1).standard_normal(dim))
        self._edges = np.geomspace(80.0, min(4000.0, sample_rate / 2.0), n_bands + 1)

    def encode(self, waveform: np.ndarray) -> np.ndarray:
        x = np.asarray(waveform, dtype=np.float32).reshape(-1)
        if x.size == 0:
            return self.silence_vec.copy()
        rms = float(np.sqrt(np.mean(x * x)))
        if rms < self.silence_rms * 0.5:
            return self.silence_vec.copy()
        energy = _sigmoid((rms - self.silence_rms) / (0.25 * self.silence_rms))

        chunk = max(64, x.size // self.n_frames)
        win = np.hanning(chunk).astype(np.float32)
        freqs = np.fft.rfftfreq(chunk, d=1.0 / self.sample_rate)
        feats = []
        for i in range(self.n_frames):
            seg = x[i * chunk : (i + 1) * chunk]
            if seg.size < chunk:
                seg = np.pad(seg, (0, chunk - seg.size))
            spec = np.abs(np.fft.rfft(seg * win))
            bands = np.zeros(self.n_bands, dtype=np.float32)
            for b in range(self.n_bands):
                mask = (freqs >= self._edges[b]) & (freqs < self._edges[b + 1])
                if np.any(mask):
                    bands[b] = float(spec[mask].mean())
            pooled = np.log1p(bands)
            pooled -= pooled.mean()
            feats.append(pooled)
        sig_vec = _unit(np.concatenate(feats) @ self.proj)
        return _unit(energy * sig_vec + (1.0 - energy) * self.silence_vec)


class VisionEncoder:
    """4x4 spatial RGB color + edge encoder with flat-frame contrast floor."""

    def __init__(self, dim: int = 64, grid: int = 4, flat_std: float = 0.035, seed: int = 303):
        self.dim = dim
        self.grid = grid
        self.flat_std = flat_std
        self.proj = _fixed_proj(grid * grid * 8, dim, seed=seed)
        self.ambient_vec = _unit(np.random.default_rng(seed + 1).standard_normal(dim))

    def encode_path(self, path: str) -> np.ndarray:
        """Load a JPEG/PNG screenshot via Pillow (resized small for speed) and encode."""
        from PIL import Image

        with Image.open(path) as im:
            rgb = im.convert("RGB").resize((64, 64))
            arr = np.asarray(rgb, dtype=np.float32) / 255.0
        return self.encode(arr)

    def encode(self, image: np.ndarray) -> np.ndarray:
        img = np.asarray(image, dtype=np.float32)
        if img.ndim != 3 or img.shape[2] != 3:
            raise ValueError(f"Expected [H, W, 3] RGB array, got {img.shape}")
        if img.max() > 1.5:
            img = img / 255.0
        contrast = float(img.std())
        if contrast < self.flat_std * 0.5:
            return self.ambient_vec.copy()
        energy = _sigmoid((contrast - self.flat_std) / (0.25 * self.flat_std))

        H, W, _ = img.shape
        g = self.grid
        gray = img.mean(axis=2)
        gx = np.abs(np.diff(gray, axis=1, append=gray[:, -1:]))
        gy = np.abs(np.diff(gray, axis=0, append=gray[-1:, :]))

        feats = []
        r_edges = np.linspace(0, H, g + 1, dtype=int)
        c_edges = np.linspace(0, W, g + 1, dtype=int)
        for r in range(g):
            for c in range(g):
                patch = img[r_edges[r] : max(r_edges[r] + 1, r_edges[r + 1]),
                            c_edges[c] : max(c_edges[c] + 1, c_edges[c + 1])]
                ex = gx[r_edges[r] : max(r_edges[r] + 1, r_edges[r + 1]),
                        c_edges[c] : max(c_edges[c] + 1, c_edges[c + 1])].mean()
                ey = gy[r_edges[r] : max(r_edges[r] + 1, r_edges[r + 1]),
                        c_edges[c] : max(c_edges[c] + 1, c_edges[c + 1])].mean()
                feats.extend(patch.mean(axis=(0, 1)).tolist())
                feats.extend(patch.std(axis=(0, 1)).tolist())
                feats.extend([float(ex), float(ey)])
        f = np.asarray(feats, dtype=np.float32)
        f -= f.mean()
        sig_vec = _unit(f @ self.proj)
        return _unit(energy * sig_vec + (1.0 - energy) * self.ambient_vec)


class TextEncoder:
    """Fast deterministic word + char-ngram signed projection (zero external models)."""

    def __init__(self, dim: int = 64):
        self.dim = dim

    def _hash_vec(self, token: str, weight: float) -> np.ndarray:
        digest = hashlib.blake2b(token.encode("utf-8"), digest_size=8).digest()
        seed = int.from_bytes(digest, "little")
        rng = np.random.default_rng(seed)
        return (rng.standard_normal(self.dim).astype(np.float32)) * weight

    def encode(self, text: str) -> np.ndarray:
        clean = " ".join(re.sub(r"[^a-z0-9 ]+", " ", text.lower()).split())
        if not clean:
            return np.zeros(self.dim, dtype=np.float32)
        acc = np.zeros(self.dim, dtype=np.float32)
        for word in clean.split():
            acc += self._hash_vec("w:" + word, 2.0)
            padded = f"<{word}>"
            for n in (3, 4):
                for i in range(max(0, len(padded) - n + 1)):
                    acc += self._hash_vec(f"c{n}:" + padded[i : i + n], 0.5)
        return _unit(acc)


# ---------- online fast weights + nonlinear slow cortex (pure NumPy) ----------


class FastAssociativeMemory:
    """Surprise-gated decaying delta-rule fast weights over 3 modalities."""

    def __init__(
        self,
        dim: int = 64,
        n_modalities: int = 3,
        lr: float = 0.45,
        decay: float = 0.99,
        surprise_thresh: float = 0.72,
        surprise_temp: float = 0.08,
    ):
        self.dim = dim
        self.n_modalities = n_modalities
        self.total_dim = dim * n_modalities
        self.lr = lr
        self.decay = decay
        self.surprise_thresh = surprise_thresh
        self.surprise_temp = surprise_temp

        mask = np.ones((self.total_dim, self.total_dim), dtype=np.float32)
        for m in range(n_modalities):
            lo, hi = m * dim, (m + 1) * dim
            mask[lo:hi, lo:hi] = 0.0
        self.cross_mask = mask
        self.M = np.zeros((self.total_dim, self.total_dim), dtype=np.float32)
        self.steps = 0

    def reset(self) -> None:
        self.M.fill(0.0)
        self.steps = 0

    def pack(self, modalities: list[np.ndarray | None]) -> np.ndarray:
        parts = []
        for x in modalities:
            if x is None:
                parts.append(np.zeros(self.dim, dtype=np.float32))
            else:
                parts.append(_unit(np.asarray(x, dtype=np.float32)))
        return np.concatenate(parts)

    def predict(self, modalities: list[np.ndarray | None]) -> list[np.ndarray]:
        z = self.pack(modalities)
        out = self.M @ z
        return [out[m * self.dim : (m + 1) * self.dim] for m in range(self.n_modalities)]

    def observe(self, modalities: list[np.ndarray | None]) -> dict[str, float]:
        z = self.pack(modalities)
        n_present = sum(1 for x in modalities if x is not None)
        if n_present < 2:
            self.steps += 1
            return {"surprise": 0.0, "gate": 0.0}

        pred = (self.M @ z) / max(1, n_present - 1)
        err = z - pred
        surprise = float(np.linalg.norm(err) / (np.linalg.norm(z) + 1e-8))
        gate = _sigmoid((surprise - self.surprise_thresh) / self.surprise_temp)
        eff_decay = 1.0 - (1.0 - self.decay) * gate
        step_lr = (self.lr * gate) / max(1, n_present - 1)
        self.M = (eff_decay * self.M + step_lr * np.outer(err, z)) * self.cross_mask
        self.steps += 1
        return {"surprise": surprise, "gate": gate}


def _gelu(x: np.ndarray) -> np.ndarray:
    return (0.5 * x * (1.0 + np.tanh(math.sqrt(2.0 / math.pi) * (x + 0.044715 * x * x * x)))).astype(np.float32)


def _gelu_grad(x: np.ndarray) -> np.ndarray:
    k = math.sqrt(2.0 / math.pi)
    u = k * (x + 0.044715 * x * x * x)
    t = np.tanh(u)
    du = k * (1.0 + 3.0 * 0.044715 * x * x)
    return (0.5 * (1.0 + t) + 0.5 * x * (1.0 - t * t) * du).astype(np.float32)


class SlowCortex:
    """2-layer GELU MLP in pure NumPy trained with Adam during offline sleep."""

    def __init__(self, dim: int = 64, n_modalities: int = 3, hidden_mult: int = 4, seed: int = 0):
        self.dim = dim
        self.n_modalities = n_modalities
        total = dim * n_modalities
        hidden = dim * hidden_mult
        rng = np.random.default_rng(seed)
        self.W1 = (rng.standard_normal((total, hidden)) * math.sqrt(2.0 / total)).astype(np.float32)
        self.b1 = np.zeros(hidden, dtype=np.float32)
        self.W2 = (rng.standard_normal((hidden, total)) * math.sqrt(2.0 / hidden)).astype(np.float32)
        self.b2 = np.zeros(total, dtype=np.float32)

    def params(self) -> dict[str, np.ndarray]:
        return {"W1": self.W1.copy(), "b1": self.b1.copy(), "W2": self.W2.copy(), "b2": self.b2.copy()}

    def load_params(self, p: dict[str, np.ndarray]) -> None:
        self.W1 = p["W1"].astype(np.float32).copy()
        self.b1 = p["b1"].astype(np.float32).copy()
        self.W2 = p["W2"].astype(np.float32).copy()
        self.b2 = p["b2"].astype(np.float32).copy()

    def forward(self, x: np.ndarray) -> np.ndarray:
        h = _gelu(x @ self.W1 + self.b1)
        return (h @ self.W2 + self.b2).astype(np.float32)

    def fit(self, cues: np.ndarray, targets: np.ndarray, masks: np.ndarray, steps: int = 120, lr: float = 4e-3,
            seed: int = 0) -> None:
        rng = np.random.default_rng(seed)
        names = ("W1", "b1", "W2", "b2")
        m = {k: np.zeros_like(getattr(self, k)) for k in names}
        v = {k: np.zeros_like(getattr(self, k)) for k in names}
        b1_adam, b2_adam, eps = 0.9, 0.999, 1e-8
        denom = float(masks.sum()) + 1e-8
        jitter_scale = 0.08 / math.sqrt(self.dim)

        for t in range(1, steps + 1):
            noise = rng.standard_normal(cues.shape).astype(np.float32) * jitter_scale
            x = cues + noise * (1.0 - masks)
            z1 = x @ self.W1 + self.b1
            h1 = _gelu(z1)
            out = h1 @ self.W2 + self.b2

            d_out = (2.0 * (out - targets) * masks) / denom
            grads = {
                "W2": h1.T @ d_out,
                "b2": d_out.sum(axis=0),
            }
            d_z1 = (d_out @ self.W2.T) * _gelu_grad(z1)
            grads["W1"] = x.T @ d_z1
            grads["b1"] = d_z1.sum(axis=0)

            for k in names:
                g = grads[k]
                m[k] = b1_adam * m[k] + (1.0 - b1_adam) * g
                v[k] = b2_adam * v[k] + (1.0 - b2_adam) * (g * g)
                m_hat = m[k] / (1.0 - b1_adam ** t)
                v_hat = v[k] / (1.0 - b2_adam ** t)
                setattr(self, k, (getattr(self, k) - lr * m_hat / (np.sqrt(v_hat) + eps)).astype(np.float32))


@dataclass
class SleepReport:
    committed: bool
    reason: str
    quiz_before: float
    quiz_after: float
    new_concepts: int
    replay_size: int


# ---------- unified Voicebot SensoryMemory manager ----------


class SensoryMemory:
    """Thread-safe persistent multimodal sensory memory for Carter / Voicebot.

    Modalities packed in order: [vision (0), audio (1), text/label (2)].
    """

    def __init__(
        self,
        home: str,
        dim: int = 64,
        confidence_floor: float = 0.42,
        max_quiz_drop: float = 0.08,
        surprise_write_min: float = 0.52,
        seed: int = 0,
    ):
        self.home = home
        self.dim = dim
        self.confidence_floor = confidence_floor
        self.max_quiz_drop = max_quiz_drop
        self.surprise_write_min = surprise_write_min
        self.npz_path = os.path.join(home, "sensory_memory.npz")
        self.meta_path = os.path.join(home, "sensory_memory.json")
        self._lock = threading.Lock()

        self.vision = VisionEncoder(dim=dim)
        self.audio = AudioEncoder(dim=dim)
        self.text = TextEncoder(dim=dim)
        self.fast = FastAssociativeMemory(dim=dim)
        self.cortex = SlowCortex(dim=dim, seed=seed)
        self.cortex_trained = False

        self.episode_buffer: list[np.ndarray] = []
        self.replay_bank: list[np.ndarray] = []
        self.labels: dict[str, np.ndarray] = {}
        self.label_meta: dict[str, dict[str, Any]] = {}
        self.sleep_logs: list[dict[str, Any]] = []

        self._bg_t = self.text.encode("ambient room silence")
        if os.path.isfile(self.npz_path) and os.path.isfile(self.meta_path):
            self.load()
        else:
            # Prime fast weights with ambient silence/room so quiet frames start familiar.
            for _ in range(12):
                self.fast.observe([self.vision.ambient_vec, self.audio.silence_vec, self._bg_t])

    def _encode_vision(self, image: np.ndarray | None = None, image_path: str | None = None) -> np.ndarray:
        if image is not None:
            return self.vision.encode(image)
        if image_path and os.path.isfile(image_path):
            return self.vision.encode_path(image_path)
        return self.vision.ambient_vec.copy()

    def _encode_audio(self, audio: np.ndarray | None = None) -> np.ndarray:
        if audio is not None and len(audio) > 0:
            return self.audio.encode(audio)
        return self.audio.silence_vec.copy()

    def observe_ambient(self, audio: np.ndarray | None = None, image: np.ndarray | None = None) -> dict[str, float]:
        """Passive live tick through the surprise gate (keeps background room familiar)."""
        av = self._encode_audio(audio)
        vv = self._encode_vision(image=image)
        with self._lock:
            return self.fast.observe([vv, av, self._bg_t])

    def remember(
        self,
        label: str,
        audio: np.ndarray | None = None,
        image: np.ndarray | None = None,
        image_path: str | None = None,
        repeats: int = 3,
    ) -> dict[str, Any]:
        """Bind current room sound and/or screen snapshot to a human concept label."""
        clean_label = " ".join(label.strip().split())
        if not clean_label:
            raise ValueError("Label must be non-empty.")

        vv = self._encode_vision(image=image, image_path=image_path)
        av = self._encode_audio(audio=audio)
        tv = self.text.encode(clean_label)

        has_vision = (image is not None) or bool(image_path and os.path.isfile(image_path))
        has_audio = bool(audio is not None and len(audio) > 0 and float(np.sqrt(np.mean(np.square(audio)))) >= self.audio.silence_rms * 0.5)

        with self._lock:
            self.labels[clean_label] = tv
            self.label_meta[clean_label] = {
                "updated_at": round(time.time(), 1),
                "has_vision": has_vision,
                "has_audio": has_audio,
            }
            surps, gates = [], []
            for _ in range(repeats):
                info = self.fast.observe([vv, av, tv])
                surps.append(info["surprise"])
                gates.append(info["gate"])
                if info["gate"] >= self.surprise_write_min:
                    self.episode_buffer.append(self.fast.pack([vv, av, tv]))
            # Ensure explicitly user-taught memories always enter the sleep buffer even if already partly familiar.
            if not any(g >= self.surprise_write_min for g in gates):
                self.episode_buffer.append(self.fast.pack([vv, av, tv]))
            self._save_locked()
            return {
                "label": clean_label,
                "has_vision": has_vision,
                "has_audio": has_audio,
                "mean_surprise": round(sum(surps) / len(surps), 3),
                "mean_gate": round(sum(gates) / len(gates), 3),
                "buffer_size": len(self.episode_buffer),
                "total_labels": len(self.labels),
            }

    def _predict_locked(self, modalities: list[np.ndarray | None]) -> list[np.ndarray]:
        fast_out = self.fast.predict(modalities)
        if not self.cortex_trained:
            return fast_out
        z_cue = self.fast.pack(modalities)[None, :]
        slow_raw = self.cortex.forward(z_cue)[0]
        slow_parts = [slow_raw[m * self.dim : (m + 1) * self.dim] for m in range(3)]
        return [
            np.zeros(self.dim, dtype=np.float32) if modalities[m] is not None else (slow_parts[m] + fast_out[m])
            for m in range(3)
        ]

    def _top_labels_locked(self, vec: np.ndarray, k: int = 3) -> list[dict[str, Any]]:
        if not self.labels or float(np.linalg.norm(vec)) < 1e-6:
            return []
        names = list(self.labels.keys())
        mat = np.stack([self.labels[n] for n in names])
        sims = mat @ _unit(vec)
        order = np.argsort(-sims)[:k]
        return [{"label": names[int(i)], "cos": round(float(sims[int(i)]), 3), **self.label_meta.get(names[int(i)], {})}
                for i in order]

    def recall(
        self,
        cue: str = "auto",
        query: str = "",
        audio: np.ndarray | None = None,
        image: np.ndarray | None = None,
        image_path: str | None = None,
    ) -> dict[str, Any]:
        """Cross-modal recall from live audio, live screen, or a spoken text query."""
        cue = cue.strip().lower()
        if cue in ("sound", "mic"):
            cue = "audio"
        elif cue in ("vision", "screenshot"):
            cue = "screen"
        if cue == "auto":
            if query.strip():
                cue = "text"
            elif audio is not None and len(audio) > 0 and float(np.sqrt(np.mean(np.square(audio)))) >= self.audio.silence_rms * 0.5:
                cue = "audio"
            elif image is not None or (image_path and os.path.isfile(image_path)):
                cue = "screen"
            else:
                cue = "audio"

        with self._lock:
            if not self.labels:
                return {"cue": cue, "familiar": False, "matches": [], "reason": "No sensory memories stored yet."}

            if cue == "audio":
                if audio is None or len(audio) == 0 or float(np.sqrt(np.mean(np.square(audio)))) < self.audio.silence_rms * 0.5:
                    return {"cue": "audio", "familiar": False, "matches": [], "reason": "Only quiet room silence was heard."}
                av = self.audio.encode(audio)
                _, _, pt = self._predict_locked([None, av, None])
                matches = self._top_labels_locked(pt)
                best = matches[0]["cos"] if matches else 0.0
                return {"cue": "audio", "familiar": best >= self.confidence_floor, "matches": matches}

            if cue == "screen":
                if image is None and not (image_path and os.path.isfile(image_path)):
                    return {"cue": "screen", "familiar": False, "matches": [], "reason": "No screen snapshot available."}
                vv = self._encode_vision(image=image, image_path=image_path)
                _, _, pt = self._predict_locked([vv, None, None])
                matches = self._top_labels_locked(pt)
                best = matches[0]["cos"] if matches else 0.0
                return {"cue": "screen", "familiar": best >= self.confidence_floor, "matches": matches}

            if cue == "text":
                if not query.strip():
                    return {"cue": "text", "familiar": False, "matches": [], "reason": "Empty query."}
                qt = self.text.encode(query)
                matches = self._top_labels_locked(qt)
                best = matches[0]["cos"] if matches else 0.0
                return {"cue": "text", "familiar": best >= 0.50, "matches": matches}

            raise ValueError(f"Unknown cue type: {cue}")

    def _dedup_locked(self, raw: list[np.ndarray], merge_cos: float = 0.88) -> list[np.ndarray]:
        clusters: list[tuple[np.ndarray, int]] = []
        for z in raw:
            best_i, best_s = -1, -1.0
            for i, (cent, _) in enumerate(clusters):
                s = float(np.dot(z, cent) / (np.linalg.norm(z) * np.linalg.norm(cent) + 1e-8))
                if s > best_s:
                    best_s, best_i = s, i
            if best_s >= merge_cos:
                cent, cnt = clusters[best_i]
                merged = (cent * cnt + z) / (cnt + 1)
                parts = [_unit(merged[m * self.dim : (m + 1) * self.dim]) for m in range(3)]
                clusters[best_i] = (np.concatenate(parts), cnt + 1)
            else:
                clusters.append((z.copy(), 1))
        return [c for c, _ in clusters]

    def _make_cue_pairs(self, triples: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
        d = self.dim
        cues, targets, masks = [], [], []
        for m in range(3):
            cue = np.zeros_like(triples)
            lo, hi = m * d, (m + 1) * d
            cue[:, lo:hi] = triples[:, lo:hi]
            mask = np.ones_like(triples)
            mask[:, lo:hi] = 0.0
            cues.append(cue)
            targets.append(triples)
            masks.append(mask)
        return np.concatenate(cues, axis=0), np.concatenate(targets, axis=0), np.concatenate(masks, axis=0)

    def _quiz_score_locked(self) -> float:
        if len(self.replay_bank) < 2 or not self.cortex_trained:
            return 1.0
        bank = np.stack(self.replay_bank, axis=0)
        d = self.dim
        protos = [bank[:, m * d : (m + 1) * d] for m in range(3)]
        hits, total = 0, 0
        for src in range(3):
            cue = np.zeros_like(bank)
            lo, hi = src * d, (src + 1) * d
            cue[:, lo:hi] = bank[:, lo:hi]
            out = self.cortex.forward(cue)
            for dst in range(3):
                if dst == src:
                    continue
                pred = out[:, dst * d : (dst + 1) * d]
                pred = pred / (np.linalg.norm(pred, axis=1, keepdims=True) + 1e-8)
                sims = pred @ protos[dst].T
                hits += int(np.sum(np.argmax(sims, axis=1) == np.arange(len(bank))))
                total += len(bank)
        return hits / max(1, total)

    def sleep(
        self,
        steps: int = 120,
        lr: float = 4e-3,
        max_replay: int = 256,
        use_replay: bool = True,
        force_episodes: list[np.ndarray] | None = None,
    ) -> SleepReport:
        """Offline sleep consolidation with replay + regression-quiz rollback gate."""
        with self._lock:
            new_eps = force_episodes if force_episodes is not None else self._dedup_locked(self.episode_buffer)
            if not new_eps:
                return SleepReport(False, "nothing new to consolidate", 1.0, 1.0, 0, len(self.replay_bank))

            snapshot = self.cortex.params()
            was_trained = self.cortex_trained
            quiz_before = self._quiz_score_locked()

            replay_slice = self.replay_bank[-max_replay:] if (use_replay and self.replay_bank) else []
            train_triples = np.stack(new_eps + replay_slice, axis=0)
            cues, targets, masks = self._make_cue_pairs(train_triples)

            self.cortex.fit(cues, targets, masks, steps=steps, lr=lr, seed=len(self.sleep_logs))
            self.cortex_trained = True
            quiz_after = self._quiz_score_locked()

            if len(self.replay_bank) >= 2 and (quiz_before - quiz_after) > self.max_quiz_drop:
                self.cortex.load_params(snapshot)
                self.cortex_trained = was_trained
                self.episode_buffer.clear()
                rep = SleepReport(
                    committed=False,
                    reason=f"rolled back: quiz dropped from {quiz_before:.2f} to {quiz_after:.2f}",
                    quiz_before=quiz_before,
                    quiz_after=quiz_after,
                    new_concepts=len(new_eps),
                    replay_size=len(replay_slice),
                )
                self.sleep_logs.append(rep.__dict__)
                self._save_locked()
                return rep

            self.replay_bank.extend(new_eps)
            self.episode_buffer.clear()
            self.fast.reset()
            for _ in range(8):
                self.fast.observe([self.vision.ambient_vec, self.audio.silence_vec, self._bg_t])
            rep = SleepReport(
                committed=True,
                reason="committed",
                quiz_before=quiz_before,
                quiz_after=quiz_after,
                new_concepts=len(new_eps),
                replay_size=len(replay_slice),
            )
            self.sleep_logs.append(rep.__dict__)
            self._save_locked()
            return rep

    def status(self) -> dict[str, Any]:
        with self._lock:
            return {
                "labels": sorted(self.labels.keys()),
                "total_labels": len(self.labels),
                "buffer_size": len(self.episode_buffer),
                "consolidated": len(self.replay_bank),
                "sleeps": len(self.sleep_logs),
                "fast_norm": round(float(np.linalg.norm(self.fast.M)), 3),
            }

    def _save_locked(self) -> None:
        import io

        arrays: dict[str, np.ndarray] = {
            "fast_M": self.fast.M,
            "W1": self.cortex.W1,
            "b1": self.cortex.b1,
            "W2": self.cortex.W2,
            "b2": self.cortex.b2,
        }
        if self.episode_buffer:
            arrays["episode_buffer"] = np.stack(self.episode_buffer, axis=0)
        if self.replay_bank:
            arrays["replay_bank"] = np.stack(self.replay_bank, axis=0)
        label_names = sorted(self.labels.keys())
        if label_names:
            arrays["label_vecs"] = np.stack([self.labels[k] for k in label_names], axis=0)

        buf = io.BytesIO()
        np.savez(buf, **arrays)
        _write_private_bytes(self.npz_path, buf.getvalue())

        meta = {
            "dim": self.dim,
            "fast_steps": self.fast.steps,
            "cortex_trained": self.cortex_trained,
            "label_names": label_names,
            "label_meta": self.label_meta,
            "sleep_logs": self.sleep_logs[-50:],
        }
        _write_private_bytes(self.meta_path, json.dumps(meta, indent=2).encode("utf-8"))

    def load(self) -> None:
        with self._lock:
            with open(self.meta_path, "r", encoding="utf-8") as f:
                meta = json.load(f)
            with np.load(self.npz_path) as data:
                self.fast.M = data["fast_M"].astype(np.float32).copy()
                self.cortex.load_params({k: data[k] for k in ("W1", "b1", "W2", "b2")})
                self.episode_buffer = (
                    [row.copy() for row in data["episode_buffer"]] if "episode_buffer" in data else []
                )
                self.replay_bank = (
                    [row.copy() for row in data["replay_bank"]] if "replay_bank" in data else []
                )
                self.labels = {}
                if "label_vecs" in data:
                    for name, vec in zip(meta.get("label_names", []), data["label_vecs"]):
                        self.labels[name] = vec.astype(np.float32).copy()
            self.fast.steps = int(meta.get("fast_steps", 0))
            self.cortex_trained = bool(meta.get("cortex_trained", False))
            self.label_meta = dict(meta.get("label_meta", {}))
            self.sleep_logs = list(meta.get("sleep_logs", []))
