"""Assemble app-owned Voicebot guidance and the user's personal prompt safely."""
import json
import os
import re
import tempfile
import threading
from collections import Counter
from pathlib import Path

TOOL_GUIDANCE_VERSION = 1
TOOL_GUIDANCE_FILENAME = "tool_guidance.md"
PROMPT_FILENAME = "system_prompt.md"
MIGRATION_STATE_FILENAME = ".prompt-migration.json"
_MIGRATION_LOCK = threading.Lock()

# Exact stock lines from the previously shipped default. Cleanup removes at most
# one copy of each exact line; customized and user-added lines are never rewritten.
LEGACY_TOOL_GUIDANCE = """How you work:
- When a request needs a tool, call the tool right away in this reply. Never say you are checking, looking or running something unless this same reply makes that tool call. Every tool call has a say field: a few words in your own words, specific to that call, spoken while the tool runs (for example \"Let me check your Downloads folder\"); vary it, put it only in the say field (not also as text in the same reply), and don't repeat it in your answer.
- When tool results come back, give the answer in one sentence. If a tool failed, say what failed in plain words.
- Never guess anything a tool can tell you: the time, the screen, files, command output, your MCP servers. Call the tool.
- When asked what you can do, answer from the list below; when asked about your MCP servers, call list_mcp_servers.
- Never say something is done unless a tool just confirmed it.
- An \"unread transcript\" is what the user said while you were asleep; use it as context and answer their latest request.
- Long conversations are compacted: older turns become a summary at the end of these instructions. For anything older or more exact, call search_conversations.

Tools:
- see_screen: the app and window the user is on right now, plus recent screen history.
- look_at_screen: a screenshot read by a vision model, for when the window title is not enough.
- type_up: when the user says \"type up\", \"write this\" or \"put this in the box\", write the finished text and pass exactly that text; it is pasted where the cursor is.
- press_keys: press keys in their current app: \"enter\" to send what you typed, \"tab\", \"escape\", arrows, or shortcuts like \"cmd+enter\", \"cmd+a\", \"cmd+shift+t\"; several presses separated by spaces. After type_up, press enter only when they ask you to send or submit.
- run_applescript: run AppleScript (osascript) to control the Mac when it is turned on: open, quit or switch apps (tell application \"Safari\" to activate), click menu items through System Events, show a notification (display notification \"Done\"), read or set the volume, and similar. Prefer press_keys for plain key presses.
- stop_listening: when the user says stop listening, goodbye, be quiet, or mute me.
- enroll_voice: when the user asks to set up their voice print, so you only answer their voice.
- read_system_prompt, edit_system_prompt, reload_system_prompt, open_system_prompt, open_voicebot_folder: read, change (applies at once, old version kept), reload, or open your own instructions; show your folder.
- set_name, set_wake_word: change your name or your wake word when the user asks.
- set_mood: show your mood through the orb (calm, focused, warm, playful, dreamy, cool) when the conversation's tone shifts; it changes at most every few minutes, and not when the user picked a design.
- my_settings, change_setting: know and change your own settings (voice, orb design, chat model, reasoning, vision model, thinking sounds, mic check, terminal off); a model or reasoning change starts a fresh conversation after your reply. You can't change the API key or URL or turn the terminal on.
- run_terminal: run a zsh command on this computer, when the user has turned it on; osascript -e works there too. Keep commands fast: find files with Spotlight (mdfind -name NAME) or by listing the likely folder, never by crawling the whole home folder. A command still running after a few seconds becomes a job: you get its number, process id and output file at once, and you're told when it finishes.
- check_job, stop_job, list_jobs: see a job's progress and latest output, stop it, or list jobs.
- search_conversations: search the transcript of every conversation, with dates, when the user asks what was said or done before, or you need something from earlier.
- list_mcp_servers, add_mcp_server, remove_mcp_server, reload_mcp_servers: see and change your MCP tool servers; adding and removing need the terminal switch.
- Your MCP servers add more tools (for example the current time); they are in your tool list. Use them when they fit."""

_MIGRATION_OFFER = (
    "This user's personal system_prompt.md still contains exact lines from the old built-in tool guide. "
    "Offer once: say exactly this question by itself as a standalone turn: 'Would you like me to "
    "remove the old tool guidance from your personal prompt?' Do not edit anything now. "
    "Do not preface, paraphrase, quote, or explain the question. "
    "If and only if the user explicitly agrees, call edit_system_prompt with "
    "remove_legacy_tool_guidance=true and empty find/replace. Never infer consent from a question, silence, "
    "or an ambiguous reply. That cleanup removes exact stock lines only and preserves every other line. "
    "If they decline, do not offer again."
)
_MIGRATION_OFFER_QUESTION = "Would you like me to remove the old tool guidance from your personal prompt?"


def load_personal_prompt(home: str | Path, resource_dir: str | Path) -> str:
    """Read the personal file exactly; read errors fall back to the shipped seed."""
    home_prompt = Path(home) / PROMPT_FILENAME
    try:
        with home_prompt.open("r", encoding="utf-8", newline="") as stream:
            return stream.read()
    except OSError:
        default_prompt = Path(resource_dir) / "system_prompt.default.md"
        with default_prompt.open("r", encoding="utf-8", newline="") as stream:
            return stream.read()


def load_app_tool_guidance(resource_dir: str | Path) -> str:
    """Load the versioned guidance from the app bundle, never from the user's folder."""
    path = Path(resource_dir) / TOOL_GUIDANCE_FILENAME
    with path.open("r", encoding="utf-8", newline="") as stream:
        guidance = stream.read()
    first_line = guidance.splitlines()[0] if guidance.splitlines() else ""
    if first_line != f"Version: {TOOL_GUIDANCE_VERSION}":
        raise ValueError(
            f"{path} must start with Version: {TOOL_GUIDANCE_VERSION}; "
            "bump TOOL_GUIDANCE_VERSION when the shipped guide changes"
        )
    return guidance


def assemble_instructions(name: str, app_guidance: str, personal_prompt: str, recent: str = "") -> str:
    """Place current app guidance before the untouched, user-owned prompt."""
    text = (
        f"Your name is {name}.\n\n"
        f"Current app-owned tool guidance (version {TOOL_GUIDANCE_VERSION}; shipped with and refreshed by Zana):\n\n"
        f"{app_guidance.rstrip()}\n\n"
        "User's personal instructions from system_prompt.md:\n\n"
        f"{personal_prompt}"
    )
    if recent:
        text += (
            "\n\nRecent conversation, from your transcript file (oldest first; continue from it, and never "
            "read it out unless asked):\n" + recent
        )
    return text


def _legacy_line_counts() -> Counter[str]:
    return Counter(line for line in LEGACY_TOOL_GUIDANCE.splitlines() if line.strip())


def has_legacy_tool_guidance(prompt: str) -> bool:
    """Recognize the former stock sections without treating arbitrary prose as app text."""
    lines = prompt.splitlines()
    if not any(line in ("Tools:", "How you work:") for line in lines):
        return False
    stock = _legacy_line_counts()
    matches = sum(1 for line in lines if line in stock)
    return matches >= 3


def _read_migration_state(home: str | Path) -> dict | None:
    path = Path(home) / MIGRATION_STATE_FILENAME
    try:
        with path.open("r", encoding="utf-8") as stream:
            state = json.load(stream)
    except FileNotFoundError:
        return {}
    except NotADirectoryError:
        return {}
    except (OSError, ValueError):
        return None  # fail closed; never repeat an offer or mutate an unknown state
    return state if isinstance(state, dict) else None


def _write_migration_state(home: str | Path, state: dict) -> bool:
    home_path = Path(home)
    temp_path = None
    try:
        home_path.mkdir(parents=True, exist_ok=True)
        fd, temp_path = tempfile.mkstemp(prefix=".prompt-migration-", dir=home_path)
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            json.dump(state, stream, sort_keys=True)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp_path, home_path / MIGRATION_STATE_FILENAME)
        return True
    except OSError:
        if temp_path is not None:
            try:
                os.unlink(temp_path)
            except OSError:
                pass
        return False


def take_legacy_prompt_offer(
    home: str | Path, personal_prompt: str, transcript: str = ""
) -> str:
    """Keep offering until the transcript confirms the user heard the offer."""
    if not has_legacy_tool_guidance(personal_prompt):
        return ""
    with _MIGRATION_LOCK:
        state = _read_migration_state(home)
        if state is None or state.get("delivered"):
            return ""
        if state.get("offered"):
            if not last_voicebot_turn_offered_cleanup(transcript):
                return _MIGRATION_OFFER
            state.update(delivered=True)
            _write_migration_state(home, state)
            return ""
        state.update(offered=True, version=TOOL_GUIDANCE_VERSION)
        _write_migration_state(home, state)
    return _MIGRATION_OFFER


def last_voicebot_turn_offered_cleanup(transcript: str) -> bool:
    """Bind a short affirmative to the immediately preceding spoken cleanup offer."""
    turns = []
    for line in transcript.splitlines():
        match = re.search(r"\]\s+(User|Voicebot|Other):\s*(.*)$", line)
        if match:
            turns.append((match.group(1), match.group(2)))
    if len(turns) < 2 or turns[-1][0] != "User" or turns[-2][0] != "Voicebot":
        return False
    return turns[-2][1].strip().casefold() == _MIGRATION_OFFER_QUESTION.casefold()


def explicitly_consents_to_cleanup(user_text: str, *, offer_was_immediately_prior: bool = False) -> bool:
    """Accept a direct yes or clear cleanup instruction; negatives and ambiguity fail closed."""
    text = re.sub(r"\s+", " ", user_text.strip().lower())
    if not text or re.search(
        r"\b(no|not|don't|dont|do not|never|wait|later|cancel|maybe|"
        r"won't|will not|wouldn't|would not|shouldn't|should not|can't|cannot|"
        r"skip|changed my mind)\b|\b(leave|keep)\s+(it|that|the old|the tool|the guidance|the prompt)\b",
        text,
    ):
        return False
    if re.fullmatch(r"(yes|yeah|yep|sure|okay|ok|please do|go ahead|do it|do that)[.!?]*", text):
        return offer_was_immediately_prior
    return _requests_tool_guidance_cleanup(text)


def _requests_tool_guidance_cleanup(text: str) -> bool:
    """Require a direct request naming the app-owned tool guidance."""
    action_and_target = (
        r"(?:remove|delete|clean up|strip)\b.{0,60}\btool[- ](?:guidance|instructions)\b"
    )
    direct_request = (
        rf"^(?:yes[, ]+)?(?:please\s+)?{action_and_target}",
        rf"^(?:can|could|would|will)\s+you\s+(?:please\s+)?{action_and_target}",
        rf"^(?:i want|i'd like|i would like)\s+you to\s+{action_and_target}",
    )
    return any(re.search(pattern, text) for pattern in direct_request)


def _remove_exact_legacy_lines(prompt: str) -> tuple[str, int]:
    remaining = _legacy_line_counts()
    output = []
    removed = 0
    for original_line in prompt.splitlines(keepends=True):
        line = original_line.rstrip("\r\n")
        if line and remaining[line] > 0:
            remaining[line] -= 1
            removed += 1
        else:
            output.append(original_line)
    return "".join(output), removed


def remove_legacy_guidance_after_consent(
    home: str | Path, prompt: str, user_text: str, transcript: str = ""
) -> tuple[str, bool, str]:
    """Clean only after the recorded offer was spoken immediately before consent."""
    state = _read_migration_state(home)
    offer_was_immediately_prior = last_voicebot_turn_offered_cleanup(transcript)
    if (state is None or not state.get("offered")) and not offer_was_immediately_prior:
        return prompt, False, "The one-time cleanup offer is not recorded; the prompt was not changed."
    if not explicitly_consents_to_cleanup(
        user_text, offer_was_immediately_prior=offer_was_immediately_prior
    ):
        return prompt, False, "Explicit user consent is required; the prompt was not changed."
    cleaned, removed = _remove_exact_legacy_lines(prompt)
    if not removed:
        return prompt, False, "No exact stock tool-guidance lines were found; the prompt was not changed."
    return cleaned, True, f"Removed {removed} exact stock tool-guidance lines; all other prompt text was preserved."
