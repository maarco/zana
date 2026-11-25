# Changelog

All notable user-facing changes will be documented here.

This project follows SemVer once public releases begin.

## Unreleased

## 0.1.7

- First-run setup walks you through Voicebot's permissions and lets you open
  the relevant macOS settings directly.
- Voicebot replies are more conversational. Removing old tool guidance from
  your personal prompt requires consent for that specific cleanup.
- Voicebot startup and shutdown are more reliable, and local diagnostic logs
  are bounded.
- The macOS installer now targets Apple silicon only.

## 0.1.6

- Voicebot tells you when a new Zana release is out and installs it when you
  say yes. It checks GitHub about 30 seconds into each conversation and every
  6 hours after that, and asks once per version. On a yes it downloads the
  release, checks its signature and notarization, swaps the app in and
  restarts Zana, then says the new version. A failed download or check leaves
  Zana as it is. Install 0.1.6 by hand once; later releases come through
  Voicebot, as long as Zana is in a folder you can write to, such as
  Applications.
- With a voice print set up, other voices no longer interrupt Voicebot: a TV
  or other people can't stop a reply or cut a tool step short; your voice
  still can. This starts with the first conversation after the voice print is
  set up. To let anyone interrupt again, ask Voicebot to let anyone interrupt
  it; that also starts with the next conversation.
- Fixed: with an OpenRouter model, every reply failed with "My chat model
  failed" once the balance couldn't cover the model's whole output length,
  although replies are short. Replies now ask for at most 16,384 tokens, so
  a balance that covers that much still answers.
- Fixed: with a voice print set up, speech too short to check against it was
  answered anyway, so a TV line could get a reply. Now it is answered only if
  the voice print matched your voice in the 3 seconds before it.
- Fixed: Voicebot's built-in lines (written by the app, not the model) were
  cut short. The voice print prompt stopped after its first sentence, and the
  goodbye lost its opening "Goodbye.".
- Fixed: the spoken description of a tool step no longer gets an extra period
  after a ! or ?.

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
