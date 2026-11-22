# Zana Privacy Policy

Last updated: May 2026

## Our Commitment

Zana is designed with privacy as a core principle. Local transcription runs on
your Mac first. Optional rewrite features can send transcript context to a
provider only when you enable and configure them.

## Data Collection

### What We Don't Collect

- **Audio recordings**: audio is processed locally for transcription and is not
  uploaded by Zana
- **Transcriptions**: transcribed text stays on your device unless optional
  rewrite is enabled
- **Personal information**: We don't collect names, emails, or identifiers
- **Usage analytics**: We don't track how you use the app
- **Telemetry**: No data is sent to our servers

### What Stays on Your Device

- Audio recordings (temporarily, during transcription)
- Transcribed text (copied to clipboard)
- Whisper AI models (downloaded once, stored locally)
- App preferences and settings
- Optional rewrite provider configuration, if you enable rewrite

## Local Processing

Zana uses Whisper.cpp, an open-source speech recognition model that runs entirely on your Mac. This means:

1. Your voice is captured by your microphone
2. Audio is processed by Whisper.cpp on your CPU/GPU
3. Transcribed text is pasted at your cursor
4. Audio is discarded immediately after transcription

No network connection is required for transcription after the selected model is
downloaded.

## Permissions

Zana requires these macOS permissions:

- **Microphone**: To capture your voice for transcription
- **Accessibility**: To monitor the Fn key and simulate keyboard input

These permissions are used solely for the app's core functionality.

## Third-Party Services

By default, Zana does not send audio or transcript content to third-party
services. Network activity can include:

- Downloading Whisper models on first run (from Hugging Face)
- Checking for app updates (from GitHub's public release list: Preferences shows
  an update, and Voicebot looks soon after it starts and every 6 hours, asks
  before installing, and downloads the release's disk image from GitHub)
- Sending transcript, clipboard/profile context, or screenshot context to your
  configured rewrite provider when optional rewrite is enabled
- Voicebot Mode (off by default, macOS): sending what you say after the wake
  word, the unread transcript, tool results, window titles and, when you ask it
  to look, a screenshot to the chat and vision models you configure in
  Voicebot's `.env` (OpenRouter by default; any OpenAI-compatible endpoint,
  including a local one, works). Speech-to-text and text-to-speech stay on your
  Mac. Turning it on downloads its Python packages, models and voices.

### Voicebot Mode

Voicebot keeps an always-on transcript of everything it hears, in
`~/Library/Application Support/Zana/voicebot/transcript.md`, including speech
it does not answer. Nothing it hears reaches a model until you say the wake
word (if you set one). Optional local sensory memory (`sensory_memory.npz` and
`sensory_memory.json`, mode `0600`) stores only compact 64-number embeddings and
associative weights on your Mac—never raw microphone audio or screenshots. Its
optional Terminal Tool and the MCP servers you add
can run programs on your Mac; text on your screen can try to steer the model,
so turn them on only if you accept that.

## Data Storage

All data is stored locally in:

- `~/Library/Application Support/Zana/` - settings, models, local memory, and
  related app data

You can delete this data at any time by removing these directories.

## Children's Privacy

Zana does not knowingly collect any information from children under 13.

## Changes to This Policy

We may update this privacy policy occasionally. Changes will be noted in the app's release notes.

## Contact

For privacy concerns, contact: privacy@zana.app

## Open Source

Zana is open source. You can audit the code in this repository.
