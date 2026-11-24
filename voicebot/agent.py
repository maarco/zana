"""Voicebot: Zana's LiveKit voice agent (local Parakeet STT and VibeVoice TTS, a chat LLM, tools).

Zana starts this process, joins each conversation's room as the user's mic and
speaker (participant attribute client=zana), and answers RPCs from it:
zana.paste (type text at the cursor), zana.screen (front app and history),
zana.screenshot (path of a fresh screenshot) and zana.mute (turn the mic off).

Per-user files live in VOICEBOT_HOME (Zana's app-data folder):
  .env               keys and model overrides (see README.md)
  system_prompt.md   the system prompt (edited in Zana's Preferences)
  config.json        {"terminal": bool, "wake_word": str, "wake_timeout_seconds": int}
  mcp.json           MCP servers, Claude Desktop format: {"mcpServers": {...}}
  transcript.md      everything heard and said, always appended
"""
import asyncio
import base64
import datetime
import difflib
import json
import logging
import os
import random
import re
import threading
import time
from pathlib import Path

import updater
from dotenv import load_dotenv
from fillers import DEFAULT_PHRASES, Fillers, chunks_with_gap
from jobs import Jobs, conversation_end_warning
from listening import (
    ToolCallText,
    Transcript,
    clean,
    heard_wake_word,
    is_goodbye,
    parse_text_tool_calls,
    repeats,
    split_say,
    topics,
    wants_redo,
    wants_voiceprint,
)
from livekit import agents, rtc
from livekit.agents import (
    Agent,
    AgentServer,
    AgentSession,
    APIConnectOptions,
    JobProcess,
    RunContext,
    StopResponse,
    function_tool,
    get_job_context,
    inference,
    llm,
    mcp,
)
from livekit.agents.voice import room_io
from livekit.agents.voice.agent_session import SessionConnectOptions
from livekit.plugins import openai, silero
from orb_state import OrbState
from prompt_guidance import (
    assemble_instructions,
    has_legacy_tool_guidance,
    load_app_tool_guidance,
    load_personal_prompt,
    remove_legacy_guidance_after_consent,
    take_legacy_prompt_offer,
)
from speech import TOOL_SAY, join_sentences, one_sentence, speakable_stream
from voiceprint import RATE, VoicePrint, VoiceTap, speech_seconds, to_mono_16k

HERE = os.path.dirname(os.path.abspath(__file__))
HOME = os.path.expanduser(os.getenv("VOICEBOT_HOME", os.path.join(HERE, ".home")))
load_dotenv(os.path.join(HOME, ".env"))

VOICE = os.getenv("VOICE_SERVER_URL", "http://127.0.0.1:8800/v1")
FAST = float(os.getenv("VOICEBOT_FAST_SECONDS", "3"))  # slower terminal commands answer "on it" now and report later
TERMINAL_TIMEOUT = float(os.getenv("VOICEBOT_TERMINAL_TIMEOUT", "900"))  # jobs are stopped after 15 minutes
TRANSCRIPT = Transcript(HOME)  # always-on: every line heard and said
FILLERS = Fillers(HOME)  # thinking sounds, made in Voicebot's voice and cached
DEBUG_LOG = os.path.join(HOME, "logs", "debug.jsonl")  # what Voicebot does, for Zana's Debug window
SAY = "\n(Now answer concisely in plain spoken words.)"
MIN_VOICE_CHECK = int(0.5 * RATE)  # shorter turns are not checked against the voice print
# Every tool, as the model sees it, gets this required field; its text is spoken while the tool runs.
SAY_FIELD = {"type": "string",
             "description": "What you tell the user while this runs: a few words in your own words, specific to "
                            "this call, like \"Let me check your Downloads folder\". Vary it from call to call."}


def with_say(tool):
    """The tool as the model sees it, with the required "say" field first. Calls still run the real
    tool (by name); llm_node speaks "say" and removes it from the arguments."""
    if isinstance(tool, llm.RawFunctionTool):
        schema = {k: v for k, v in tool.info.raw_schema.items()}
    elif isinstance(tool, llm.FunctionTool):
        schema = llm.utils.build_legacy_openai_schema(tool, internally_tagged=True)
        schema.pop("type", None)
    else:
        return tool  # provider tools run on the provider's side
    params = dict(schema.get("parameters") or {"type": "object"})
    params["properties"] = {"say": SAY_FIELD, **(params.get("properties") or {})}
    params["required"] = ["say", *[r for r in params.get("required") or [] if r != "say"]]
    schema["parameters"] = params

    async def shown_only(raw_arguments: dict, context: RunContext):
        raise RuntimeError("shown to the model only; the real tool runs")
    return llm.function_tool(shown_only, raw_schema=schema)


# The orb designs as moods Voicebot can show (set_mood); the user's own pick goes through change_setting.
MOODS = {"calm": "original", "focused": "strands", "warm": "halo", "playful": "fur", "dreamy": "nebula", "cool": "liquid"}
STEP_GAP = 1.0  # seconds of quiet before a follow-up tool step speaks, so steps don't fire back to back
SHORT_TURN = 2.0  # seconds of actual speech; short phrases give noisier voice prints, so they get a lower cutoff
ADAPT_SCORE = 0.55  # confident matches this long or longer refine the saved print
COMPACT_CHARS = 12000  # conversation size (besides the instructions) that triggers compaction
KEEP_TURNS = 3  # recent turns kept word for word when compacting
ENROLL_SECONDS = 8
BARGE_IN_SECONDS = 0.5  # seconds of actual speech before it is checked against the voice print
UPDATE_HOURS = 6  # how often Voicebot looks for a newer Zana
MAX_REPLY_TOKENS = 16384  # the longest reply asked of OpenRouter, which holds credit for it (#50)

log = logging.getLogger("voicebot")


def debug(kind: str, **data) -> None:
    """One line per thing Voicebot does: heard, ignored, woke, said, tool, error, session."""
    try:
        os.makedirs(os.path.dirname(DEBUG_LOG), exist_ok=True)
        with open(DEBUG_LOG, "a") as f:
            f.write(json.dumps({"t": time.time(), "kind": kind, **data}, default=str) + "\n")
    except OSError:
        pass


def read_json(name: str) -> dict:
    try:
        with open(os.path.join(HOME, name)) as f:
            return json.load(f)
    except FileNotFoundError:
        return {}
    except (OSError, ValueError) as exc:
        log.warning("ignoring %s: %s", name, exc)
        return {}


def load_prompt() -> str:
    return load_personal_prompt(HOME, HERE)


def context_chars(ctx) -> int:
    """Characters the model reads each turn besides its instructions: messages, tool calls, tool results."""
    total = 0
    for item in ctx.items:
        if item.type == "message" and item.role not in ("system", "developer"):
            total += len(item.text_content or "")
        elif item.type == "function_call":
            total += len(item.arguments or "")
        elif item.type == "function_call_output":
            total += len(item.output or "")
    return total


def build_instructions(name: str) -> str:
    """App-owned guidance, the untouched personal prompt, then recent transcript context."""
    recent = TRANSCRIPT.tail(3000)
    return assemble_instructions(
        name, load_app_tool_guidance(HERE), load_prompt(), recent=recent
    )


def write_config(**changes) -> dict:
    """Merge keys into config.json (Zana writes terminal and wake_word; the agent writes its name)."""
    config = read_json("config.json")
    config.update(changes)
    with open(os.path.join(HOME, "config.json"), "w") as f:
        json.dump(config, f)
    return config


def make_llm(vision: bool = False):
    """The chat model from Zana's Preferences > Model (config.json "llm"); without one, LLM_* or
    OPENROUTER_* in .env. VISION_MODEL / VISION_BASE_URL / VISION_API_KEY pick the model that reads
    screenshots. A model from Preferences never gets a key from .env (it may be another provider's)."""
    chosen = read_json("config.json").get("llm") or {}
    if chosen.get("model"):
        model = chosen["model"]
        base_url = chosen.get("base_url") or "https://api.openai.com/v1"
        api_key = chosen.get("api_key") or "none"
        extra_body = chosen.get("extra_body") or {}  # Preferences > Model > Reasoning, shaped by Zana
        vision_model = chosen.get("vision_model") or ""
    else:
        model = os.getenv("LLM_MODEL") or os.getenv("OPENROUTER_MODEL", "nvidia/nemotron-3.5-super-vl:free")
        base_url = os.getenv("LLM_BASE_URL", "https://openrouter.ai/api/v1")
        api_key = os.getenv("LLM_API_KEY") or os.getenv("OPENROUTER_API_KEY") or os.getenv("openrouter") or "none"
        extra_body = {"reasoning": {"effort": "low", "exclude": True}} if "openrouter.ai" in base_url else {}
        vision_model = ""
    if vision:
        model = vision_model or os.getenv("VISION_MODEL", model)
        base_url = os.getenv("VISION_BASE_URL", base_url)
        api_key = os.getenv("VISION_API_KEY", api_key)
    # OpenRouter refuses a request its balance can't cover at the longest reply allowed, which is the model's
    # whole output length unless capped. Other providers hold no credit, and some reject unknown fields.
    cap = {"max_completion_tokens": MAX_REPLY_TOKENS} if "openrouter.ai" in base_url.lower() else {}
    return openai.LLM(model=model, base_url=base_url, api_key=api_key, **cap,
                      **({"extra_body": extra_body} if extra_body else {}))


def tts_voice() -> str:
    """Preferences > Voicebot > Voice, else TTS_VOICE in .env, else Emma."""
    return read_json("config.json").get("voice") or os.getenv("TTS_VOICE", "en-Emma_woman")


def make_tts():
    """Local VibeVoice by default; TTS_BASE_URL points at any OpenAI-compatible speech endpoint instead."""
    return openai.TTS(base_url=os.getenv("TTS_BASE_URL", VOICE), api_key=os.getenv("TTS_API_KEY", "local"),
                      model=os.getenv("TTS_MODEL", "tts-1"), voice=tts_voice(), response_format="pcm")


async def filler_audio(path: str):
    """A cached thinking sound as 20 ms audio frames, then the pause that keeps the reply apart."""
    pcm, rate, channels = Fillers.read(path)
    for chunk in chunks_with_gap(pcm, rate, channels):
        yield rtc.AudioFrame(chunk, rate, channels, len(chunk) // (2 * channels))


# Named keys for press_keys, as macOS key codes.
KEY_CODES = {"enter": 36, "return": 36, "tab": 48, "space": 49, "delete": 51, "backspace": 51, "escape": 53,
             "esc": 53, "left": 123, "right": 124, "down": 125, "up": 126, "home": 115, "end": 119,
             "pageup": 116, "pagedown": 121, "forwarddelete": 117}
MODIFIERS = {"cmd": "command down", "command": "command down", "shift": "shift down", "option": "option down",
             "alt": "option down", "ctrl": "control down", "control": "control down"}


def key_script(combo: str) -> str:
    """AppleScript for one key combo like "enter", "cmd+v", "cmd+shift+t" or "a"."""
    parts = [p.strip().lower() for p in combo.split("+") if p.strip()]
    if not parts:
        raise ValueError("empty key")
    *mods, key = parts
    unknown = [m for m in mods if m not in MODIFIERS]
    if unknown:
        raise ValueError(f"unknown modifier {unknown[0]}")
    using = f" using {{{', '.join(MODIFIERS[m] for m in mods)}}}" if mods else ""
    if key in KEY_CODES:
        return f'tell application "System Events" to key code {KEY_CODES[key]}{using}'
    if len(key) == 1:
        return f'tell application "System Events" to keystroke "{key}"{using}'
    raise ValueError(f"unknown key {key}")


async def osascript(script: str, timeout: float = 20) -> tuple[int, str]:
    proc = await asyncio.create_subprocess_exec("osascript", "-e", script, stdout=asyncio.subprocess.PIPE,
                                                stderr=asyncio.subprocess.STDOUT)
    try:
        out, _ = await asyncio.wait_for(proc.communicate(), timeout)
    except asyncio.TimeoutError:
        proc.kill()
        return 1, "timed out"
    return proc.returncode, clean(out.decode(errors="replace"))


async def zana(method: str, payload: str = "") -> str:
    """Call the Zana app in this room (it joins with attribute client=zana)."""
    room = get_job_context().room
    target = next((p for p in room.remote_participants.values() if p.attributes.get("client") == "zana"), None)
    if target is None:
        return "Zana isn't in this conversation, so the computer can't be reached."
    try:
        return await room.local_participant.perform_rpc(
            destination_identity=target.identity, method=method, payload=payload, response_timeout=15)
    except Exception as exc:  # noqa: BLE001 - spoken back, not raised
        return f"Zana didn't answer: {exc}"


class Voicebot(Agent):
    def __init__(self, config: dict, publish) -> None:
        self.name = str(config.get("name") or "Voicebot").strip()
        self.base_instructions = build_instructions(self.name)
        self.summary = ""  # the compacted part of this conversation, kept at the end of the instructions
        super().__init__(instructions=self.base_instructions)
        self.terminal = bool(config.get("terminal"))
        self.wake_word = str(config.get("wake_word") or "").strip()
        self.wake_timeout = float(config.get("wake_timeout_seconds") or 120)
        self.asleep = bool(self.wake_word)
        self.last_heard = 0.0
        self.last_user_text = ""
        self.publish = publish
        self.tap = VoiceTap()
        self.voiceprint = VoicePrint(HOME)
        self.threshold = float(config.get("voiceprint_threshold") or 0.4)
        self.short_threshold = float(config.get("voiceprint_short_threshold") or 0.28)
        fillers = config.get("fillers") or {}
        self.fillers_on = fillers.get("on", True)
        self.filler_phrases = [p.strip() for p in fillers.get("phrases") or [] if p.strip()] or DEFAULT_PHRASES
        self.filler_days = float(fillers.get("days") or 7)
        self.last_opener = ""
        self.mood_minutes = float(config.get("mood_minutes") or 3)
        self.mood_at = 0.0  # when the orb's mood last changed
        self.compact_chars = int(config.get("compact_chars") or COMPACT_CHARS)
        self.compact_at = self.compact_chars
        self.compacting = False
        self.enrolling = False
        self.enroll_audio: list = []
        threading.Thread(target=self._warm_voiceprint, daemon=True).start()
        self.base_tools = list(self.tools)
        self.mcp_servers_live: list = []
        self.mcp_tools: list = []
        self.updating = False
        self.watch: asyncio.Task | None = None  # watch_updates
        # Who can interrupt: "voice" (only the enrolled voice, once one is set up) or "anyone" (any speech,
        # livekit's own rule). Saved in config.json by change_setting; applies per conversation.
        self.interrupt = "anyone" if str(config.get("interrupt") or "").strip().lower() == "anyone" else "voice"
        self.voice_gated = False  # set by entrypoint when this conversation's interruptions are voice-checked
        self.barging = False
        self.last_voice = (0.0, False)  # when the last turn was voice-checked, and whether it was the user

    # ---------- listening: always record, answer only when awake ----------

    def go_to_sleep(self) -> None:
        debug("sleep", wake_word=self.wake_word)
        self.asleep = True
        TRANSCRIPT.mark_read()
        self.publish("sleep", asleep=True, wake_word=self.wake_word)

    def _warm_voiceprint(self) -> None:
        try:
            self.voiceprint.warm()
        except Exception as exc:  # noqa: BLE001 - reported, the agent still works without it
            debug("error", source="voiceprint", error=f"voice print model failed to load: {exc}")

    async def stt_node(self, audio, model_settings):
        """Pass the mic audio to speech-to-text, keeping a copy for the voice print."""
        async def tapped():
            async for frame in audio:
                try:
                    self.tap.push(to_mono_16k(frame.data, frame.sample_rate, frame.num_channels))
                except Exception:  # noqa: BLE001 - never break speech-to-text over the tap
                    pass
                yield frame
        async for event in Agent.default.stt_node(self, tapped(), model_settings):
            yield event

    def feature_notes(self, heard: str) -> str:
        """How-tos for the features the user just mentioned, with their live state, injected for one turn."""
        servers = ", ".join((read_json("mcp.json").get("mcpServers") or {}).keys()) or "none"
        guides = {
            "voiceprint": (f"Voice print: {'SET UP - you only answer the enrolled voice' if self.voiceprint.enrolled else 'NOT set up - you answer anyone'}. "
                           "To set it up the user says \"enroll my voice\" (or you call enroll_voice): you say you're going into "
                           "voice print mode, the orb turns red, and you record about eight seconds of them talking. To replace "
                           "it they say \"redo my voice print\". To remove it: Zana Preferences, Voicebot, Voice Print, Clear."),
            "wake": (f"Wake word: {repr(self.wake_word) if self.wake_word else 'none (you always answer)'}; you are "
                     f"{'asleep' if self.asleep else 'awake'}. While asleep you only wake for the wake word, and everything "
                     "said still goes into the transcript. The wake word is set in Zana Preferences under Voicebot."),
            "goodbye": ("Saying goodbye or stop listening puts you to sleep until the wake word, or turns their mic off "
                        "when no wake word is set; stop_listening does the same."),
            "mcp": (f"MCP servers configured: {servers}; {len(self.mcp_tools)} MCP tools loaded. Use list_mcp_servers, "
                    "reload_mcp_servers, add_mcp_server and remove_mcp_server (adding and removing need the Terminal "
                    f"Tool switch, which is {'on' if self.terminal else 'off'})."),
            "terminal": (f"Terminal Tool is {'on: run_terminal runs zsh commands on this computer' if self.terminal else 'off: they can turn it on in Zana Preferences under Voicebot'}."),
            "screen": ("see_screen gives the front app and window title; look_at_screen takes a screenshot and has a "
                       "vision model answer a question about it."),
            "debug": ("Right-clicking the orb (or Preferences, Voicebot, Debug) opens the Debug window: everything you "
                      "heard, said and ran, with errors."),
            "transcript": ("Everything heard and said is written to the always-on transcript file, across every "
                           "conversation; on the wake word you get the part said while you were asleep. "
                           "search_conversations finds words in it, with dates. Long conversations are compacted: "
                           "older turns are folded into a summary to keep you fast, and the transcript keeps everything."),
            "prompt": (f"Your system prompt is the user's own file, system_prompt.md in your Voicebot folder ({HOME}). "
                       "You may read it out or summarize it when asked (read_system_prompt). edit_system_prompt changes "
                       "it (exact find and replace, or append) and reloads it at once, keeping a backup; "
                       "reload_system_prompt picks up edits made elsewhere and re-attaches the recent conversation; "
                       "open_system_prompt opens it in their editor; open_voicebot_folder shows the folder. Don't speak "
                       "the path."),
            "name": (f"Your name is {self.name}; set_name changes it. Your wake word is "
                     f"{repr(self.wake_word) if self.wake_word else 'not set'}; set_wake_word changes it."),
            "settings": ("my_settings tells you every setting of yours (voice and the voices available, chat model, "
                         "reasoning, vision model, thinking sounds, mic check, terminal tool, voice print); "
                         "change_setting changes voice, model, reasoning, vision_model, thinking_sounds, "
                         "thinking_sound_lines, remake_sounds_days, mic_check_minutes, orb_design (original, "
                         "strands, halo, fur, nebula, liquid), mood_minutes, interrupt (voice: only the enrolled "
                         "voice can interrupt you, or anyone), and terminal (off only). set_mood shows "
                         "your mood with the orb (calm, focused, warm, playful, dreamy, cool), at most every few minutes. "
                         "A model, reasoning or vision change starts a fresh conversation after your reply; say so. "
                         "You cannot change the API key or URL or turn the terminal on: the user does that in "
                         "Zana's Preferences."),
        }
        notes = [guides[t] for t in topics(heard) if t in guides]
        personal_prompt = load_prompt()
        cleanup_offer_history = (
            TRANSCRIPT.search("old tool guidance from your personal prompt", limit=1)
            if has_legacy_tool_guidance(personal_prompt)
            else []
        )
        migration_offer = take_legacy_prompt_offer(
            HOME,
            personal_prompt,
            "\n".join([*TRANSCRIPT.tail_lines(2), *cleanup_offer_history]),
        )
        if migration_offer:
            notes.append(migration_offer)
        if notes:
            debug("context", topics=topics(heard), migration_offer=bool(migration_offer))
        return "\n".join(notes)

    def say_all(self, text: str, **kwargs) -> None:
        """Speak a line written by the app, not the model, in full: TOOL_SAY marks it so the
        one-sentence cap never cuts it (the transcript and the "said" log strip the marker)."""
        self.session.say(TOOL_SAY + text, **kwargs)

    def answer_voiceprint_request(self, heard: str) -> bool:
        """Handle "enroll my voice" in code. True when handled."""
        if not wants_voiceprint(heard):
            return False
        if self.voiceprint.enrolled and not wants_redo(heard):
            self.say_all("Your voice print is already set up. Say redo my voice print to record a new one.")
            debug("voiceprint", action="already set up")
            return True
        self.start_enrollment()
        return True

    def start_enrollment(self) -> None:
        """Voice print mode: the orb turns red and everything said next is recorded for the print."""
        self.enrolling, self.enroll_audio = True, []
        self.tap.take()  # the request itself is not part of the print
        debug("voiceprint", action="started")
        self.publish("voiceprint", action="started", seconds=0, needed=ENROLL_SECONDS)
        replacing = "Replacing your voice print. " if self.voiceprint.enrolled else ""
        self.say_all(f"{replacing}Going into voice print mode. Talk naturally for about {ENROLL_SECONDS} seconds.")

    async def collect_enrollment(self, audio) -> None:
        self.enroll_audio.append(audio)
        total = sum(len(a) for a in self.enroll_audio)
        debug("voiceprint", action="recording", seconds=round(total / RATE, 1))
        self.publish("voiceprint", action="recording", seconds=round(total / RATE, 1), needed=ENROLL_SECONDS)
        if total < ENROLL_SECONDS * RATE:
            return
        import numpy as np
        recorded = np.concatenate(self.enroll_audio)
        self.enrolling, self.enroll_audio = False, []
        await asyncio.get_running_loop().run_in_executor(None, self.voiceprint.enroll, recorded)
        debug("voiceprint", action="enrolled", seconds=round(len(recorded) / RATE, 1))
        self.publish("voiceprint", action="enrolled", seconds=round(len(recorded) / RATE, 1), needed=ENROLL_SECONDS)
        self.say_all("Voice print saved. From now on I only answer your voice.")

    def gate(self, heard: str, score: float | None = None) -> str | None:
        """Record what was heard; return what the model should get, or None while asleep."""
        TRANSCRIPT.record("User", heard)
        self.last_user_text = heard
        now = time.time()
        if self.wake_word and not self.asleep and now - self.last_heard > self.wake_timeout:
            self.go_to_sleep()
        extra = ""
        if self.asleep:
            if not heard_wake_word(heard, self.wake_word):
                debug("heard", text=heard, action="ignored (asleep)")
                return None  # stays in the transcript as unread; nothing reaches the model
            debug("wake", wake_word=self.wake_word)
            self.asleep = False
            self.publish("sleep", asleep=False, wake_word=self.wake_word)
            unread = TRANSCRIPT.take_unread().strip()
            if unread:
                extra = ("\n\nUnread transcript (what the user said while you were asleep, oldest first):\n"
                         + unread + "\n(Answer their latest request directly and concisely; don't say you're listening.)")
        else:
            TRANSCRIPT.mark_read()
        self.last_heard = now
        debug("heard", text=heard, action="answering", unread_chars=len(extra),
              voice_match=None if score is None else round(score, 2))
        return extra

    async def llm_node(self, chat_ctx, tools, model_settings):
        """Every tool call carries a "say" field the model writes for this call; it is spoken while the
        tool runs and removed before the tool sees its arguments. Some models now and then write a tool
        call out as text (<|tool_call_begin|>..., <tool_call>...): that text is held back, never spoken,
        and run as a real tool call."""
        split = ToolCallText()
        shown = [with_say(tool) for tool in tools]  # a new list: execution keeps the real tools
        # a step right after tool results is research going on: it sounds like it ("Hmm... checking the logs")
        after_tools = bool(chat_ctx.items) and chat_ctx.items[-1].type == "function_call_output"
        step_text = ""  # what the model itself wrote in this step, to keep the description from repeating it
        async for chunk in Agent.default.llm_node(self, chat_ctx, shown, model_settings):
            delta = getattr(chunk, "delta", None)
            if delta is not None and delta.content:
                delta.content = split.feed(delta.content) or None
                step_text += delta.content or ""
                if not delta.content and not delta.tool_calls:
                    continue
            said = []
            if delta is not None and delta.tool_calls:
                for call in delta.tool_calls:
                    say, call.arguments = split_say(call.arguments)
                    said.append(say)
            yield chunk  # the tool starts now; what it is doing is said next
            if any(said) and (text := await self.describe([s for s in said if s], after_tools, step_text)):
                yield text
        if rest := split.finish():
            yield rest
        held = split.held
        if not held:
            return
        calls = parse_text_tool_calls(held)
        debug("model", note=(f"The model wrote {len(calls)} tool call(s) as text; Voicebot ran them" if calls
                             else "The model wrote tool-call text Voicebot could not read; it was not spoken"),
              text=held[:400])
        if calls:
            split_calls = [(name, *split_say(args)) for name, args in calls]
            yield llm.ChatChunk(id="text-tool-calls", delta=llm.ChoiceDelta(role="assistant", tool_calls=[
                llm.FunctionToolCall(name=name, arguments=args, call_id=f"text_{i}_{int(time.time() * 1000)}")
                for i, (name, _, args) in enumerate(split_calls)]))
            said = [say for _, say, _ in split_calls if say]
            if said and (text := await self.describe(said, after_tools, step_text)):
                yield text

    async def on_user_turn_completed(self, turn_ctx, new_message) -> None:
        heard = new_message.text_content or ""
        audio = self.tap.take()
        if self.enrolling:
            TRANSCRIPT.record("User", heard)
            await self.collect_enrollment(audio)
            raise StopResponse()
        score = None
        if self.voiceprint.enrolled and len(audio) >= MIN_VOICE_CHECK:
            score, embedding, seconds = await asyncio.get_running_loop().run_in_executor(
                None, self.voiceprint.score_and_embedding, audio)
            cutoff = self.cutoff(seconds)
            self.last_voice = (time.time(), score >= cutoff)
            if score < cutoff:
                TRANSCRIPT.record("Other", heard)
                debug("heard", text=heard, action=f"ignored (not your voice, match {score:.2f} < {cutoff:.2f}, "
                                                   f"{seconds:.1f} s of speech)")
                raise StopResponse()
            if score >= ADAPT_SCORE and seconds >= SHORT_TURN:
                await asyncio.get_running_loop().run_in_executor(None, self.voiceprint.adapt, embedding)
        elif self.voiceprint.enrolled and not self.own_turn_without_audio():
            TRANSCRIPT.record("Other", heard)
            debug("heard", text=heard, action="ignored (too little audio of its own to check the voice)")
            raise StopResponse()
        extra = self.gate(heard, score)
        if extra is not None and is_goodbye(heard):
            await self.sign_off()
            raise StopResponse()
        if extra is not None and self.answer_voiceprint_request(heard):
            raise StopResponse()
        notes = self.feature_notes(heard) if extra is not None else ""
        if notes:
            turn_ctx.add_message(role="system", content="Context for this turn (live state and how-to):\n" + notes)
        if extra is None:
            raise StopResponse()
        self.play_filler()
        if extra:
            new_message.content.append(extra)

    def own_turn_without_audio(self) -> bool:
        """A turn with too little audio of its own to check: two turns ended together (a TV's dialogue often
        does) and the one before took the audio of both. It counts as the user's only when that audio was
        checked as theirs a moment ago; otherwise it isn't answered (it used to be, unchecked)."""
        when, theirs = self.last_voice
        return theirs and time.time() - when < 3

    def cutoff(self, seconds: float) -> float:
        """The voice print score a turn with this many seconds of speech needs (short ones are noisier)."""
        return self.short_threshold if seconds < SHORT_TURN else self.threshold

    # ---------- interruptions: only the enrolled voice ----------

    def replying(self) -> bool:
        """A reply is under way: being written, speaking, or between tool steps (the state can read
        "listening" while the model writes, after the thinking sound)."""
        speech = self.session.current_speech
        return speech is not None and not speech.done() and not speech.interrupted

    def maybe_barge_in(self) -> None:
        """Someone is talking while a reply is under way. Only the enrolled voice interrupts it: this
        conversation's built-in interruptions are off (entrypoint), check_barge_in decides."""
        if self.voice_gated and not self.barging and not self.updating and self.replying():
            self.barging = True
            asyncio.create_task(self.check_barge_in())

    async def check_barge_in(self) -> None:
        """Score the speech against the voice print once BARGE_IN_SECONDS of it is in (or when it ends, if
        shorter), and again as more comes, up to SHORT_TURN seconds: a match interrupts Voicebot, as any voice
        would have before. Other voices never do: Voicebot keeps going, their turn is dropped unanswered, and
        their audio is let go so it doesn't blur the next voice check."""
        loop, checked, verdict = asyncio.get_running_loop(), 0.0, None
        try:
            while self.replying():
                talking = self.session.user_state == "speaking"
                audio = self.tap.latest()
                spoken = speech_seconds(audio)
                if (spoken >= BARGE_IN_SECONDS and spoken - checked >= 0.4) or \
                        (not talking and spoken > checked and len(audio) >= MIN_VOICE_CHECK):
                    checked, reply = spoken, self.session.current_speech
                    if not self.voiceprint.enrolled:  # cleared since this conversation began: anyone interrupts
                        score, seconds = 1.0, spoken
                    else:
                        score, _, seconds = await loop.run_in_executor(
                            None, self.voiceprint.score_and_embedding, audio)
                    if self.session.current_speech is not reply or not self.replying():
                        return  # that reply ended while scoring; the turn goes the normal way
                    verdict = score >= self.cutoff(seconds)
                    self.last_voice = (time.time(), verdict)
                    if verdict:
                        debug("interrupt", voice_match=round(score, 2), seconds=round(seconds, 1))
                        reply.allow_interruptions = True  # the user's turn is then taken while the reply winds down
                        self.session.interrupt(force=True)
                        return
                    if seconds >= SHORT_TURN:
                        break
                if not talking:
                    break
                await asyncio.sleep(0.1)
            if verdict is False and self.replying():
                self.tap.take()  # its turn is dropped while the reply plays; its audio goes with it
                debug("interrupt", action="ignored (not your voice)", seconds=round(checked, 1))
        except RuntimeError:
            pass  # the conversation is closing
        finally:
            self.barging = False

    # ---------- thinking sounds ----------

    async def refresh_fillers(self) -> None:
        """Make the thinking sounds in the current voice when they are missing, older than the
        configured days, or the voice or lines changed; cached in between. Made one at a time and
        only while Voicebot is not talking or thinking, so live replies come first."""
        voice = tts_voice()
        if not self.fillers_on or not FILLERS.stale(voice, self.filler_phrases, self.filler_days):
            return
        started, clips = time.time(), []
        for phrase in self.filler_phrases:
            while self.session.agent_state in ("thinking", "speaking"):
                await asyncio.sleep(0.5)
            try:
                frames = []
                async with self.session.tts.synthesize(phrase) as stream:
                    async for audio in stream:
                        frames.append(audio.frame)
                if frames:
                    clips.append((phrase, b"".join(bytes(f.data) for f in frames),
                                  frames[0].sample_rate, frames[0].num_channels))
            except Exception as exc:  # noqa: BLE001 - one bad line should not stop the rest
                debug("error", source="thinking sounds", error=f"{phrase}: {exc}"[:300])
        if clips:
            await asyncio.get_running_loop().run_in_executor(None, FILLERS.save, voice, self.filler_phrases, clips)
            debug("fillers", count=len(clips), voice=voice, seconds=round(time.time() - started))

    async def describe(self, said: list[str], after_tools: bool, step_text: str = "") -> str:
        """The spoken description of a tool step, marked so the one-sentence cap keeps it. Skipped
        when the model already said the same in this step. A step after tool results waits a moment
        and, when the model said nothing else, opens with a thinking word ("Hmm..."), never the same
        twice in a row, so a chain of tool calls sounds like research rather than a list."""
        said = [s for s in said if not repeats(s, step_text)]
        if not said:
            return ""
        opener = ""
        if after_tools and self.fillers_on:
            await asyncio.sleep(STEP_GAP)
            if len(step_text.split()) <= 3:
                choices = [p for p in self.filler_phrases if p != self.last_opener] or self.filler_phrases
                self.last_opener = opener = random.choice(choices)
                opener = (opener if opener.rstrip().endswith((".", ",", "!", "?")) else opener + "...") + " "
        return " " + TOOL_SAY + opener + join_sentences(said) + " "

    @function_tool()
    async def set_mood(self, context: RunContext, mood: str) -> str:
        """Show how you feel through the orb's look: calm (the original orb), focused (strands), warm (halo),
        playful (fur), dreamy (nebula) or cool (liquid). Change it when the conversation's tone shifts, not
        every turn; it changes at most once every few minutes. Don't use it when the user picked a design or
        asked you to keep one.

        Args:
            mood: calm, focused, warm, playful, dreamy or cool.
        """
        design = MOODS.get(mood.strip().lower())
        if design is None:
            return f"Moods are {', '.join(MOODS)}." + SAY
        wait = self.mood_at + self.mood_minutes * 60 - time.time()
        if wait > 0:
            return f"The mood stays for about {wait / 60:.0f} more minutes; the orb keeps its look." + SAY
        result = await zana("zana.set_setting", json.dumps({"name": "orb_design", "value": design}))
        if result.startswith("Orb design set"):
            self.mood_at = time.time()
            debug("mood", mood=mood, design=design)
        return result + SAY

    def play_filler(self) -> None:
        """A random thinking sound while the reply is worked out; a short pause follows it."""
        picked = FILLERS.pick() if self.fillers_on else None
        if not picked:
            return
        path, phrase = picked
        try:
            self.session.say(phrase, audio=filler_audio(path), allow_interruptions=not self.voice_gated,
                             add_to_chat_ctx=False)
            debug("filler", text=phrase)
        except RuntimeError:
            pass  # the conversation is closing

    # ---------- conversation memory: compaction and transcript search ----------

    def maybe_compact(self) -> None:
        """Fold older turns into a summary once the conversation passes its size budget."""
        if not self.compacting and context_chars(self.chat_ctx) > self.compact_at:
            self.compacting = True
            asyncio.create_task(self.compact())

    def full_instructions(self) -> str:
        if not self.summary:
            return self.base_instructions
        return (self.base_instructions + "\n\nSummary of the earlier part of this conversation (older turns were "
                "compacted into it; search_conversations finds the exact words):\n" + self.summary)

    async def compact(self) -> None:
        """Summarize all but the last few turns with the chat model and move the summary into the instructions
        (one system message; a made-up assistant turn holding it led the model to write tool calls as text).
        The previous summary is folded into the new one, so the context stays bounded; the transcript file
        keeps everything."""
        try:
            snapshot = self.chat_ctx.copy()
            order = {item.id: index for index, item in enumerate(snapshot.items)}
            seen = set(order)
            calls = [item for item in snapshot.items if item.type == "function_call"]
            completed = {item.call_id for item in snapshot.items if item.type == "function_call_output"}
            pending = {call.call_id for call in calls} - completed
            before_items, before_chars = len(snapshot.items), context_chars(snapshot)
            if self.summary:
                snapshot.items.insert(0, llm.ChatMessage(role="assistant", content=[
                    "Summary of the conversation before this point: " + self.summary]))
            compacted = await snapshot._summarize(self.session.llm, keep_last_turns=KEEP_TURNS)
            made = [i for i in compacted.items if i.type == "message" and i.extra.get("is_summary")]
            if not made:
                debug("compact", action="skipped", before_items=before_items, before_chars=before_chars)
                self.compact_at = before_chars + self.compact_chars // 2
                return
            self.summary = re.sub(r"</?chat_history_summary>", "", made[-1].text_content or "").strip()
            compacted.items = [i for i in compacted.items if i not in made]
            compacted.items.extend(item for item in self.chat_ctx.items if item.id not in seen)  # said meanwhile
            # LiveKit splits on messages, so an intervening message can leave a tool call in the
            # summarized head and its result in the tail (or a result can arrive meanwhile).
            # Keep matching and still-pending calls; provider formatting otherwise drops results.
            needed = pending | {item.call_id for item in compacted.items if item.type == "function_call_output"}
            present = {item.call_id for item in compacted.items if item.type == "function_call"}
            for call in calls:
                if call.call_id in needed and call.call_id not in present:
                    # Preserve snapshot order rather than sorting by timestamps: LiveKit can
                    # put instructions first even when their timestamp is newer than the call.
                    pos = next((i for i, item in enumerate(compacted.items)
                                if order.get(item.id, len(order)) > order[call.id]), len(compacted.items))
                    compacted.items.insert(pos, call)
                    present.add(call.call_id)
            await self.update_instructions(self.full_instructions())
            await self.update_chat_ctx(compacted)
            after = context_chars(compacted)
            debug("compact", before_items=before_items, before_chars=before_chars,
                  after_items=len(compacted.items), after_chars=after, summary=self.summary)
            self.compact_at = max(self.compact_chars, after + len(self.summary) + self.compact_chars // 2)
        except Exception as exc:  # noqa: BLE001 - logged; the conversation goes on uncompacted
            debug("error", source="compaction", error=str(exc)[:500])
            self.compact_at = context_chars(self.chat_ctx) + self.compact_chars // 2
        finally:
            self.compacting = False

    @function_tool()
    async def search_conversations(self, context: RunContext, words: str) -> str:
        """Search the transcript of every conversation (everything heard and said, with dates) for words.
        Use it when the user asks what was said or done before, or for anything from earlier that is no
        longer in front of you (older turns of this conversation are compacted into a summary).

        Args:
            words: The words to look for, like "pty manager" or "wake word".
        """
        found = TRANSCRIPT.search(words)
        if not found:
            return f'Nothing in the transcript has "{words}".' + SAY
        return "Transcript lines, oldest first:\n" + clean("\n".join(found), 4000) + SAY

    # ---------- MCP servers ----------

    async def load_mcp(self) -> str:
        """(Re)start every server in mcp.json and swap their tools in."""
        for server in self.mcp_servers_live:
            try:
                await server.aclose()
            except Exception:  # noqa: BLE001 - closing a dead server is fine
                pass
        self.mcp_servers_live, self.mcp_tools, report = [], [], []
        base_env = {k: v for k, v in os.environ.items() if k in ("PATH", "HOME", "USER", "LANG", "TMPDIR")}
        for name, spec in (read_json("mcp.json").get("mcpServers") or {}).items():
            try:
                timeout = float(spec.get("timeout", 15))
                if spec.get("url"):
                    server = mcp.MCPServerHTTP(url=spec["url"], headers=spec.get("headers"),
                                               client_session_timeout_seconds=timeout)
                else:
                    server = mcp.MCPServerStdio(command=spec["command"], args=list(spec.get("args", [])),
                                                env={**base_env, **(spec.get("env") or {})}, cwd=spec.get("cwd"),
                                                client_session_timeout_seconds=timeout)
                await asyncio.wait_for(server.initialize(), timeout=timeout + 30)
                tools = await server.list_tools()
                self.mcp_servers_live.append(server)
                self.mcp_tools.extend(tools)
                report.append(f"{name}: {len(tools)} tools")
            except Exception as exc:  # noqa: BLE001 - one bad server must not stop the others
                log.warning("MCP server %s failed: %s", name, exc)
                report.append(f"{name}: failed ({exc})")
        await self.update_tools(self.base_tools + self.mcp_tools)
        debug("mcp", servers=report)
        log.info("MCP: %s", "; ".join(report) or "none configured")
        return "; ".join(report) or "No MCP servers are configured."

    @function_tool()
    async def list_mcp_servers(self, context: RunContext) -> str:
        """List the MCP tool servers you have and how many tools each gives you."""
        servers = read_json("mcp.json").get("mcpServers") or {}
        return (", ".join(servers) or "none") + f"; {len(self.mcp_tools)} MCP tools loaded right now." + SAY

    @function_tool()
    async def reload_mcp_servers(self, context: RunContext) -> str:
        """Restart all MCP tool servers and load their current tools (after the user changed them)."""
        return await self.load_mcp() + SAY

    @function_tool()
    async def add_mcp_server(self, context: RunContext, name: str, command: str = "", args: list[str] | None = None,
                             url: str = "") -> str:
        """Add an MCP tool server and load it right away. Give either a command (with args) or a url.

        Args:
            name: A short name for the server, like "github".
            command: The program to start, like "npx" or "uvx", for a local server.
            args: The program's arguments, like ["-y", "some-mcp-server"].
            url: The address of a remote MCP server instead of a command.
        """
        if not self.terminal:
            return "Adding MCP servers runs programs, so it needs the Terminal Tool switch in Zana's Preferences." + SAY
        if not re.fullmatch(r"[A-Za-z0-9_-]{1,40}", name) or not (command or url):
            return "That needs a short name and either a command or a url." + SAY
        config = read_json("mcp.json")
        servers = config.setdefault("mcpServers", {})
        servers[name] = {"url": url} if url else {"command": command, "args": args or []}
        with open(os.path.join(HOME, "mcp.json"), "w") as f:
            json.dump(config, f, indent=2)
        return f"Added {name}. " + await self.load_mcp() + SAY

    @function_tool()
    async def remove_mcp_server(self, context: RunContext, name: str) -> str:
        """Remove an MCP tool server by name and reload the rest.

        Args:
            name: The server's name, as list_mcp_servers says it.
        """
        if not self.terminal:
            return "Changing MCP servers needs the Terminal Tool switch in Zana's Preferences." + SAY
        config = read_json("mcp.json")
        if (config.get("mcpServers") or {}).pop(name, None) is None:
            return f"There is no MCP server called {name}." + SAY
        with open(os.path.join(HOME, "mcp.json"), "w") as f:
            json.dump(config, f, indent=2)
        return f"Removed {name}. " + await self.load_mcp() + SAY

    # ---------- the computer ----------

    @function_tool()
    async def type_up(self, context: RunContext, text: str) -> str:
        """Type text for the user into whatever box their cursor is in: Zana puts it on the clipboard and pastes it.
        Use when the user says "type up", "write this" or "put this in the box". Never end with a newline.

        Args:
            text: The exact text to paste, ready to send, with no quotes or commentary around it.
        """
        return await zana("zana.paste", text) + SAY

    @function_tool()
    async def press_keys(self, context: RunContext, keys: str) -> str:
        """Press keys in the app the user is in: named keys (enter, tab, escape, delete, up, down, left, right,
        space, home, end, pageup, pagedown) or combos with cmd, shift, option, ctrl, like "cmd+v", "cmd+enter",
        "cmd+shift+t". Several presses are separated by spaces, like "tab tab enter". Use enter to send what
        type_up just pasted when the user asks you to send or submit it.

        Args:
            keys: The keys to press, like "enter" or "cmd+a delete".
        """
        for combo in keys.split():
            try:
                script = key_script(combo)
            except ValueError as exc:
                return f"Couldn't press {combo}: {exc}." + SAY
            code, out = await osascript(script)
            if code != 0:
                return f"Pressing {combo} failed: {out.strip()[:200]} (Zana may need Accessibility permission)." + SAY
            await asyncio.sleep(0.05)
        return f"Pressed {keys}." + SAY

    @function_tool()
    async def run_applescript(self, context: RunContext, script: str) -> str:
        """Run an AppleScript with osascript to control the Mac: open or quit apps, click menu items, read or set
        values, show notifications, and more. Needs the Terminal Tool switch.

        Args:
            script: The AppleScript source, like 'tell application "Notes" to activate'.
        """
        if not self.terminal:
            return "AppleScript needs the Terminal Tool switch in Zana's Preferences." + SAY
        code, out = await osascript(script, timeout=60)
        return f"exit code {code}\n{out}" + SAY

    @function_tool()
    async def see_screen(self, context: RunContext) -> str:
        """See which app and window the user is on right now, plus their recent screen history."""
        return await zana("zana.screen")

    @function_tool()
    async def look_at_screen(self, context: RunContext, question: str) -> str:
        """Take a screenshot and have a vision model answer a question about it.

        Args:
            question: What you want to know about the screen, like "what error is showing".
        """
        path = await zana("zana.screenshot")
        if not os.path.isfile(path):
            return f"No screenshot: {path}" + SAY
        with open(path, "rb") as f:
            data_url = "data:image/jpeg;base64," + base64.b64encode(f.read()).decode()
        ctx = llm.ChatContext.empty()
        ctx.add_message(role="system", content="Answer the question about this screenshot of the user's screen in "
                                               "two or three plain sentences. Describe; never quote paths or links.")
        ctx.add_message(role="user", content=[question, llm.ImageContent(image=data_url)])
        answer = ""
        try:
            async with make_llm(vision=True).chat(chat_ctx=ctx) as stream:
                async for chunk in stream:
                    if chunk.delta and chunk.delta.content:
                        answer += chunk.delta.content
        except Exception as exc:  # noqa: BLE001 - spoken back
            return f"The vision model failed ({exc}); it may not accept images, see VISION_MODEL." + SAY
        return answer.strip() + SAY

    async def reload_instructions(self) -> None:
        self.base_instructions = build_instructions(self.name)
        await self.update_instructions(self.full_instructions())
        debug("prompt", action="reloaded", name=self.name)

    @function_tool()
    async def edit_system_prompt(
        self, context: RunContext, find: str = "", replace: str = "",
        remove_legacy_tool_guidance: bool = False,
    ) -> str:
        """Edit personal instructions, or remove exact legacy app-tool lines after explicit consent.

        Args:
            find: Exact text to replace (empty to append; leave empty for migration mode).
            replace: Replacement text (leave empty for migration mode).
            remove_legacy_tool_guidance: Use only after the user explicitly agreed to the one-time offer;
                removes exact unchanged stock guidance lines and preserves all other prompt text.
        """
        path = os.path.join(HOME, "system_prompt.md")
        old = load_prompt()
        if remove_legacy_tool_guidance:
            if find or replace:
                return "For the legacy cleanup, leave find and replace empty; nothing was changed." + SAY
            new, changed, message = remove_legacy_guidance_after_consent(
                HOME, old, self.last_user_text, TRANSCRIPT.tail_lines(2)
            )
            if not changed:
                return message + SAY
        else:
            if find and find not in old:
                return "That exact text isn't in the prompt; read_system_prompt shows the wording." + SAY
            new = old.replace(find, replace, 1) if find else old.rstrip("\n") + "\n" + replace.strip() + "\n"
        history = os.path.join(HOME, "prompt-history")
        os.makedirs(history, exist_ok=True)
        stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
        with open(os.path.join(history, f"system_prompt-{stamp}.md"), "w") as f:
            f.write(old)
        with open(path, "w") as f:
            f.write(new)
        diff = "\n".join(difflib.unified_diff(old.splitlines(), new.splitlines(), "before", "after", lineterm="", n=0))
        debug("prompt", action="edited", diff=diff[:4000], backup=f"prompt-history/system_prompt-{stamp}.md")
        await self.reload_instructions()
        return "Prompt edited and reloaded; the old version is saved in the prompt history." + SAY

    @function_tool()
    async def reload_system_prompt(self, context: RunContext) -> str:
        """Reload your system prompt from its file (after the user edited it) and re-attach the recent
        conversation from the transcript file."""
        await self.reload_instructions()
        return "Reloaded your instructions and the recent conversation." + SAY

    @function_tool()
    async def set_wake_word(self, context: RunContext, wake_word: str) -> str:
        """Change your wake word (empty removes it, so you always answer). Saved in Zana's settings.

        Args:
            wake_word: The new wake word, like "hey jarvis", or empty.
        """
        result = await zana("zana.set_wake_word", wake_word.strip())
        if result.startswith("Zana"):
            return result + SAY
        self.wake_word = wake_word.strip()
        debug("settings", wake_word=self.wake_word)
        return (f"Wake word is now {self.wake_word}." if self.wake_word else "Wake word removed; you always answer.") + SAY

    @function_tool()
    async def set_name(self, context: RunContext, name: str) -> str:
        """Change your name (what you call yourself). Applies right away.

        Args:
            name: Your new name, like "Jarvis".
        """
        self.name = name.strip() or "Voicebot"
        write_config(name=self.name)
        debug("settings", name=self.name)
        await self.reload_instructions()
        return f"Your name is now {self.name}." + SAY

    @function_tool()
    async def my_settings(self, context: RunContext) -> str:
        """Your own settings, as the user sees them in Zana's Preferences: name, wake word, voice and the
        voices available, chat model, reasoning, vision model, thinking sounds, mic check minutes, terminal
        tool and voice print. Use it when asked about yourself or before changing a setting."""
        raw = await zana("zana.settings")
        try:
            settings = json.loads(raw)
        except ValueError:
            return raw + SAY
        settings["interrupt"] = (f"{self.interrupt} (who can interrupt you: voice = only the enrolled voice, "
                                 f"anyone = any speech; now: {'voice checked' if self.voice_gated else 'anyone'})")
        return "\n".join(f"{k}: {v}" for k, v in settings.items()) + SAY

    @function_tool()
    async def change_setting(self, context: RunContext, setting: str, value: str) -> str:
        """Change one of your own settings; it is saved in Zana's Preferences. Use set_name and
        set_wake_word for your name and wake word.

        Args:
            setting: One of voice, model, reasoning, vision_model, thinking_sounds, thinking_sound_lines,
                remake_sounds_days, mic_check_minutes, orb_design, terminal, interrupt.
            value: A voice from my_settings (a first name like "Emma" works); a model id; reasoning default,
                off, low, medium or high; on or off; sound lines separated by "|" ("default" for the built-in
                ones); a whole number of days or minutes; orb_design original, strands, halo, fur, nebula or
                liquid; terminal only "off"; interrupt "voice" (only the user's enrolled voice can interrupt
                you) or "anyone".
        """
        if setting.strip().lower() == "interrupt":  # Voicebot's own setting, in config.json
            choice = value.strip().lower()
            if choice not in ("voice", "anyone"):
                return 'interrupt is "voice" (only the enrolled voice) or "anyone".' + SAY
            write_config(interrupt=choice)
            self.interrupt = choice
            debug("settings", setting="interrupt", value=choice)
            who = "only your voice" if choice == "voice" else "anyone"
            return (f"Saved: {who} can interrupt you, from the next conversation (double-click the orb for a new "
                    "one now)." + ("" if self.voiceprint.enrolled or choice == "anyone" else
                                    " Until a voice print is set up, anyone can.")) + SAY
        result = await zana("zana.set_setting", json.dumps({"name": setting, "value": value}))
        debug("settings", setting=setting, value=value, result=result)
        if result.startswith(("Not changed", "Zana didn't answer", "Zana isn't")):
            return result + SAY
        if setting == "voice":  # speak in it from the next sentence, without a new conversation
            voice = re.search(r"Voice set to (\S+)", result)
            if voice:
                self.session.tts.update_options(voice=voice.group(1))
        elif setting == "thinking_sounds":
            self.fillers_on = value.strip().lower() in ("on", "true", "yes", "1", "enable", "enabled")
        elif setting == "thinking_sound_lines":
            lines = [p.strip() for p in re.split(r"[|\n]", value) if p.strip() and p.strip().lower() != "default"]
            self.filler_phrases = lines or DEFAULT_PHRASES
            asyncio.create_task(self.refresh_fillers())
        elif setting == "remake_sounds_days":
            self.filler_days = float(value)
        elif setting == "mood_minutes":
            self.mood_minutes = float(value)
        elif setting == "terminal":
            self.terminal = False
        return result + SAY

    @function_tool()
    async def open_system_prompt(self, context: RunContext) -> str:
        """Open your system prompt file (your instructions, system_prompt.md) in the user's text editor."""
        path = os.path.join(HOME, "system_prompt.md")
        proc = await asyncio.create_subprocess_exec("open", "-t", path)
        await proc.wait()
        return ("Opened your system prompt in their text editor; changes apply from the next conversation."
                if proc.returncode == 0 else "Opening the system prompt failed.") + SAY

    @function_tool()
    async def read_system_prompt(self, context: RunContext) -> str:
        """Read your own system prompt (the user's file) so you can tell them what your instructions say."""
        return load_prompt()[:6000] + "\n(Summarize the instructions briefly.)"

    @function_tool()
    async def open_voicebot_folder(self, context: RunContext) -> str:
        """Open your Voicebot folder in Finder: system prompt, keys file, MCP servers, transcript and logs."""
        proc = await asyncio.create_subprocess_exec("open", HOME)
        await proc.wait()
        return ("Opened the Voicebot folder in Finder." if proc.returncode == 0 else "Opening the folder failed.") + SAY

    @function_tool()
    async def enroll_voice(self, context: RunContext) -> None:
        """Set up (or redo) the user's voice print, so you only answer their voice. Use when they ask to set up
        their voice, enroll, or stop you answering other people. It announces itself; say nothing more."""
        self.start_enrollment()

    async def sign_off(self) -> None:
        """Goodbye said in code, not by the model: sleep (wake word set) or turn the mic off."""
        debug("goodbye", wake_word=self.wake_word)
        if self.wake_word:
            self.go_to_sleep()
            self.say_all(f"Goodbye. Say {self.wake_word} when you need me.")
        else:
            await zana("zana.mute")
            self.say_all("Goodbye. Click the orb when you need me.")

    @function_tool()
    async def stop_listening(self, context: RunContext) -> str:
        """Stop listening to the user. Use when they say stop listening, goodbye, be quiet, that's all, or mute me.
        With a wake word set, sleep until they say it; otherwise their microphone is turned off."""
        if self.wake_word:
            self.go_to_sleep()
            return f"Say goodbye and that they can say {self.wake_word} when they need you." + SAY
        await zana("zana.mute")
        return "Their microphone is now off; they click the orb to turn it back on. Say goodbye briefly." + SAY

    @function_tool()
    async def run_terminal(self, context: RunContext, command: str) -> str:
        """Run a zsh command on the user's computer. Commands that finish within a few seconds return their
        output; longer ones keep running as a job whose output streams to a file, and you get the job number,
        process id and output file right away, then check_job and stop_job. You are told when a job finishes.
        Use fast, targeted commands: to find a file or folder by name use Spotlight (mdfind -name NAME | head;
        it skips hidden folders, so list those directly) or list the likely folder; never crawl the whole
        home folder with find or grep -r.

        Args:
            command: The zsh command to run.
        """
        if not self.terminal:
            return "The terminal is off; the user can turn on Terminal Tool in Zana's Preferences." + SAY
        job = await JOBS.start(command)
        if isinstance(job, str):
            return job + SAY
        done, _ = await asyncio.wait({job.task}, timeout=FAST)
        if done:
            return f"exit code {job.exit_code}\n{job.output()}" + SAY

        def finished(_t: asyncio.Task):
            if job.quiet:  # stopped on purpose; stop_job already answered
                return
            summary = job.summary(lines=60)
            debug("tool", name=f"run_terminal job {job.id} (finished)", args={"command": command}, output=summary[:2000])
            context.session.generate_reply(instructions=f"Terminal job {job.id} finished.\n{summary}{SAY}")
        job.task.add_done_callback(finished)
        debug("job", id=job.id, pid=job.pid, command=command, log=job.log)
        return (f"Still running as job {job.id} (process {job.pid}), output streaming to {job.log}. "
                f"IMPORTANT: Tell the user explicitly that {conversation_end_warning(job.id)} "
                "check_job shows progress and stop_job stops it now; you'll be told if it finishes first.")

    @function_tool()
    async def check_job(self, context: RunContext, job: int) -> str:
        """Show a terminal job's status (running or finished, exit code, how long) and its latest output.

        Args:
            job: The job number run_terminal gave you.
        """
        found = JOBS.get(job)
        return (found.summary(lines=40) if found else f"There is no job {job}.") + SAY

    @function_tool()
    async def stop_job(self, context: RunContext, job: int) -> str:
        """Stop a running terminal job and everything it started.

        Args:
            job: The job number.
        """
        found = JOBS.get(job)
        if not found:
            return f"There is no job {job}." + SAY
        found.stop(quiet=True)
        debug("job", id=found.id, action="stopped")
        return f"Stopped job {job}." + SAY

    @function_tool()
    async def list_jobs(self, context: RunContext) -> str:
        """List terminal jobs from this conversation with their status."""
        return ("\n".join(j.summary(lines=0) for j in JOBS.jobs.values()) or "No terminal jobs.") + SAY

    # ---------- Zana updates ----------

    async def watch_updates(self) -> None:
        """Look for a newer Zana soon after starting, then every few hours. Each new version is offered once,
        when Voicebot is awake and the conversation is quiet; update_zana installs it."""
        app = updater.installed_app(HERE)
        if app is None:
            return  # running from source
        await asyncio.sleep(30)
        # After the restart helper's 20 s check (it may still need the old app), when no update is going in:
        # old or failed copies go.
        if "done" not in updater.load_state(HOME):
            updater.remove_old(app)
        while True:
            try:
                found = await asyncio.get_running_loop().run_in_executor(None, updater.check, HERE)
            except Exception as exc:  # noqa: BLE001 - logged; the next check is in a few hours
                debug("update", error=str(exc), detail=getattr(exc, "detail", ""))
                found = None
            if found and found["newer"] and self.should_offer(found["version"]):
                while self.asleep or self.session.agent_state != "listening" or self.session.user_state == "speaking":
                    await asyncio.sleep(5)
                if self.should_offer(found["version"]):  # asked in another conversation, or updating, meanwhile
                    updater.save_state(HOME, offered=found["version"], offered_at=time.time())
                    debug("update", offered=found["version"], current=found["current"])
                    # A fixed line, not a model reply: a model can skip an instruction, and this has to be asked.
                    # It joins the conversation, so a yes reaches the model as the answer to it (update_zana).
                    self.say_all(f"Zana {found['version']} is out, and this Mac has {found['current']}. "
                                 "Want me to update it now?")
            await asyncio.sleep(UPDATE_HOURS * 3600)

    def should_offer(self, version: str) -> bool:
        """Once per version, across conversations, and not while an update is going in."""
        return not self.updating and updater.load_state(HOME).get("offered") != version

    @function_tool()
    async def check_for_update(self, context: RunContext) -> str:
        """Check whether a newer version of Zana, the app you run in, is out."""
        try:
            found = await asyncio.get_running_loop().run_in_executor(None, updater.check, HERE)
        except Exception as exc:  # noqa: BLE001 - said back
            debug("update", error=str(exc), detail=getattr(exc, "detail", ""))
            return f"Checking for updates failed: {exc}." + SAY
        if not found["newer"]:
            return f"Zana {found['current']} is the newest version." + SAY
        updater.save_state(HOME, offered=found["version"], offered_at=time.time())
        return (f"Zana {found['version']} is out; this computer has Zana {found['current']}. Ask if they want to "
                "update now; update_zana installs it.") + SAY

    @function_tool()
    async def update_zana(self, context: RunContext):
        """Update Zana to its newest version: download it, check its signature, then restart Zana with it. You
        restart too, and you're back by yourself in about a minute. Only when the user asked for the update or
        said yes to it."""
        if self.updating:
            return "The update is already under way." + SAY
        self.updating = True
        loop = asyncio.get_running_loop()
        try:
            found = await loop.run_in_executor(None, updater.check, HERE)
            if not found["newer"]:
                self.updating = False
                return f"Zana {found['current']} is already the newest version." + SAY
            if not updater.may_install(updater.load_state(HOME), found["version"], self.last_heard):
                # Zana restarts only after the user was told the version and has answered since: not on a
                # model's own initiative, and not on text that only claims they agreed.
                self.updating = False
                updater.save_state(HOME, offered=found["version"], offered_at=time.time())
                return (f"Zana {found['version']} is out; this computer has Zana {found['current']}. Ask the user to "
                        "confirm, and call update_zana again once they say yes.") + SAY
            staged = await loop.run_in_executor(None, updater.stage, found, HOME)
        except Exception as exc:  # noqa: BLE001 - said back; Zana stays as it is
            self.updating = False
            debug("update", error=str(exc), detail=getattr(exc, "detail", ""))
            return f"The update didn't work: {exc}. Zana stays as it is." + SAY
        debug("update", installing=found["version"], current=found["current"], staged=str(staged))
        # A fixed line, spoken before Zana quits; the new app goes in once it has played.
        line = context.session.say("The update checks out, so I'm restarting now and I'll be right back.",
                                   allow_interruptions=False)
        line.add_done_callback(lambda _: self.finish_update(found, staged))
        return llm.ToolResult(f"Zana {found['version']} is checked and goes in as soon as you finish your line; "
                              "Zana then restarts with you in it.", reply_required=False)

    def finish_update(self, found: dict, staged) -> None:
        """Put the checked app in place and restart Zana with it; a failure is said instead, and Zana stays."""
        app = Path(found["app"])
        try:
            updater.install(app, staged)  # puts the old app back itself when it fails
        except Exception as exc:  # noqa: BLE001 - said back
            self.update_failed(exc)
            return
        try:
            updater.save_state(HOME, done={"from": found["current"], "to": found["version"], "t": time.time()})
            updater.start_restart(app, HOME)
        except Exception as exc:  # noqa: BLE001 - the old app goes back; said back
            try:
                updater.roll_back(app)
            except Exception as back:  # noqa: BLE001 - logged; the new app stays in place
                debug("update", error=f"putting the old app back failed: {back}")
                self.updating = False
                self.session.say(f"Zana {found['version']} is in place but couldn't restart, so it starts the next "
                                 "time you open Zana.")
                return
            self.update_failed(exc)

    def update_failed(self, exc: Exception) -> None:
        self.updating = False
        debug("update", error=str(exc), detail=getattr(exc, "detail", ""))
        self.session.say(f"Sorry, the update didn't go in ({exc}), so Zana stays as it is.")

    def greet_after_update(self, done: dict) -> None:
        """First words after an update restart: back, on the version now running. The user was talking to you
        a moment ago, so you start awake."""
        self.asleep, self.last_heard = False, time.time()
        app = updater.installed_app(HERE)
        now = updater.version(app) if app else "?"
        debug("update", updated=now, wanted=done.get("to"), previous=done.get("from"))
        if now == done.get("to"):
            self.say_all(f"I'm back, and Zana is now {now}.")
        else:
            self.say_all(f"Zana restarted, but it's still {now}, so the update didn't take.")


JOBS = Jobs(HOME, TERMINAL_TIMEOUT)


server = AgentServer(port=0, num_idle_processes=1, load_threshold=float("inf"))


def prewarm(proc: JobProcess):
    proc.userdata["vad"] = silero.VAD.load()


server.setup_fnc = prewarm


@server.rtc_session()
async def entrypoint(ctx: agents.JobContext):
    # The room connects in session.start below; look the participant up only when publishing.
    orb = OrbState(lambda attributes: ctx.room.local_participant.set_attributes(attributes))
    ctx.add_shutdown_callback(orb.close)

    def publish(kind, **data):
        payload = json.dumps({"kind": kind, "t": time.time(), **data}, default=str).encode()
        asyncio.create_task(ctx.room.local_participant.publish_data(payload, reliable=True, topic="events"))

    agent = Voicebot(read_json("config.json"), publish)
    await agent.load_mcp()
    # With a voice print set up, only that voice interrupts (maybe_barge_in): the built-in rule, where any speech
    # pauses Voicebot and cuts the reply and its remaining tool steps, is off, and audio heard while Voicebot
    # talks still reaches speech-to-text. "interrupt": "anyone" in config.json keeps the built-in rule.
    agent.voice_gated = agent.interrupt == "voice" and agent.voiceprint.enrolled
    interruption = {"mode": "vad"}
    if agent.voice_gated:
        interruption.update(enabled=False, discard_audio_if_uninterruptible=False)
    transforms = ["filter_markdown", "filter_emoji", speakable_stream]
    if os.getenv("VOICEBOT_ONE_SENTENCE", "0") != "0":
        transforms.append(one_sentence)

    async def log_spoken(text):
        """Last transform: logs exactly what goes to speech, one line per reply (logs/agent.log)."""
        spoken = []
        async for chunk in text:
            spoken.append(chunk)
            yield chunk
        log.info("spoken: %s", "".join(spoken).strip())
    transforms.append(log_spoken)
    session = AgentSession(
        vad=ctx.proc.userdata["vad"],
        stt=openai.STT(base_url=VOICE, api_key="local", model="parakeet", language="en"),
        llm=make_llm(),
        tts=make_tts(),
        tts_text_transforms=transforms,
        max_tool_steps=8,
        # local speech models can stall when the computer is busy; speak late rather than drop the reply
        conn_options=SessionConnectOptions(stt_conn_options=APIConnectOptions(timeout=45),
                                           tts_conn_options=APIConnectOptions(timeout=45)),
        turn_handling={"turn_detection": inference.TurnDetector(version="v1-mini"), "interruption": interruption},
    )

    @session.on("tool_execution_updated")
    def _on_tool_execution(ev):
        try:
            orb.tool_execution_updated(ev)
        except Exception:  # noqa: BLE001 - display updates must not interrupt tool execution
            log.exception("Could not update orb tool state")

    @session.on("function_tools_executed")
    def _on_tools(ev):
        calls = getattr(ev, "function_calls", []) or []
        outs = getattr(ev, "function_call_outputs", []) or []
        for i, call in enumerate(calls):
            out = outs[i] if i < len(outs) else None
            args = getattr(call, "arguments", "")
            try:
                args = json.loads(args)
            except (TypeError, ValueError):
                pass
            log.info("tool %s %s", getattr(call, "name", "?"), args)
            debug("tool", name=getattr(call, "name", "?"), args=args,
                  output=(getattr(out, "output", "") if out else "")[:2000])
            publish("tool", name=getattr(call, "name", "?"), args=args,
                    output=(getattr(out, "output", "") if out else "")[:3000])

    @session.on("conversation_item_added")
    def _on_item(ev):
        if getattr(ev.item, "role", None) == "assistant":
            said = " ".join((getattr(ev.item, "text_content", "") or "").replace(TOOL_SAY, " ").split())
            TRANSCRIPT.record("Voicebot", said)
            if said.strip():
                debug("said", text=said, interrupted=bool(getattr(ev.item, "interrupted", False)),
                      context_chars=context_chars(agent.chat_ctx))

    spoke_error_at = [0.0]

    @session.on("error")
    def _on_error(ev):
        err = getattr(ev, "error", ev)
        debug("error", source=type(getattr(ev, "source", None)).__name__, error=str(err)[:1000])
        # A failing chat model would leave Voicebot silent: say why (speech is local), at most every 30 s.
        if getattr(err, "type", "") == "llm_error" and not getattr(err, "recoverable", True) \
                and time.time() - spoke_error_at[0] > 30:
            spoke_error_at[0] = time.time()
            found = re.search(r"'message': '([^']+)'", str(err))
            reason = (found.group(1) if found else "it did not answer").rstrip(".")
            message = f"My chat model failed: {reason}; check Preferences, Model."
            try:
                session.say(message, add_to_chat_ctx=False)
                debug("said", text=message, interrupted=False)
            except RuntimeError:
                pass  # the conversation is closing

    @session.on("user_state_changed")
    def _on_user_state(ev):
        state = str(getattr(ev, "new_state", ""))
        orb.user_changed(state)
        if state == "speaking":
            agent.tap.start()
            agent.maybe_barge_in()
        else:
            agent.tap.end()

    @session.on("agent_state_changed")
    def _on_state(ev):
        state = str(getattr(ev, "new_state", ""))
        orb.agent_changed(state)
        debug("state", state=state)
        if state in ("thinking", "speaking") and agent.tap.pending():
            agent.maybe_barge_in()  # a reply starts while the user's words are still being heard or turned in
        if state == "listening":
            agent.maybe_compact()

    @session.on("close")
    def _on_close(_ev):
        JOBS.stop_all()
        if agent.watch:
            agent.watch.cancel()
        asyncio.create_task(orb.close())

    async def on_text(sess: AgentSession, ev) -> None:
        """Typed messages pass the same wake-word gate and transcript as speech."""
        extra = agent.gate(ev.text)
        if extra is None:
            return
        if is_goodbye(ev.text):
            await agent.sign_off()
            return
        if agent.answer_voiceprint_request(ev.text):
            return
        notes = agent.feature_notes(ev.text)
        async with sess._claim_user_turn():
            # typed text is the user's own: it stops Voicebot even when speech can't (voice-checked interruptions)
            await sess.interrupt(force=agent.voice_gated)
            agent.play_filler()
            if notes:
                sess.generate_reply(user_input=ev.text + extra,
                                    instructions="Context for this turn (live state and how-to):\n" + notes)
            else:
                sess.generate_reply(user_input=ev.text + extra)

    log.info("conversation in %s: wake word %r, terminal %s, %d tools", ctx.room.name,
             agent.wake_word, agent.terminal, len(agent.tools))
    debug("session", voiceprint="on" if agent.voiceprint.enrolled else "not set up", room=ctx.room.name, model=session.llm.model if hasattr(session.llm, "model") else "?",
          vision_model=os.getenv("VISION_MODEL") or "(same as chat model)", wake_word=agent.wake_word,
          terminal=agent.terminal, interrupt="your voice" if agent.voice_gated else "anyone",
          tools=sorted(t.id for t in agent.tools))
    await session.start(room=ctx.room, agent=agent, room_options=room_io.RoomOptions(
        text_input=room_io.TextInputOptions(text_input_cb=on_text)))
    orb.start()
    if done := updater.take_done(HOME):
        agent.greet_after_update(done)
    if agent.asleep:
        publish("sleep", asleep=True, wake_word=agent.wake_word)
    asyncio.create_task(agent.refresh_fillers())
    agent.watch = asyncio.create_task(agent.watch_updates())


if __name__ == "__main__":
    agents.cli.run_app(server)
