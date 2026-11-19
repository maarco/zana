# Changelog

All notable user-facing changes will be documented here.

This project follows SemVer once public releases begin.

## Unreleased

## 0.1.5

- Fixed: Voicebot never answered in 0.1.4; its conversation crashed as it
  started.
- Orb designs: Strands, Halo, Fur, Nebula and Liquid next to the original,
  picked in Preferences > Voicebot > Orb. Voicebot can also show its mood
  through the orb (calm, focused, warm, playful, dreamy, cool), at most every
  few minutes.
- The orb pulses red when there is no working microphone for a few minutes.
- Voicebot knows its own settings and can change its voice, model, reasoning,
  thinking sounds, orb and more when you ask; it cannot change the API key or
  turn its terminal access on.

## 0.1.4

- Zana is now Voicebot, a hands-free voice agent with a floating orb, in place
  of Fn dictation and Whisper. Speech stays local (Parakeet and VibeVoice); the
  chat model is the one you set in Preferences > Model. It can see your screen,
  type for you, press keys, run terminal commands and use MCP servers, and it
  says what it is doing while it works.
- Preferences: Voicebot first (wake word, voice, thinking sounds, voice print,
  terminal tool, system prompt, MCP servers), then Model (provider, model,
  reasoning, vision model) and General.
- Fixed a crash when saving Preferences.
- Fixed memory growth in long Voicebot sessions.
- Releases start by pushing a version tag.

## 0.1.3

- Restyled Preferences, Transcriptions and About: neutral gray surfaces with
  no borders or orange accents, and a greyscale app icon.
- Frameless windows with rounded corners, macOS traffic lights or Windows
  caption buttons, Command/Ctrl-W and -M shortcuts, and header dragging.
- Preferences: controls sit inline on each row, keyboard-operable switches,
  show/hide for the API key, an Unsaved changes status and Command-S to save.
- Transcriptions: search, Today/Yesterday grouping, copy confirmation and
  clearer empty states.
- About shows the real app version instead of a fixed 1.0.0.
- Release workflow now runs from main for an existing tag, checks the Apple
  notarization login first and supports App Store Connect API keys.

## 0.1.2

- Save every transcription to local history, not only AI-polished ones;
  local-only dictation was previously discarded.
- Moved transcript history into its own window, opened from Preferences or the
  tray menu.
- Fixed the Preferences Save button being clipped off the bottom of the window.
- Finalized the editable two-message rewrite prompt (system + user prompt) and
  removed the hidden response-contract layer.
- Hardened recording lifecycle cleanup for quick hotkey releases and failed
  audio start/stop paths.
- Added hidden orb animation parking to reduce background WebView work.
- Added single-instance protection to prevent duplicate resident app launches.
- Moved Whisper transcription work onto a blocking worker thread.
- Added guarded macOS release scripts for stable Developer ID signing,
  notarization, and tag-driven GitHub releases.
- Clarified optional rewrite privacy behavior.
