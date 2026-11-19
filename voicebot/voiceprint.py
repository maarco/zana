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
FRAME = RATE // 50  # 20 ms


def speech_seconds(audio: np.ndarray, floor_db: float = 35.0) -> float:
    """Seconds of actual speech in a turn: 20 ms frames within `floor_db` of the loudest one. The preroll
    and the silence before a turn ends don't count, so "go ahead" measures short."""
    n = len(audio) // FRAME
    if n == 0:
        return 0.0
    level = 10 * np.log10(np.mean(audio[: n * FRAME].reshape(n, FRAME) ** 2, axis=1) + 1e-12)
    return float(np.sum(level > level.max() - floor_db)) * FRAME / RATE


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


WINDOW = 3 * RATE  # the voice print always embeds fixed 3 s windows


def windows(audio: np.ndarray, most: int = 3) -> np.ndarray:
    """Up to `most` full 3 s windows, evenly spaced over the newest audio; shorter audio repeats to
    fill one window. Fixed shapes keep the speaker model's scratch memory steady: scoring whole turns
    of 0.5 to 12 s made the process keep about 530 MB more (976 MB against 547 MB after 300 turns),
    and scores stay within 0.04 of whole-turn scoring."""
    audio = audio[-most * WINDOW:]
    if len(audio) <= WINDOW:
        return np.resize(audio if len(audio) else np.zeros(1, np.float32), WINDOW)[None, :].astype(np.float32)
    starts = np.linspace(0, len(audio) - WINDOW, -(-len(audio) // WINDOW)).astype(int)
    return np.stack([audio[s:s + WINDOW] for s in starts]).astype(np.float32)


class VoiceTap:
    """Collects the audio of each speech segment (marked by the session's VAD states). Only the newest
    `max_seconds` are ever kept, also while speech never pauses (a TV or music in the room)."""

    def __init__(self, max_seconds: float = 12.0, preroll_seconds: float = 0.3):
        self.max = int(max_seconds * RATE)
        self.preroll = deque(maxlen=max(1, int(preroll_seconds * RATE / 160)))  # ~10 ms chunks
        self.speaking = False
        self.segment: deque[np.ndarray] = deque()
        self.segment_len = 0
        self.done: deque[np.ndarray] = deque()

    def push(self, pcm: np.ndarray) -> None:
        if self.speaking:
            self.segment.append(pcm)
            self.segment_len += len(pcm)
            while self.segment_len > self.max and len(self.segment) > 1:
                self.segment_len -= len(self.segment.popleft())
        else:
            self.preroll.append(pcm)

    def start(self) -> None:
        if not self.speaking:
            self.speaking = True
            self.segment = deque(self.preroll)
            self.segment_len = sum(len(c) for c in self.segment)

    def end(self) -> None:
        if self.speaking:
            self.speaking = False
            if self.segment:
                self.done.append(np.concatenate(self.segment))
                while sum(len(d) for d in self.done) > self.max and len(self.done) > 1:
                    self.done.popleft()
            self.segment = deque()
            self.segment_len = 0

    def take(self) -> np.ndarray:
        """Everything spoken since the last take (newest `max_seconds`)."""
        parts = list(self.done) + ([np.concatenate(self.segment)] if self.segment else [])
        self.done = deque()
        if self.speaking:
            self.segment = deque()
            self.segment_len = 0
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
        """The speaker embedding of a turn: the mean over its fixed 3 s windows."""
        import torch
        if self._encoder is None:
            from speechbrain.inference.speaker import EncoderClassifier
            torch.set_num_threads(2)  # small fixed inputs; leave the other cores to speech
            self._encoder = EncoderClassifier.from_hparams(
                source=self.MODEL, savedir=self.cache, run_opts={"device": "cpu"})
        with torch.inference_mode():
            e = self._encoder.encode_batch(torch.from_numpy(windows(audio))).squeeze(1).numpy().mean(axis=0)
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

    def score_and_embedding(self, audio: np.ndarray) -> tuple[float, np.ndarray, float]:
        """Similarity to the enrolled voice, the turn's embedding, and its seconds of actual speech."""
        embedding = self._embed(audio)
        return float(embedding @ self.reference), embedding, speech_seconds(audio)

    def adapt(self, embedding: np.ndarray, weight: float = 0.1) -> None:
        """Nudge the saved print toward a confidently matched turn, so it keeps learning the voice."""
        mixed = (1 - weight) * self.reference + weight * embedding
        self.reference = mixed / np.linalg.norm(mixed)
        np.save(self.path, self.reference)
