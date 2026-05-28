# Contributing to Zana

Zana is a Tauri 2 desktop app with a Rust backend and vanilla HTML/CSS/JS UI.
The active workspace member is `src-tauri`; the root `src/` tree is historical
unless it is intentionally reactivated.

## Setup

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo install tauri-cli --version "^2" --locked
cargo check -p Zana-app --manifest-path src-tauri/Cargo.toml --locked
```

## Quality Gates

Run these before opening a PR:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo check -p Zana-app --manifest-path src-tauri/Cargo.toml --locked
cargo test -p Zana-app --manifest-path src-tauri/Cargo.toml --locked
cargo clippy -p Zana-app --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
```

## Privacy Rules

- Local Whisper transcription must remain the first step.
- Do not add telemetry or uploaded audio.
- Optional rewrite/network behavior must stay opt-in and documented in
  `PRIVACY.md`.
- If a change touches microphone, accessibility, paste-at-cursor, rewrite, or
  screenshots, update tests and docs together.

## Pull Requests

Keep PRs focused. Include:

- what changed
- checks run
- manual macOS QA when permissions, packaging, or hotkeys are affected
- screenshots or screen recordings for visible UI changes

Do not commit generated bundles, Whisper models, certificates, local app data,
logs, `.DS_Store`, or secrets.
