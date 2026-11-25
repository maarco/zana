# Privacy Policy

Last updated: November 21, 2026

Zana is a local-first macOS voice assistant. Voicebot is enabled by default.
Speech recognition and spoken replies use local models by default, but the text
conversation and any information returned by tools can be sent to the model
provider you configure. Zana does not operate a central service that receives
your conversations.

## Voice and transcripts

Voicebot uses the microphone while it is running. A local LiveKit server passes
the audio to the local Parakeet speech-to-text model; microphone audio is not
sent to the chat model. Voicebot writes a transcript of what it hears and says to
`~/Library/Application Support/Zana/voicebot/transcript.md`. Dated lines older
than the configured retention window are pruned when a conversation starts;
the default is 30 days. Preferences > Voicebot lets you change the window or
use Clear Transcript while Voicebot is running.

By default, the wake word is empty, so Voicebot answers each recognized turn.
If you set a wake word in Preferences > Voicebot, speech is still transcribed
and saved while Voicebot is asleep. When it hears the wake word, the unread
transcript text is included in the request to the chat model. This lets it
respond to what you said while it was asleep; it also means nearby speech may
be included in that text.

Replies are spoken with the local VibeVoice model by default. If you configure
`TTS_BASE_URL` in the Voicebot `.env` file, reply text is sent to that speech
endpoint instead. Voicebot can also save a local voice signature if you ask it
to set up a voice print. The signature is used on-device; raw enrollment audio
is not saved as the voice print.

## Information sent to model providers

The API URL and API key in Preferences > Model select the OpenAI-compatible
chat endpoint. Requests can include your recognized speech, recent conversation
text, the Voicebot system prompt, and tool results. If you enable the Terminal
Tool or configure MCP servers, their returned output can also be included in
chat requests. The Terminal Tool is off by default.

When Voicebot uses `see_screen`, it reads the frontmost app and window title;
that text, and any configured screen-history command output, is returned to the
chat model. When Voicebot uses `look_at_screen`, Zana captures a screenshot,
scales it locally, and sends it to the configured Vision Model. If no separate
Vision Model is set, the chat model is used. Screen content can contain
sensitive information, so only ask Voicebot to inspect a screen when you are
comfortable sharing that content with the selected endpoint.

These requests go to the API URL you choose. It may be a remote provider or a
local server. For a remote provider, its privacy, logging, and retention
policies apply; Zana cannot control what that provider keeps. API keys are
stored in Zana's local settings. You can change the provider or use a local
model endpoint in Preferences > Model.

## Local files and downloads

Voicebot stores its files in `~/Library/Application Support/Zana/voicebot/`.
This includes the transcript, prompt, configuration, optional `.env` and MCP
settings, logs, voice assets, and a conversation-scoped screenshot
`screen-<conversation-id>.jpg` for the active conversation. The screenshot and
that conversation's terminal job logs are removed when the conversation ends.
Zana's other settings and application data are also stored locally under
`~/Library/Application Support/Zana/`. Use Preferences > Voicebot > Open Folder
to inspect the Voicebot files. Operational history is bounded: `logs/debug.jsonl`
keeps at most 20 MB, each terminal job log keeps at most 5 MB, and prompt history
keeps the 20 newest prompt backups. Transcript search reads bounded chunks off the
agent event loop; age pruning removes only dated transcript lines, while Clear
Transcript removes all local transcript text and resets its unread marker.

On first launch, Zana downloads Python packages, local speech models, and voice
assets needed to run Voicebot. The app checks GitHub for release information
soon after startup and every six hours; it asks before installing an update.
These downloads and release checks do not include your microphone audio or
transcript. Zana does not send usage analytics or telemetry to a Zana service.

## macOS permissions

- Microphone: lets Voicebot capture speech for local transcription.
- Accessibility: lets Voicebot paste text and send keyboard presses when you
  use its computer-control tools. It is not required for speech recognition.
- Screen Recording: lets Voicebot capture a screenshot when you ask it to look
  at your screen. It is not required for voice conversations.
- Automation: macOS may ask to let Zana control System Events for keyboard
  actions or to read the frontmost app and window title.

You can review or revoke these permissions in System Settings > Privacy &
Security. If you delete local transcript or screenshot files, that does not
remove information already sent to a model provider; contact that provider for
its data-retention controls.
