# Zana Privacy Policy

Last updated: January 2025

## Our Commitment

Zana is designed with privacy as a core principle. Your voice data never leaves your device.

## Data Collection

### What We Don't Collect

- **Audio recordings**: All audio is processed locally and never transmitted
- **Transcriptions**: Your transcribed text stays on your device
- **Personal information**: We don't collect names, emails, or identifiers
- **Usage analytics**: We don't track how you use the app
- **Telemetry**: No data is sent to our servers

### What Stays on Your Device

- Audio recordings (temporarily, during transcription)
- Transcribed text (copied to clipboard)
- Whisper AI models (downloaded once, stored locally)
- App preferences and settings

## Local Processing

Zana uses Whisper.cpp, an open-source speech recognition model that runs entirely on your Mac. This means:

1. Your voice is captured by your microphone
2. Audio is processed by Whisper.cpp on your CPU/GPU
3. Transcribed text is pasted at your cursor
4. Audio is discarded immediately after transcription

No network connection is required for transcription.

## Permissions

Zana requires these macOS permissions:

- **Microphone**: To capture your voice for transcription
- **Accessibility**: To monitor the Fn key and simulate keyboard input

These permissions are used solely for the app's core functionality.

## Third-Party Services

Zana does not integrate with any third-party services. The only network activity is:

- Downloading Whisper models on first run (from Hugging Face)
- Checking for app updates (optional, from GitHub)

## Data Storage

All data is stored locally in:

- `~/.Zana/` - Configuration files
- `~/Library/Application Support/Zana/` - Whisper models

You can delete this data at any time by removing these directories.

## Children's Privacy

Zana does not knowingly collect any information from children under 13.

## Changes to This Policy

We may update this privacy policy occasionally. Changes will be noted in the app's release notes.

## Contact

For privacy concerns, contact: privacy@Zana.dev

## Open Source

Zana is open source. You can audit the code at: https://github.com/Zana/Zana
