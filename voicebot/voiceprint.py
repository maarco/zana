"""Voice print: answer only the enrolled speaker.

VoiceTap keeps the microphone audio of each spoken turn (16 kHz mono); VoicePrint
turns audio into a speaker embedding (SpeechBrain ECAPA, run on the CPU) and
compares it with the saved one. Only the embedding (192 numbers) is stored, as
`voiceprint.npy` in the Voicebot folder; audio is never saved.
"""
import os
from collections import deque
from math import gcd

import numpy as np

RATE = 16000


def to_mono_16k(data, sample_rate: int, channels: int) -> np.ndarray:
    """int16 PCM (interleaved) to float32 mono at 16 kHz."""
    pcm = np.frombuffer(data, dtype=np.int16).astype(np.float32) / 32768.0
    if channels > 1:
        pcm = pcm.reshape(-1, channels).mean(axis=1)
    if sample_rate != RATE:
        from scipy.signal import resample_poly
        g = gcd(RATE, sample_rate)
        pcm = resample_poly(pcm, RATE // g, sample_rate // g).astype(np.float32)
    return pcm


class VoiceTap:
    """Collects the audio of each speech segment (marked by the session's VAD states)."""

    def __init__(self, max_seconds: float = 12.0, preroll_seconds: float = 0.3):
        self.max = int(max_seconds * RATE)
        self.preroll = deque(maxlen=max(1, int(preroll_seconds * RATE / 160)))  # ~10 ms chunks
        self.speaking = False
        self.segment: list[np.ndarray] = []
        self.done: list[np.ndarray] = []

    def push(self, pcm: np.ndarray) -> None:
        if self.speaking:
            self.segment.append(pcm)
        else:
            self.preroll.append(pcm)

    def start(self) -> None:
        if not self.speaking:
            self.speaking = True
            self.segment = list(self.preroll)

    def end(self) -> None:
        if self.speaking:
            self.speaking = False
            if self.segment:
                self.done.append(np.concatenate(self.segment))
            self.segment = []

    def take(self) -> np.ndarray:
        """Everything spoken since the last take (newest `max_seconds`)."""
        parts = self.done + ([np.concatenate(self.segment)] if self.segment else [])
        self.done = []
        if self.speaking:
            self.segment = []
        if not parts:
            return np.zeros(0, dtype=np.float32)
        audio = np.concatenate(parts)
        return audio[-self.max:]


class VoicePrint:
    """The enrolled speaker's embedding and a scorer for new audio."""

    MODEL = "speechbrain/spkrec-ecapa-voxceleb"

    def __init__(self, home: str):
        self.path = os.path.join(home, "voiceprint.npy")
        self.cache = os.path.join(home, "models", "spkrec-ecapa-voxceleb")
        self._encoder = None
        self.reference = np.load(self.path) if os.path.isfile(self.path) else None

    @property
    def enrolled(self) -> bool:
        return self.reference is not None

    def _embed(self, audio: np.ndarray) -> np.ndarray:
        import torch
        if self._encoder is None:
            from speechbrain.inference.speaker import EncoderClassifier
            self._encoder = EncoderClassifier.from_hparams(
                source=self.MODEL, savedir=self.cache, run_opts={"device": "cpu"})
        with torch.no_grad():
            e = self._encoder.encode_batch(torch.from_numpy(audio).unsqueeze(0)).squeeze().numpy()
        return e / np.linalg.norm(e)

    def warm(self) -> None:
        """Load the model ahead of the first turn."""
        self._embed(np.zeros(RATE, dtype=np.float32))

    def enroll(self, audio: np.ndarray) -> None:
        """Save the speaker's embedding from about ten seconds of their speech (in 3 s windows)."""
        win = 3 * RATE
        chunks = [audio[i:i + win] for i in range(0, max(1, len(audio) - win // 2), win)] or [audio]
        mean = np.mean([self._embed(c) for c in chunks if len(c) >= RATE], axis=0)
        self.reference = mean / np.linalg.norm(mean)
        os.makedirs(os.path.dirname(self.path), exist_ok=True)
        np.save(self.path, self.reference)

    def score(self, audio: np.ndarray) -> float:
        """Cosine similarity to the enrolled voice (same speaker is usually above 0.5)."""
        return self.score_and_embedding(audio)[0]

    def score_and_embedding(self, audio: np.ndarray) -> tuple[float, np.ndarray]:
        embedding = self._embed(audio)
        return float(embedding @ self.reference), embedding

    def adapt(self, embedding: np.ndarray, weight: float = 0.1) -> None:
        """Nudge the saved print toward a confidently matched turn, so it keeps learning the voice."""
        mixed = (1 - weight) * self.reference + weight * embedding
        self.reference = mixed / np.linalg.norm(mixed)
        np.save(self.path, self.reference)
