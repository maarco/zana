# Agents Guide

This file gives AI/code agents the local context needed to work safely in this repository.

## Project Snapshot

Zana is a local-first voice-to-text desktop app. It records audio, transcribes with Whisper locally, and pastes the result at the cursor. The app is primarily a Tauri 2 desktop app with Rust backend code and vanilla HTML/CSS/JS frontend assets.

Privacy is core to the product: do not add cloud transcription, telemetry, uploaded audio, or network-dependent behavior unless the task explicitly asks for it and the privacy docs are updated.

## Repository Map

- `src-tauri/`: active Tauri app crate, Rust backend, commands, state, audio, STT, hooks, plugins, and platform integration.
- `src-ui/`: app UI assets loaded by Tauri windows and panels; currently plain HTML/CSS/JS, not a bundled frontend app.
- `plugins/`: bundled plugin manifests and plugin examples.
- `docs/`: architecture, API, plugin, onboarding, migration, and implementation notes. Some docs are plans or roadmaps — treat them as planned direction, not proof that code exists.
- `src-tauri/tests/`: the active integration and contract tests. `app_contracts.rs` asserts that specific Preferences UI markers and Tauri commands stay wired together, so renaming a UI id or dropping a command will fail these tests.
- `tests/` (root) and `src/`: legacy trees from the pre-Tauri egui app. Neither is a member of the root Cargo workspace, so default `cargo` commands do not build or test them. Verify relevance before editing; do not assume they are live.
- `scripts/`: macOS build, signing, and notarization helpers.

The root `Cargo.toml` currently declares only `src-tauri` as a workspace member, so default build and test commands exercise the Tauri crate.

## Before Editing

1. Run `git status --short` and preserve unrelated local changes.
2. Read the live codepath before changing it; some docs describe planned or migrated designs.
3. Keep edits scoped to the requested behavior. Do not clean up unrelated files opportunistically.
4. Do not commit generated build outputs, Whisper models, `.DS_Store`, `target/`, or local app data.
5. Avoid changing `Cargo.lock` unless dependencies actually change.

## Common Commands

```bash
# Check the active workspace
cargo check

# Run all tests
cargo test

# Run the contract tests (assert Preferences UI <-> command wiring)
cargo test --test app_contracts

# Run a single focused test by name
cargo test legacy_writing_profile_is_migrated_to_exact_prompt_templates

# Format Rust
cargo fmt

# Lint Rust when available
cargo clippy --all-targets --all-features -- -D warnings

# Run the app
cargo run -p Zana-app

# Run with debug logs
RUST_LOG=debug cargo run -p Zana-app

# Build release binary
cargo build -p Zana-app --release

# Build Tauri app bundle
cargo tauri build
```

For long-running local app tests, the README uses:

```bash
tmux new-session -d -s Zana "cargo run -p Zana-app 2>&1 | tee /tmp/Zana-run.log"
tmux attach -t Zana
```

## Coding Notes

- Prefer existing Rust module boundaries: `audio`, `stt`, `hooks`, `plugins`, `commands`, `state`, `panel` (macOS NSPanel orb window), and `onboarding`.
- Register new Tauri commands in the relevant `commands` module and in the `tauri::generate_handler!` list in `src-tauri/src/main.rs`.
- Keep frontend changes compatible with the current vanilla `src-ui/` setup unless the task explicitly introduces a build pipeline.
- When changing UI-facing command payloads, update Rust types, frontend callers, and docs/tests together.
- The AI rewrite pipeline sends exactly two messages: a system prompt (`WritingProfile.purpose`) and a user prompt (`WritingProfile.tone`). The user prompt must contain `{captured}`, and the model returns text through a single `submit_result` tool call. Do not append hidden system text or a separate "response contract" outside these visible templates; `src-tauri/tests/app_contracts.rs` fails if you do. The legacy `format` field is deprecated, and `WritingProfile::migrate_legacy_fields` upgrades old saved settings on load. Supported prompt variables: `{time}`, `{captured}`, `{clipboard}`, `{screen_shot}`, `{dictionary}`, `{history}`, `{style_memory}`, `{project_memory}`.
- Treat macOS-specific code carefully. Accessibility, microphone permission, NSPanel behavior, and Fn-key handling are user-visible and easy to regress.
- Prefer typed parsing/serialization via `serde`/`toml`/`serde_json` over ad hoc string manipulation.
- Add or update focused tests for nontrivial behavior changes. The project docs expect agents to validate compilation and tests before handing work back.

## Manual QA Triggers

Use manual checks when a change touches:

- Fn-key monitoring or double-tap recording.
- Microphone permissions or audio device selection.
- Whisper model download, cache paths, or transcription flow.
- AI rewrite prompts, prompt variables, or the System/User prompt fields in Preferences.
- The local transcript history panel in Preferences (`get_transcript_history`).
- Auto-paste behavior.
- Floating orb lifecycle, fullscreen overlay behavior, or `src-ui/orb*` assets.
- Plugin discovery, plugin state, or hook event propagation.
- macOS packaging, entitlements, signing, notarization, or `tauri.conf.json`.

## Documentation Expectations

- Update `README.md`, `INSTALL.md`, `PRIVACY.md`, or files under `docs/` when behavior, setup, privacy posture, or extension APIs change.
- Keep user-facing docs aligned with the actual runtime path, not only design specs.
- When a doc is aspirational or a plan, avoid treating it as proof that code exists.

## Handoff Checklist

Before reporting completion:

1. Summarize the files changed.
2. State exactly which checks ran and their results.
3. Call out checks that could not run, especially macOS permission or hardware-dependent tests.
4. Mention any existing unrelated dirty files that were left untouched.
