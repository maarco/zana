"""Voicebot's listening helpers: wake-word matching, the always-on transcript and its
unread mark, and output scrubbing. Standard library only, so CI tests them without
the speech stack."""
import datetime
import difflib
import os
import re

_ANSI = re.compile(r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07]*\x07|\x1b[@-Z\\-_]")
# KEY=value anywhere in a name (API_KEY, GH_TOKEN, db_password), plus long token-like strings.
_SECRETS = [re.compile(r"(?i)\w*(token|key|secret|passw(or)?d|auth)\w*\s*[=:]\s*\S+"), re.compile(r"[A-Za-z0-9_\-]{32,}")]


def clean(text: str, limit: int = 2500) -> str:
    """Strip terminal colors and anything that looks like a secret; keep the head and tail of long output."""
    text = _ANSI.sub("", text)
    for pat in _SECRETS:
        text = pat.sub("[hidden]", text)
    return text if len(text) <= limit else text[: limit // 2] + "\n...\n" + text[-limit // 2:]


def normalize(text: str) -> str:
    return " ".join(re.sub(r"[^a-z0-9 ]+", " ", text.lower()).split())


def heard_wake_word(text: str, wake: str, threshold: float = 0.8) -> bool:
    """True when the wake word is in what was heard, allowing for small mis-hearings."""
    said, wake_words = normalize(text).split(), normalize(wake).split()
    if not wake_words:
        return True
    target, n = " ".join(wake_words), len(wake_words)
    for i in range(max(1, len(said) - n + 1)):
        if difflib.SequenceMatcher(None, " ".join(said[i:i + n]), target).ratio() >= threshold:
            return True
    return False


GOODBYES = ("goodbye", "good bye", "bye", "bye bye", "stop listening", "that s all", "thats all",
            "good night", "see you later", "talk to you later", "go to sleep")


def is_goodbye(text: str, max_words: int = 5) -> bool:
    """A short sign-off ("goodbye", "stop listening", "that's all"); longer sentences that merely
    mention one ("I said goodbye to her") do not count."""
    words = normalize(text).split()
    if not words or len(words) > max_words:
        return False
    said = " " + " ".join(words) + " "
    return any(f" {phrase} " in said for phrase in GOODBYES)


_VOICEPRINT_VERBS = ("enroll", "set up", "setup", "record", "redo", "train", "start", "make", "create", "do my", "mode",
                     "enable", "turn on", "activate", "use")
_REDO_WORDS = ("redo", "again", "new", "reset", "update", "replace", "rerecord", "re record", "re enroll")


def wants_voiceprint(text: str) -> bool:
    """"Let's enroll my voice", "set up my voice print", "voice print mode" and the like."""
    said = " " + normalize(text) + " "
    if any(p in said for p in (" enroll my voice ", " enrol my voice ", " roll my voice ", " learn my voice ", " enroll me ")):
        return True
    if " voice print " not in said and " voiceprint " not in said:
        return False
    return any(f" {verb} " in said or said.strip().startswith(verb) for verb in _VOICEPRINT_VERBS)


def wants_redo(text: str) -> bool:
    """"Redo my voice print", "record a new one", "do it again"."""
    said = " " + normalize(text) + " "
    return any(f" {word} " in said for word in _REDO_WORDS)


# Words that mean the user is asking about one of Voicebot's own features; the agent
# gets that feature's how-to (with its live state) injected for the turn.
TOPICS = {
    "voiceprint": ("voice print", "voiceprint", "my voice", "enroll", "only answer me", "other voices", "other people"),
    "wake": ("wake word", "wake up word", "wakeword", "asleep", "sleeping", "wake you"),
    "goodbye": ("goodbye", "stop listening", "go to sleep", "mute"),
    "mcp": ("mcp", "tool server", "tool servers", "plugin", "plugins"),
    "terminal": ("terminal", "command", "shell", "run a"),
    "screen": ("screen", "screenshot", "what am i looking at", "window", "app am i"),
    "debug": ("debug", "what are you doing", "logs", "why didn t you", "why did you"),
    "transcript": ("transcript", "what did i say", "what was i saying", "earlier"),
    "prompt": ("system prompt", "your prompt", "the prompt", "your instructions", "instructions", "prompt file"),
    "name": ("your name", "call you", "rename", "who are you", "wake word"),
}


def topics(text: str) -> list[str]:
    said = " " + normalize(text) + " "
    return [name for name, words in TOPICS.items() if any(f" {w} " in said for w in words)]


class Transcript:
    """Append-only transcript of everything heard and said, with a read mark."""

    def __init__(self, home: str, unread_limit: int = 6000):
        self.path = os.path.join(home, "transcript.md")
        self.mark = os.path.join(home, "transcript.read")
        self.unread_limit = unread_limit
        os.makedirs(home, exist_ok=True)

    def record(self, speaker: str, text: str) -> None:
        if not text.strip():
            return
        stamp = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        with open(self.path, "a") as f:
            f.write(f"[{stamp}] {speaker}: {' '.join(text.split())}\n")

    def _size(self) -> int:
        try:
            return os.path.getsize(self.path)
        except OSError:
            return 0

    def tail(self, max_chars: int = 3000) -> str:
        """The newest whole lines of the transcript, up to `max_chars`."""
        try:
            with open(self.path, "rb") as f:
                f.seek(max(0, self._size() - max_chars))
                text = f.read().decode(errors="replace")
        except OSError:
            return ""
        if self._size() > max_chars and "\n" in text:
            text = text.split("\n", 1)[1]  # drop the partial first line
        return text.strip()

    def mark_read(self) -> None:
        with open(self.mark, "w") as f:
            f.write(str(self._size()))

    def take_unread(self) -> str:
        """Lines added since the last read (the newest `unread_limit` characters); marks them read."""
        try:
            start = int(open(self.mark).read().strip() or 0)
        except (OSError, ValueError):
            start = 0
        end = self._size()
        try:
            with open(self.path, "rb") as f:
                f.seek(min(start, end))
                unread = f.read(end - min(start, end)).decode(errors="replace")
        except OSError:
            unread = ""
        with open(self.mark, "w") as f:
            f.write(str(end))
        return unread[-self.unread_limit:]
