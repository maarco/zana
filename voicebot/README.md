# Voicebot

Zana's voice agent (macOS, Apple Silicon). Zana starts it on launch: it installs
what it needs, starts the processes below and shows a floating orb. Fn dictation
and Whisper are retired.

## How it runs

Zana supervises three processes (code in this folder, bundled into the app):

| Process | What it does |
|---|---|
| `livekit-server` | Local LiveKit room server; random keys and free ports on every start |
| `voice_server.py` | Local speech: Parakeet speech-to-text, VibeVoice text-to-speech |
| `agent.py` | The agent: wake word, transcript, chat model, tools, MCP servers |

The orb joins a fresh room as your mic and speaker. Zana mints the join token
itself and answers the agent's calls: `zana.paste`, `zana.screen`,
`zana.screenshot`, `zana.mute`. Quitting Zana stops all three processes; a crash
leaves a `pids` file that the next launch cleans up.

Prerequisites: `brew install uv livekit`. The first start creates a Python 3.12
environment (about 1.5 GB) and downloads the speech models and voices.

## Your files

Everything per-user lives in the Voicebot folder,
`~/Library/Application Support/Zana/voicebot/` (Preferences > Voicebot > Open Folder):

| File | What it is |
|---|---|
| `.env` | Keys and models (never committed; see below) |
| `system_prompt.md` | The system prompt (edit it in Preferences) |
| `config.json` | Terminal switch, wake word and chat model, written by Zana |
| `mcp.json` | MCP servers, Claude Desktop format |
| `transcript.md` | Always-on transcript of everything heard and said |
| `logs/` | `debug.jsonl` (the Debug window), plus each process's log |
| `fillers/` | Cached thinking-sound audio and its manifest |
| `venv/`, `voices/` | The Python environment and VibeVoice voices |

Preferences > Model sets the chat model (API key, model, OpenAI-compatible API
URL), its reasoning level and the vision model; Preferences > Voicebot sets the
voice. Zana writes them into `config.json`, readable only by you, and a change
starts a fresh conversation so it applies at once. When the model fails,
Voicebot says why out loud. Test AI sends the request Voicebot sends.

`.env` settings (all optional, for special setups):

```
VISION_BASE_URL=... VISION_API_KEY=...        # vision model on another provider
TTS_BASE_URL=... TTS_MODEL=...                # another OpenAI-compatible speech server
OPENROUTER_API_KEY=... OPENROUTER_MODEL=...   # only used when Preferences > Model has no model
```

## What it does

- **Wake word** (Preferences): Voicebot sleeps until it hears it. Everything you
  say is still written to `transcript.md`; on the wake word it gets the unread
  part, so it knows what you said while it slept.
- **Voice print**: say "set up my voice print" and talk for about ten seconds;
  from then on Voicebot only answers your voice (SpeechBrain ECAPA, run locally;
  only a 192-number voice signature is saved, never audio). Other voices still
  go to the transcript, marked "Other" (while Voicebot is talking they are
  dropped instead). Clear it in Preferences.
- **Only your voice interrupts**: from the first conversation after a voice
  print is set up, other voices (a TV, people in the room) no longer stop
  Voicebot mid-reply or mid-tool-step; your voice does, checked after half a
  second of speech (a one-word "stop" is checked when it ends). "Let anyone
  interrupt you"
  (`change_setting interrupt anyone`) switches back to any voice, and
  `interrupt voice` turns it on again; either applies from the next
  conversation.
- **Goodbye**: "goodbye", "stop listening" or "that's all" puts it to sleep
  (wake word set) or turns the mic off.
- **One sentence at a time**: replies are cut to one sentence (a short opener
  like "I'm listening." is skipped). Numbers are spoken as words; paths, links,
  code and JSON are never read out.
- **Thinking sounds**: short clips are cached in the selected voice and retain
  their quiet endings. When enabled, clips made with older audio processing are
  regenerated in the background when Voicebot starts; the old cache remains
  available until the replacement is ready. Configure them in Preferences > Voicebot.
- **Tools**: `see_screen`, `look_at_screen` (vision model), `type_up` (pastes at
  your cursor), `press_keys`, `stop_listening` (also said with "goodbye"),
  `search_conversations` (the transcript of every conversation), `run_terminal`,
  `run_applescript` and `check_job`/`stop_job`/`list_jobs` (only with the Terminal
  Tool switch), and `list/add/remove/reload_mcp_servers` (adding and removing need
  the Terminal Tool switch). Every tool call carries a required "say" field the
  model writes for that call ("Let me check your Downloads folder"); it is spoken
  while the tool runs and never reaches the tool.
- **Terminal jobs**: a command still running after three seconds keeps going as
  a job; its output streams to `jobs/<n>.log`, Voicebot gets the job number and
  process id, and it is told when the job ends. Long-job replies warn that the
  named job stops when the conversation closes (including orb close or Voicebot
  settings changes) and cannot be resumed later. Jobs also stop after 15 minutes
  or when explicitly stopped.
- **Compaction**: past 12,000 characters (besides the instructions), the older
  part of the conversation is folded into a summary and the last three turns stay
  word for word; the Debug window shows each compaction and every reply shows the
  conversation size. The transcript file keeps everything.
- **Orb**: drag to move, click to mute, double-click for a new conversation
  (applies prompt, MCP and switch changes), right-click for the Debug window.
- **Debug window**: every step live: what it heard (and whether it ignored it),
  what it said, each tool call with arguments and output, wake and sleep,
  model and errors.
- **Updates**: soon after it starts and every 6 hours, Voicebot looks for a newer
  release on GitHub and asks once per version whether to update (or say "update
  Zana"). `update_zana` downloads the release's disk image, checks its checksum,
  that it is signed by the same developer as the running app and notarized, and
  its version, then quits Zana, swaps the app in place and opens it again; Voicebot
  comes back and says the version. Any failure keeps the old app. The result is
  in `update.json` and `logs/update.log`.

## Tests

```
python -m unittest discover -s voicebot/tests -p "test_*.py"   # CI: no installs needed
"<Voicebot folder>/venv/bin/python" voicebot/tests/e2e_probe.py agent   # live, needs Voicebot running
"<Voicebot folder>/venv/bin/python" voicebot/tests/e2e_probe.py zana    # Zana's handlers, live
```

The live check plays Zana in a fresh room and drives the real agent with typed
turns; it calls the real chat model.
