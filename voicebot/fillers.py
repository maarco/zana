"""Thinking sounds ("ummm...", "hmmm...") that Voicebot plays while it works out a reply.
Standard library only (CI tests it without the speech stack).

The sounds are made once with Voicebot's own text-to-speech, cached as WAV files in
<home>/fillers/ with a manifest, and made again after `days`, or when the voice or the
lines change. Each play picks one at random (never the same one twice in a row), and a
short pause follows it so the reply does not run into it.
"""
import array
import json
import math
import os
import random
import time
import wave

DEFAULT_PHRASES = [
    "Ummmm...", "Hmmmm...", "Uhhhhh...", "Ooooookay...",
    "Mmm...", "Hmm, let me see...", "Uhh, okay...", "Ahh...", "Well...", "Hmm, okay...",
    "Umm, let's see...", "Right, so...", "Oh, hmm...", "Alright...", "Let me think...", "Mm-hmm...",
]
GAP_SECONDS = 0.5  # silence after a sound, so the reply does not run into it
AUDIO_VERSION = 1  # increment when cached audio processing changes


def trim_silence(pcm: bytes, rate: int, channels: int, floor_db: float = 35.0) -> bytes:
    """Trim the quiet lead-in and digital-silence padding, preserving the entire ending.

    The end of a spoken sound can fall well below a relative loudness threshold;
    only frames containing exact zeros are safe to remove from that end.
    """
    samples = array.array("h", pcm)
    if not samples or not any(samples):
        return pcm
    step = max(1, rate // 50) * channels
    frames = [samples[i:i + step] for i in range(0, len(samples), step)]
    level = [10 * math.log10(sum(s * s for s in f) / len(f) + 1e-9) for f in frames]
    threshold = max(level) - floor_db
    start = next((i for i, db in enumerate(level) if db > threshold), None)
    if start is None:
        return pcm
    end = len(frames)
    while end > start and not any(frames[end - 1]):
        end -= 1
    return samples[start * step:end * step].tobytes()


def chunks_with_gap(pcm: bytes, rate: int, channels: int, gap: float = GAP_SECONDS) -> list[bytes]:
    """The clip and then `gap` seconds of silence, as 20 ms chunks of 16-bit PCM (the last padded)."""
    size = max(1, rate // 50) * channels * 2
    data = pcm + b"\x00" * (int(rate * gap) * channels * 2)
    out = [data[i:i + size] for i in range(0, len(data), size)]
    if out and len(out[-1]) < size:
        out[-1] += b"\x00" * (size - len(out[-1]))
    return out


class Fillers:
    def __init__(self, home: str):
        self.folder = os.path.join(home, "fillers")
        self.manifest_path = os.path.join(self.folder, "manifest.json")
        self.last: str | None = None

    def manifest(self) -> dict:
        try:
            with open(self.manifest_path) as f:
                return json.load(f)
        except (OSError, ValueError):
            return {}

    def stale(self, voice: str, phrases: list[str], days: float, now: float | None = None) -> bool:
        """True when the sounds need regeneration: none yet, old audio processing, another
        voice or lines, a missing file, or older than `days`."""
        m = self.manifest()
        now = time.time() if now is None else now
        return (not m.get("clips") or m.get("audio_version") != AUDIO_VERSION
                or m.get("voice") != voice or m.get("phrases") != phrases
                or now - m.get("made_at", 0) > days * 86400
                or any(not os.path.isfile(os.path.join(self.folder, c["file"])) for c in m["clips"]))

    def save(self, voice: str, phrases: list[str], clips: list[tuple[str, bytes, int, int]]) -> None:
        """Write the clips (phrase, 16-bit PCM, sample rate, channels), then the manifest, then
        delete the previous set; a reader never sees a half-written set."""
        if not clips:  # a failed refresh must leave the old cache available
            return
        os.makedirs(self.folder, exist_ok=True)
        made_at = time.time()
        entries = []
        for i, (phrase, pcm, rate, channels) in enumerate(clips):
            name = f"{int(made_at)}-{i:02d}.wav"
            with wave.open(os.path.join(self.folder, name), "wb") as w:
                w.setnchannels(channels)
                w.setsampwidth(2)
                w.setframerate(rate)
                w.writeframes(trim_silence(pcm, rate, channels))
            entries.append({"file": name, "phrase": phrase})
        tmp = self.manifest_path + ".tmp"
        with open(tmp, "w") as f:
            json.dump({"audio_version": AUDIO_VERSION, "voice": voice, "phrases": phrases,
                       "made_at": made_at, "clips": entries}, f)
        os.replace(tmp, self.manifest_path)
        keep = {e["file"] for e in entries} | {"manifest.json"}
        for name in os.listdir(self.folder):
            if name.endswith(".wav") and name not in keep:
                os.remove(os.path.join(self.folder, name))

    def pick(self) -> tuple[str, str] | None:
        """A random sound (path, phrase), never the one played last."""
        clips = [c for c in self.manifest().get("clips", [])
                 if os.path.isfile(os.path.join(self.folder, c["file"]))]
        choices = [c for c in clips if c["file"] != self.last] or clips
        if not choices:
            return None
        clip = random.choice(choices)
        self.last = clip["file"]
        return os.path.join(self.folder, clip["file"]), clip["phrase"]

    @staticmethod
    def read(path: str) -> tuple[bytes, int, int]:
        with wave.open(path, "rb") as w:
            return w.readframes(w.getnframes()), w.getframerate(), w.getnchannels()
