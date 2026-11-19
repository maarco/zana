"""Make LLM text safe to speak: no URLs, paths, code, backslashes or stray symbols reach the TTS."""
import re
from typing import AsyncIterable

_ONES = "zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen".split()
_TENS = "_ _ twenty thirty forty fifty sixty seventy eighty ninety".split()
_SCALES = [(10**9, "billion"), (10**6, "million"), (1000, "thousand"), (100, "hundred")]


def number_words(n: int) -> str:
    """Spell out a non-negative integer: 350 -> three hundred fifty."""
    if n < 20:
        return _ONES[n]
    if n < 100:
        return _TENS[n // 10] + ("" if n % 10 == 0 else " " + _ONES[n % 10])
    for size, name in _SCALES:
        if n >= size:
            head, rest = divmod(n, size)
            return f"{number_words(head)} {name}" + ("" if rest == 0 else " " + number_words(rest))
    return str(n)


def _digits(s: str) -> str:
    return " ".join(_ONES[int(d)] for d in s)


def _say_number(m: re.Match) -> str:
    text = m.group(0)
    if text.count(".") >= 2:                       # versions: 0.1.3 -> zero point one point three
        return " point ".join(number_words(int(p)) if len(p) < 13 else _digits(p) for p in text.split("."))
    whole, _, frac = text.partition(".")
    whole = whole.replace(",", "")
    said = number_words(int(whole)) if len(whole) < 13 else _digits(whole)
    return said + (" point " + _digits(frac) if frac else "")


_RULES = [
    (re.compile(r"```.*?(```|$)", re.S), " "),            # fenced code blocks
    (re.compile(r"`([^`]*)`"), r"\1"),                    # inline code: keep the words
    (re.compile(r"\[([^\]]*)\]\([^)]*\)"), r"\1"),        # markdown links: keep the label
    (re.compile(r"https?://\S+|www\.\S+"), " a link "),   # bare URLs
    (re.compile(r"(?<!\w)(~|\.{1,2})?(/[\w.@-]+){2,}/?"), " a file path "),  # unix paths
    (re.compile(r"\b(?=[0-9a-f]*\d)(?=[0-9a-f]*[a-f])[0-9a-f]{7,40}\b"), " a commit "),  # git hashes, not plain numbers
    (re.compile(r"\\[nt]"), " "),                         # escaped newlines/tabs
    (re.compile(r"\bv(?=\d+\.\d)"), "version "),            # v0.1.3 -> version 0.1.3 (spelled out next)
    (re.compile(r"(\d+(?:\.\d+)?)\s?%"), r"\1 percent"),         # 80% -> 80 percent (spelled out next)
    (re.compile(r"(?<![\w.])\d{1,3}(?:,\d{3})+(?:\.\d+)?(?![\w.])|(?<![\w.])\d+(?:\.\d+)*(?![\w])"), _say_number),
    (re.compile(r'["_]'), " "),                          # JSON quotes and snake_case keys read as words
    (re.compile(r"[\\*_#`|<>{}\[\]^~]"), " "),            # leftover markup symbols
    (re.compile(r"^\s*[-+]\s+", re.M), ""),               # list bullets
    (re.compile(r"&"), " and "),
    (re.compile(r"[ \t]{2,}"), " "),
]


def speakable(text: str) -> str:
    for pat, rep in _RULES:
        text = pat.sub(rep, text)
    return text


TOOL_SAY = "\x1f"  # starts a tool call's spoken description; the one-sentence cap never cuts it


def join_sentences(parts: list[str]) -> str:
    """Join spoken descriptions so each ends in exactly one stop: a period is added only when none is there."""
    parts = [p.strip() for p in parts if p.strip()]
    return " ".join(p if p.endswith((".", "!", "?")) else p + "." for p in parts)


async def one_sentence(text: AsyncIterable[str]) -> AsyncIterable[str]:
    """TTS text transform: speak one sentence per reply. A short opener like "I'm listening."
    or "Sure." is skipped when a real sentence follows, so the answer is what gets said.
    A tool call's description (after TOOL_SAY) is always spoken, in full, after the answer."""
    buf, done, pos, describing = "", False, 0, False
    async for chunk in text:
        if describing:
            yield chunk.replace(TOOL_SAY, "")
            continue
        answer, mark, description = chunk.partition(TOOL_SAY)
        if not done:  # past the answer's sentence, keep draining so the reply finishes cleanly
            buf += answer
            while True:
                m = re.compile(r"[.!?](?=\s)").search(buf, pos)
                if not m:
                    break
                sentence = buf[pos:m.end()].strip()
                pos = m.end()
                if len(sentence.split()) > 3:
                    yield sentence
                    done = True
                    break
            if not done and len(buf) - pos > 400:  # no sentence end in sight: stop at a word boundary
                yield buf[pos: buf.rfind(" ", pos, pos + 400)].strip() + "."
                done = True
        if mark:
            if not done and buf.strip():
                rest = buf[pos:].strip()
                yield rest if rest else buf.strip()
            done = describing = True
            yield " " + description.replace(TOOL_SAY, "")
    if not done:
        rest = buf[pos:].strip()
        yield rest if rest else buf.strip()


async def speakable_stream(text: AsyncIterable[str]) -> AsyncIterable[str]:
    """TTS text transform: buffer to a sentence/line end so a URL or code span split across chunks is caught whole."""
    buf, in_fence = "", False
    async for chunk in text:
        buf += chunk
        in_fence = buf.count("```") % 2 == 1
        if in_fence:
            continue
        cut = max(buf.rfind(c) for c in ".!?\n")
        # a period inside a URL/path/version is not a sentence end; wait for whitespace after it
        while cut != -1 and cut + 1 < len(buf) and not buf[cut + 1].isspace():
            cut = max(buf.rfind(c, 0, cut) for c in ".!?\n")
        if cut != -1 and cut + 1 < len(buf):
            yield speakable(buf[: cut + 1])
            buf = buf[cut + 1:]
    if buf:
        yield speakable(buf)
