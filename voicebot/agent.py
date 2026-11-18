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

from dotenv import load_dotenv
from livekit import agents, rtc
from livekit.agents import (
    Agent, AgentServer, AgentSession, APIConnectOptions, JobProcess, RunContext, StopResponse,
    function_tool, get_job_context, inference, llm, mcp,
)
from livekit.agents.voice import room_io
from livekit.agents.voice.agent_session import SessionConnectOptions
from livekit.plugins import openai, silero

from fillers import DEFAULT_PHRASES, Fillers, chunks_with_gap
from jobs import Jobs
from listening import (ToolCallText, Transcript, clean, heard_wake_word, is_goodbye, parse_text_tool_calls,
                       repeats, split_say, topics, wants_redo, wants_voiceprint)
from voiceprint import RATE, VoicePrint, VoiceTap, to_mono_16k
from speech import TOOL_SAY, one_sentence, speakable_stream

HERE = os.path.dirname(os.path.abspath(__file__))
HOME = os.path.expanduser(os.getenv("VOICEBOT_HOME", os.path.join(HERE, ".home")))
load_dotenv(os.path.join(HOME, ".env"))

VOICE = os.getenv("VOICE_SERVER_URL", "http://127.0.0.1:8800/v1")
FAST = float(os.getenv("VOICEBOT_FAST_SECONDS", "3"))  # slower terminal commands answer "on it" now and report later
TERMINAL_TIMEOUT = float(os.getenv("VOICEBOT_TERMINAL_TIMEOUT", "900"))  # jobs are stopped after 15 minutes
TRANSCRIPT = Transcript(HOME)  # always-on: every line heard and said
FILLERS = Fillers(HOME)  # thinking sounds, made in Voicebot's voice and cached
DEBUG_LOG = os.path.join(HOME, "logs", "debug.jsonl")  # what Voicebot does, for Zana's Debug window
SAY = "\n(Now say one short sentence with the answer.)"
MIN_VOICE_CHECK = int(0.5 * RATE)  # shorter turns are not checked against the voice print
# Every tool, as the model sees it, gets this required field; its text is spoken while the tool runs.
SAY_FIELD = {"type": "string",
             "description": "What you tell the user while this runs: a few words in your own words, specific to "
                            "this call, like \"Let me check your kollab folder\". Vary it from call to call."}


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


STEP_GAP = 1.0  # seconds of quiet before a follow-up tool step speaks, so steps don't fire back to back
SHORT_TURN = 2.0  # seconds of actual speech; short phrases give noisier voice prints, so they get a lower cutoff
ADAPT_SCORE = 0.55  # confident matches this long or longer refine the saved print
COMPACT_CHARS = 12000  # conversation size (besides the instructions) that triggers compaction
KEEP_TURNS = 3  # recent turns kept word for word when compacting
ENROLL_SECONDS = 8

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
    for path in (os.path.join(HOME, "system_prompt.md"), os.path.join(HERE, "system_prompt.default.md")):
        try:
            with open(path) as f:
                return f.read()
        except OSError:
            continue
    return "You are an AI voicebot. Answer in one short spoken sentence."


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
    """Name, system prompt, then the recent conversation from the transcript file (continuity across
    conversations and prompt reloads)."""
    recent = TRANSCRIPT.tail(3000)
    text = f"Your name is {name}.\n\n{load_prompt()}"
    if recent:
        text += ("\n\nRecent conversation, from your transcript file (oldest first; continue from it, and never "
                 "read it out unless asked):\n" + recent)
    return text


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
    return openai.LLM(model=model, base_url=base_url, api_key=api_key,
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
        self.compact_chars = int(config.get("compact_chars") or COMPACT_CHARS)
        self.compact_at = self.compact_chars
        self.compacting = False
        self.enrolling = False
        self.enroll_audio: list = []
        threading.Thread(target=self._warm_voiceprint, daemon=True).start()
        self.base_tools = list(self.tools)
        self.mcp_servers_live: list = []
        self.mcp_tools: list = []

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
        }
        notes = [guides[t] for t in topics(heard) if t in guides]
        if notes:
            debug("context", topics=topics(heard))
        return "\n".join(notes)

    def answer_voiceprint_request(self, heard: str) -> bool:
        """Handle "enroll my voice" in code. True when handled."""
        if not wants_voiceprint(heard):
            return False
        if self.voiceprint.enrolled and not wants_redo(heard):
            self.session.say("Your voice print is already set up. Say redo my voice print to record a new one.")
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
        self.session.say(f"{replacing}Going into voice print mode. Talk naturally for about {ENROLL_SECONDS} seconds.")

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
        self.session.say("Voice print saved. From now on I only answer your voice.")

    def gate(self, heard: str, score: float | None = None) -> str | None:
        """Record what was heard; return what the model should get, or None while asleep."""
        TRANSCRIPT.record("User", heard)
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
                         + unread + "\n(Answer their latest request directly in one sentence; don't say you're listening.)")
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
            cutoff = self.short_threshold if seconds < SHORT_TURN else self.threshold
            if score < cutoff:
                TRANSCRIPT.record("Other", heard)
                debug("heard", text=heard, action=f"ignored (not your voice, match {score:.2f} < {cutoff:.2f}, "
                                                   f"{seconds:.1f} s of speech)")
                raise StopResponse()
            if score >= ADAPT_SCORE and seconds >= SHORT_TURN:
                await asyncio.get_running_loop().run_in_executor(None, self.voiceprint.adapt, embedding)
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
        return " " + TOOL_SAY + opener + ". ".join(s.rstrip(".") for s in said) + ". "

    def play_filler(self) -> None:
        """A random thinking sound while the reply is worked out; a short pause follows it."""
        picked = FILLERS.pick() if self.fillers_on else None
        if not picked:
            return
        path, phrase = picked
        try:
            self.session.say(phrase, audio=filler_audio(path), allow_interruptions=True, add_to_chat_ctx=False)
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
            seen = {item.id for item in snapshot.items}
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
    async def edit_system_prompt(self, context: RunContext, find: str, replace: str) -> str:
        """Edit your own system prompt: replace an exact piece of its text with new text, or leave find empty to
        add replace at the end. It applies right away. Use when the user asks you to change your rules, style,
        personality or anything about how you behave. Call read_system_prompt first if you need the exact wording.

        Args:
            find: Exact text from your prompt to replace (empty to append).
            replace: The new text.
        """
        path = os.path.join(HOME, "system_prompt.md")
        old = load_prompt()
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
        return load_prompt()[:6000] + "\n(Summarize what they asked about in one sentence.)"

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
            self.session.say(f"Goodbye. Say {self.wake_word} when you need me.")
        else:
            await zana("zana.mute")
            self.session.say("Goodbye. Click the orb when you need me.")

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
        it skips hidden folders like ~/.claude) or list the likely folder (ls ~/.claude/skills); never crawl
        the whole home folder with find or grep -r.

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
                "Tell the user in a few words that it's running; check_job shows progress and stop_job stops it. "
                "You'll be told when it finishes.")

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


JOBS = Jobs(HOME, TERMINAL_TIMEOUT)


server = AgentServer(port=0, num_idle_processes=1, load_threshold=float("inf"))


def prewarm(proc: JobProcess):
    proc.userdata["vad"] = silero.VAD.load()


server.setup_fnc = prewarm


@server.rtc_session()
async def entrypoint(ctx: agents.JobContext):
    def publish(kind, **data):
        payload = json.dumps({"kind": kind, "t": time.time(), **data}, default=str).encode()
        asyncio.create_task(ctx.room.local_participant.publish_data(payload, reliable=True, topic="events"))

    agent = Voicebot(read_json("config.json"), publish)
    await agent.load_mcp()
    transforms = ["filter_markdown", "filter_emoji", speakable_stream]
    if os.getenv("VOICEBOT_ONE_SENTENCE", "1") != "0":
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
        turn_handling={"turn_detection": inference.TurnDetector(version="v1-mini"), "interruption": {"mode": "vad"}},
    )

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
        if state == "speaking":
            agent.tap.start()
        else:
            agent.tap.end()

    @session.on("agent_state_changed")
    def _on_state(ev):
        state = str(getattr(ev, "new_state", ""))
        debug("state", state=state)
        if state == "listening":
            agent.maybe_compact()

    @session.on("close")
    def _on_close(_ev):
        JOBS.stop_all()

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
            await sess.interrupt()
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
          terminal=agent.terminal, tools=sorted(t.id for t in agent.tools))
    await session.start(room=ctx.room, agent=agent, room_options=room_io.RoomOptions(
        text_input=room_io.TextInputOptions(text_input_cb=on_text)))
    if agent.asleep:
        publish("sleep", asleep=True, wake_word=agent.wake_word)
    asyncio.create_task(agent.refresh_fillers())


if __name__ == "__main__":
    agents.cli.run_app(server)
