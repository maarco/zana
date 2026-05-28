# Changelog

All notable user-facing changes will be documented here.

This project follows SemVer once public releases begin.

## Unreleased

- Hardened recording lifecycle cleanup for quick hotkey releases and failed
  audio start/stop paths.
- Added hidden orb animation parking to reduce background WebView work.
- Added single-instance protection to prevent duplicate resident app launches.
- Moved Whisper transcription work onto a blocking worker thread.
- Added guarded macOS release scripts for stable Developer ID signing,
  notarization, and tag-driven GitHub releases.
- Clarified optional rewrite privacy behavior.
