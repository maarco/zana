# Voicebot

Zana's optional voice agent (macOS, Apple Silicon). Turn it on in Preferences >
Voicebot > Voicebot Mode; Zana restarts, installs what it needs, starts the
processes below and shows a floating orb.

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
| `config.json` | Terminal switch and wake word, written by Zana |
| `mcp.json` | MCP servers, Claude Desktop format |
| `transcript.md` | Always-on transcript of everything heard and said |
| `logs/` | `debug.jsonl` (the Debug window), plus each process's log |
| `venv/`, `voices/` | The Python environment and VibeVoice voices |

`.env` settings (all optional except a key for the default provider):

```
OPENROUTER_API_KEY=...            # chat model provider key
OPENROUTER_MODEL=nvidia/nemotron-3.5-super-vl:free
LLM_BASE_URL=... LLM_MODEL=... LLM_API_KEY=...        # any OpenAI-compatible endpoint instead
VISION_MODEL=...                  # model that reads screenshots (default: the chat model)
TTS_BASE_URL=... TTS_MODEL=... TTS_VOICE=en-Emma_woman   # TTS override (default: local VibeVoice)
```

## What it does

- **Wake word** (Preferences): Voicebot sleeps until it hears it. Everything you
  say is still written to `transcript.md`; on the wake word it gets the unread
  part, so it knows what you said while it slept.
- **Voice print**: say "set up my voice print" and talk for about ten seconds;
  from then on Voicebot only answers your voice (SpeechBrain ECAPA, run locally;
  only a 192-number voice signature is saved, never audio). Other voices still
  go to the transcript, marked "Other". Clear it in Preferences.
- **Goodbye**: "goodbye", "stop listening" or "that's all" puts it to sleep
  (wake word set) or turns the mic off.
- **One sentence at a time**: replies are cut to one sentence (a short opener
  like "I'm listening." is skipped). Numbers are spoken as words; paths, links,
  code and JSON are never read out.
- **Tools**: `see_screen`, `look_at_screen` (vision model), `type_up` (pastes at
  your cursor), `stop_listening` (also said with "goodbye"), `run_terminal` (only with the Terminal Tool
  switch), and `list/add/remove/reload_mcp_servers` (adding and removing need
  the Terminal Tool switch). When the model goes straight to a tool, Voicebot
  says "On it." while the tool runs, then answers.
- **Orb**: drag to move, click to mute, double-click for a new conversation
  (applies prompt, MCP and switch changes), right-click for the Debug window.
- **Debug window**: every step live: what it heard (and whether it ignored it),
  what it said, each tool call with arguments and output, wake and sleep,
  model and errors.

## Tests

```
python -m unittest discover -s voicebot/tests -p "test_*.py"   # CI: no installs needed
"<Voicebot folder>/venv/bin/python" voicebot/tests/e2e_probe.py agent   # live, needs Voicebot running
"<Voicebot folder>/venv/bin/python" voicebot/tests/e2e_probe.py zana    # Zana's handlers, live
```

The live check plays Zana in a fresh room and drives the real agent with typed
turns; it calls the real chat model.
