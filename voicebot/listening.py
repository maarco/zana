"""Voicebot's listening helpers: wake-word matching, the always-on transcript and its
unread mark, and output scrubbing. Standard library only, so CI tests them without
the speech stack."""
import datetime
import difflib
import json
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


_TOOL_NAME = re.compile(r"(?:tool_name:\s*|functions\.)([A-Za-z_][\w-]*)")
TOOL_CALL_MARKERS = ("<|tool_call", "<tool_call", "<function=")


def _arg_value(text: str):
    text = text.strip()
    try:
        return json.loads(text)
    except ValueError:
        return text


def _json_after(block: str, start: int) -> str:
    start = block.find("{", start)
    if start == -1:
        return "{}"
    try:
        return json.dumps(json.JSONDecoder().raw_decode(block, start)[0])
    except ValueError:
        return "{}"


def parse_text_tool_calls(text: str) -> list[tuple[str, str]]:
    """Tool calls a model wrote out as text instead of making them, as (name, JSON arguments).
    Formats: <|tool_call_begin|>tool_name: X, arguments: {...}<|tool_call_end|>;
    <|tool_call_begin|>functions.X:0<|tool_call_argument_begin|>{...}<|tool_call_end|> (Kimi);
    <tool_call>X<arg_key>k</arg_key><arg_value>v</arg_value></tool_call> (GLM);
    <tool_call>{"name": X, "arguments": {...}}</tool_call> (Hermes, Qwen);
    <function=X>{...} or <parameter=k>v</parameter></function> (Qwen coder)."""
    calls = []
    for block in re.findall(r"<\|tool_call_begin\|>(.*?)(?:<\|tool_call_end\|>|$)", text, re.S):
        name = _TOOL_NAME.search(block)
        if name:
            calls.append((name.group(1), _json_after(block, name.end())))
    for block in re.findall(r"<tool_call>(.*?)(?:</tool_call>|$)", text, re.S):
        block = block.strip()
        if block.startswith("{"):
            try:
                obj = json.JSONDecoder().raw_decode(block)[0]
            except ValueError:
                continue
            args = obj.get("arguments", obj.get("parameters", {}))
            if obj.get("name"):
                calls.append((obj["name"], json.dumps(_arg_value(args) if isinstance(args, str) else args)))
            continue
        name = re.match(r"[A-Za-z_][\w.-]*", block)
        if name:
            pairs = re.findall(r"<arg_key>(.*?)</arg_key>\s*<arg_value>(.*?)</arg_value>", block, re.S)
            calls.append((name.group(0), json.dumps({k.strip(): _arg_value(v) for k, v in pairs})))
    for name, body in re.findall(r"<function=([\w.-]+)>(.*?)(?:</function>|$)", text, re.S):
        params = re.findall(r"<parameter=([\w.-]+)>(.*?)</parameter>", body, re.S)
        calls.append((name, json.dumps({k: _arg_value(v) for k, v in params}) if params else _json_after(body, 0)))
    return calls


def split_say(arguments: str) -> tuple[str, str]:
    """The spoken "say" field of a tool call, and the arguments without it (the tool never sees it)."""
    try:
        args = json.loads(arguments or "{}")
    except ValueError:
        return "", arguments
    if not isinstance(args, dict) or "say" not in args:
        return "", arguments
    say = str(args.pop("say") or "").strip()
    return say, json.dumps(args)


def repeats(say: str, text: str, share: float = 0.6) -> bool:
    """True when what a tool call would say is mostly already in the text spoken in the same step."""
    words = normalize(say).split()
    have = set(normalize(text).split())
    return bool(words) and sum(w in have for w in words) / len(words) >= share


class ToolCallText:
    """Splits a streamed reply into what may be spoken and the tool-call text a model wrote out,
    which is held back from its first marker to the end of the reply. A "<" that may begin a
    marker cut off at a chunk boundary waits for the next chunk."""

    def __init__(self):
        self.held = ""
        self._pending = ""

    def feed(self, text: str) -> str:
        if self.held:
            self.held += text
            return ""
        text, self._pending = self._pending + text, ""
        start = 0
        while (i := text.find("<", start)) != -1:
            rest = text[i:]
            if rest.startswith(TOOL_CALL_MARKERS):
                self.held = rest
                return text[:i]
            if any(marker.startswith(rest) for marker in TOOL_CALL_MARKERS):
                self._pending = rest
                return text[:i]
            start = i + 1
        return text

    def finish(self) -> str:
        """Text held only because it might have begun a marker."""
        out, self._pending = self._pending, ""
        return out


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
    "transcript": ("transcript", "what did i say", "what was i saying", "earlier", "conversation", "conversations",
                   "last time", "yesterday", "compact", "logs"),
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

    def search(self, words: str, limit: int = 12) -> list[str]:
        """Transcript lines with every word (word starts match, case and punctuation ignored), each with the
        line after it (usually the reply); when no line has them all, the lines with the most. Newest last."""
        wanted = normalize(words).split()
        try:
            with open(self.path, errors="replace") as f:
                lines = f.read().splitlines()
        except OSError:
            return []
        if not wanted:
            return []
        scored = []
        for i, line in enumerate(lines):
            said = normalize(line.split("] ", 1)[-1]).split()
            hits = sum(any(w.startswith(want) for w in said) for want in wanted)
            if hits:
                scored.append((hits, i))
        if not scored:
            return []
        best = max(h for h, _ in scored)
        picked = sorted({j for h, i in scored if h == best for j in (i, i + 1) if j < len(lines)})
        return [lines[j] for j in picked][-2 * limit:]

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
