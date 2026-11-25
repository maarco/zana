# Zana

**A hands-free Voicebot for macOS, with local speech recognition and playback**

Zana transcribes speech with Parakeet and speaks replies with VibeVoice locally
by default. Recognized text and tool results go to the chat model you configure.
When requested, the front window title or a screenshot can also be sent to your
configured chat or vision model. The retired Fn-key dictation and Whisper stack
is not part of the current app.

See [INSTALL.md](INSTALL.md) for the current Voicebot setup.

## Features

- **Voicebot by default**: Hands-free conversation with an optional wake word;
  leave it empty to have Voicebot answer every recognized turn.
- **Local speech**: Parakeet speech-to-text and VibeVoice text-to-speech run on
  your Mac by default. Microphone audio is not sent to the chat model.
- **Your model provider**: Choose an OpenAI-compatible chat endpoint and model
  in Preferences > Model; a local model endpoint is also supported.
- **On-demand screen tools**: Voicebot can read the front app/window title and,
  when asked, capture a screenshot for the configured vision model.
- **Computer tools**: Voicebot can paste text and press keys. The optional
  Terminal Tool is off by default; MCP servers can be configured separately.
- **Local transcript**: Voicebot saves everything it hears and says in
  `~/Library/Application Support/Zana/voicebot/transcript.md`, including while a
  configured wake word has it asleep.
- **No usage analytics**: Settings stay local; transcript text and tool data
  are sent only to the model endpoint when used for a request.

## System Requirements

- **macOS**: 13 Ventura or newer recommended
- **Processor**: Apple silicon only (M-series)
- **RAM**: 8GB minimum (16GB recommended for local speech models)
- **Storage**: several GB for the app, Voicebot runtime, and local models

## Installation

### Option 1: Download a Release DMG

1. Download the Zana DMG from GitHub Releases once releases are published
2. Open the DMG and drag Zana to Applications
3. Launch Zana from Applications folder
4. Grant the macOS permissions requested by Voicebot as needed

### Option 2: Build from Source

#### Prerequisites

1. **Rust toolchain** (1.80+)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

2. **Xcode Command Line Tools**
   ```bash
   xcode-select --install
   ```

3. **Tauri CLI**
   ```bash
   cargo install tauri-cli
   ```

#### Build Steps

```bash
# Clone the repository
git clone https://github.com/maarco/zana.git
cd zana

# Build the app (debug mode - faster compile)
cargo build -p Zana-app

# Or build optimized release
cargo build -p Zana-app --release

# Run the app
cargo run -p Zana-app
```

The compiled binary will be at:
- Debug: `target/debug/Zana-app`
- Release: `target/release/Zana-app`

## First Run Setup

On first launch, Zana opens onboarding for the local Voicebot runtime. Keep the
app open while it installs its local dependencies and speech models. Then open
**Preferences** to configure the answering model and optional Voicebot tools.

See [INSTALL.md](INSTALL.md) for prerequisites, permissions, model setup, and a
first conversation check.

## Usage

1. Launch Zana and wait for Voicebot to show **Ready**.
2. Speak normally. With no wake word configured, Voicebot answers every turn;
   with a wake word configured, say it before the request.
3. Use the floating orb to mute, start a new conversation, or open Debug.
4. Use Preferences to change the chat model, voice, wake word, tools, and other
   Voicebot settings.

## Configuration

### Voicebot (macOS)

Zana is now a hands-free voice agent with a floating orb: local Parakeet
speech-to-text and VibeVoice speech, the chat model you set in Preferences >
Model, and tools to see your screen, type for you, run commands and use MCP
servers. Follow [INSTALL.md](INSTALL.md) to set up Voicebot; see
[voicebot/README.md](voicebot/README.md) for its runtime details.

The Voicebot orb uses Aura and LiveKit activity states: dim idle, green
listening, pulsing teal thinking, blue talking, and orange working while a tool
is executing. Talking takes precedence when speech and tool activity overlap.

## Architecture

```
Zana
├── src-tauri/          Rust backend and Tauri commands
│   ├── src/commands/   Preferences, Voicebot, onboarding, update, diagnostics
│   ├── src/state.rs    Live settings
│   ├── src/voicebot.rs Voicebot supervisor and IPC
│   └── src/panel.rs    macOS floating panel
├── src-ui/             Vanilla HTML/CSS/JS windows and Voicebot orb
│   ├── preferences.html
│   ├── onboarding.html
│   ├── voicebot.html
│   └── vendor/         Bundled UI libraries
└── voicebot/           Bundled local agent and runtime
```

## Troubleshooting

### Voicebot is not ready

Open Preferences > Voicebot > Open Folder and inspect `logs/install.log` and
the process logs. Keep the app open while the local runtime and speech models
finish installing.

### Voicebot cannot hear you

Allow Zana microphone access in System Settings > Privacy & Security >
Microphone, then restart Voicebot from the app.

### Paste or key tools do not work

Grant Accessibility access if you want Voicebot to type or press keys. macOS
may also ask to let Zana control System Events.

### Screen tools do not work

Grant Screen Recording access if you ask Voicebot to capture a screenshot. A
front-window title lookup does not require screen capture permission.

### Test AI fails

Check the API URL, model ID, credentials, and provider access in Preferences >
Model. Use **Test AI** after saving the endpoint details.

## Development

### Running in Development

```bash
# Run with info logs
cargo run -p Zana-app

# Run with debug logs (shows all events)
RUST_LOG=debug cargo run -p Zana-app

# Run in tmux for background operation
tmux new-session -d -s Zana "cargo run -p Zana-app 2>&1 | tee /tmp/Zana-run.log"
tmux attach -t Zana
```

### Running Tests

```bash
# Run all tests
cargo test

# Run the Preferences and retired-stack contract tests
cargo test --test app_contracts

# Run the Voicebot orb behavior tests
node --test src-tauri/tests/voicebot_orb.test.cjs

# Run with output
cargo test -- --nocapture
```

### Building Release

```bash
# Build optimized binary
cargo build -p Zana-app --release

# Build, sign, notarize, and verify an Apple silicon macOS DMG.
# Release DMGs must use the same Developer ID identity so macOS Accessibility
# permissions stay attached across installs.
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
export APPLE_ID="you@example.com"
export APPLE_APP_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="TEAMID"
./scripts/release-macos.sh

```

To ship, open a version-bump PR that updates `Cargo.toml`, `Cargo.lock`,
`src-tauri/tauri.conf.json`, and `CHANGELOG.md`. **Merging the PR approves
publication.** After CI passes on `main`, the Release workflow builds, signs,
notarizes, and verifies an Apple silicon DMG and its checksum. Only then does
it create the annotated tag and publish the GitHub release. An already
published version is skipped; existing releases are never overwritten.

To test packaging without signing credentials, tagging, or publishing, run:

```bash
gh workflow run release.yml --ref main -f preview=true
```

Download the ad-hoc sealed DMG from the preview run's Actions artifacts.
For recovery only, if the release failed after creating the tag, rerun the
workflow for that exact existing tag from `main`:

```bash
gh workflow run release.yml --ref main -f tag=v0.1.7
```

Normal releases always use Developer ID signing and notarization. A manual
tagged run with `-f signing=unsigned` publishes an ad-hoc sealed build
instead; use `preview=true` for non-publishing tests.

Repository secrets for `developer-id`:
`APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_SIGNING_IDENTITY`, `APPLE_TEAM_ID`, plus one notarization login:
`APPLE_API_KEY_ID`, `APPLE_API_ISSUER`, `APPLE_API_PRIVATE_KEY` (App Store
Connect API key, preferred) or `APPLE_ID` and `APPLE_APP_PASSWORD`.

## Project Status

**Current Version:** 0.1.6 (Alpha)

See [CHANGELOG.md](CHANGELOG.md) for release notes.

**Completed:**
- [x] Local Parakeet speech recognition and VibeVoice playback
- [x] Voicebot conversation, wake word, and floating orb
- [x] Optional local transcript history for Voicebot
- [x] Screen, paste, keyboard, terminal, and MCP tools
- [x] Preferences and first-run Voicebot onboarding
- [x] Single-instance protection

**Roadmap:**
- [ ] Keyboard shortcuts customization
- [ ] Multi-language support
- [x] Signed macOS app bundle
- [ ] Auto-update
- [x] System tray icon
- [x] App menus
- [x] About window

## Contributing

Contributions welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Key Areas

- **Voicebot runtime**: Local speech, playback, wake word, and conversation
- **Tools**: Screen, paste, keyboard, terminal, and MCP integrations
- **UI/UX**: Preferences, onboarding, orb states, and Debug
- **Testing**: Add focused contract and runtime coverage

## License

MIT License - see [LICENSE](LICENSE) for details

## Acknowledgments

- **Tauri** - Lightweight desktop app framework
- **LiveKit** - Local room and media transport
- **Parakeet and VibeVoice** - Local speech recognition and playback

## Support

- **Issues**: GitHub Issues
- **Discussions**: GitHub Discussions
- **Support**: use GitHub Issues after the repository is published

---

Made by the Zana team
