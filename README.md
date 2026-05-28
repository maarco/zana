# Zana

**Voice-to-text transcription powered by local Whisper AI**

Zana is a macOS menubar app that provides instant voice transcription using the Fn key. Press Fn to record, release to transcribe and paste - all processed locally by default.

## Features

- **Fn Key Control**: Press and hold Fn to record, release to transcribe
- **Double-Tap Toggle**: Double-tap Fn to start recording, tap once to stop (hands-free mode)
- **Floating Orb**: Beautiful animated orb shows recording status
- **Works Everywhere**: Captures audio in any app, pastes transcription at cursor
- **Local Processing**: Uses Whisper.cpp - your audio never leaves your Mac
- **Plugin System**: Extend with custom orb styles and audio processors
- **Fullscreen Support**: Orb appears over fullscreen apps

## System Requirements

- **macOS**: 13 Ventura or newer recommended
- **Processor**: Apple Silicon (M1/M2/M3) or Intel
- **RAM**: 8GB minimum (16GB recommended for larger Whisper models)
- **Storage**: ~2GB for app + Whisper models

## Installation

### Option 1: Download a Release DMG

1. Download the Zana DMG from GitHub Releases once releases are published
2. Open the DMG and drag Zana to Applications
3. Launch Zana from Applications folder
4. Grant accessibility permissions (required for Fn key monitoring)

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

### 1. Grant Accessibility Permissions

Zana needs accessibility access to monitor the Fn key globally.

**On first launch, you'll see:**
```
Accessibility permissions not granted - Fn key monitoring disabled
Grant access in System Settings > Privacy & Security > Accessibility
```

**To grant access:**
1. Open **System Settings** > **Privacy & Security** > **Accessibility**
2. Click the **lock icon** and authenticate
3. Find **Zana-app** in the list and toggle it **ON**
4. Restart Zana

### 2. Download Whisper Model

On first transcription, Zana will automatically download the Whisper model:
- **Small model** (~500MB) - Default, best balance of speed and accuracy
- **Tiny model** (~75MB) - Faster, less accurate
- **Base/Medium/Large** - Available via settings when downloaded

Models are cached in: `~/Library/Application Support/Zana/models/`

## Usage

### Basic Recording (Hold Mode)

1. **Press and hold Fn key** → Orb appears, recording starts
2. **Speak your message** → Orb pulses with audio level
3. **Release Fn key** → Recording stops, transcription begins
4. **Text auto-pastes** → Transcription appears at cursor

### Hands-Free Recording (Double-Tap Mode)

1. **Double-tap Fn key** (two quick taps within 300ms) → Recording starts and stays on
2. **Speak your message** → Orb stays visible while recording
3. **Tap Fn once** → Recording stops, transcription begins
4. **Text auto-pastes** → Transcription appears at cursor

### Minimum Hold Duration

Quick taps (<300ms) are ignored to prevent accidental triggers. This is helpful since macOS uses Fn for system functions (emoji picker, dictation, etc.).

## Configuration

### Orb Appearance

Edit `src-ui/orb_config.json` to customize the orb:

```json
{
  "style": "nebula-aura",
  "scale": 1.0,
  "colors": {
    "primary": "#00ffff",
    "accent": "#ff00ff"
  }
}
```

See [ORB_CONFIG.md](src-ui/ORB_CONFIG.md) for full options.

### Plugin System

Place plugins in `plugins/` directory:

```
plugins/
├── nebula-aura/
│   └── plugin.toml
└── my-custom-orb/
    └── plugin.toml
```

Plugins are automatically loaded on startup. See [docs/HOOK_HANDLER_GUIDE.md](docs/HOOK_HANDLER_GUIDE.md) for plugin development.

## Architecture

```
Zana
├── src-tauri/          Rust backend (Tauri app)
│   ├── src/
│   │   ├── audio/      Audio capture via cpal
│   │   ├── stt/        Whisper transcription
│   │   ├── hooks/      Event system
│   │   └── plugins/    Plugin loader
│   └── Cargo.toml
├── src-ui/             Frontend (vanilla HTML/CSS/JS)
│   ├── orb.html        Floating orb interface
│   └── orb_config.json Orb configuration
└── plugins/            Orb style plugins
```

## Troubleshooting

### "Accessibility permissions not granted"

**Fix:** Grant access in System Settings > Privacy & Security > Accessibility

### Fn key not working

1. Check if Fn is mapped to system functions:
   - System Settings > Keyboard > Keyboard Shortcuts > Function Keys
2. Restart Zana after granting accessibility permissions
3. Check logs: `tail -f /tmp/Zana-run.log`

### Orb disappears immediately after double-tap

**Expected behavior.** macOS may be intercepting the Fn key for dictation. Try:
- System Settings > Keyboard > Dictation
- Change shortcut from "Press Fn Key Twice" to something else

### Audio not recording

1. Check microphone permissions:
   - System Settings > Privacy & Security > Microphone
   - Enable for Zana-app
2. Check audio device in logs

### Transcription is inaccurate

- Default model is "Small" (~94% accuracy)
- For better accuracy, use Medium or Large model from settings
- Ensure good audio quality (quiet environment, close to mic)

### App crashes on hide

**Fixed in v0.1.0** - Panel now moves offscreen instead of hiding to prevent WebKit throttling crash.

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

# Run specific test
cargo test test_plugin_loading

# Run with output
cargo test -- --nocapture
```

### Building Release

```bash
# Build optimized binary
cargo build -p Zana-app --release

# Build, sign, notarize, and verify a universal macOS DMG.
# Release DMGs must use the same Developer ID identity so macOS Accessibility
# permissions stay attached across installs.
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
export APPLE_ID="you@example.com"
export APPLE_APP_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="TEAMID"
./scripts/release-macos.sh

# Cut a GitHub release from a clean tree.
./scripts/cut-release.sh v0.1.1
```

GitHub releases are created from `v*` tags by `.github/workflows/release.yml`.
Set these repository secrets before publishing signed builds:
`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`,
`APPLE_ID`, `APPLE_APP_PASSWORD`, and `APPLE_TEAM_ID`.

## Project Status

**Current Version:** 0.1.0 (Alpha)

**Completed:**
- [x] Local Whisper transcription
- [x] Fn key monitoring (hold to record)
- [x] Double-tap toggle mode
- [x] Floating orb with animations
- [x] Plugin system (orb styles)
- [x] Hook event system
- [x] Auto-paste transcription
- [x] Fullscreen support

**Roadmap:**
- [x] Model selection UI
- [x] Settings panel
- [ ] Keyboard shortcuts customization
- [ ] Multi-language support
- [ ] GPU acceleration for Whisper
- [x] Signed macOS app bundle
- [ ] Auto-update
- [x] System tray icon
- [x] App menus
- [x] About window

## Contributing

Contributions welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Key Areas

- **Audio processing**: Improve noise reduction, audio preprocessing
- **Whisper optimization**: Faster transcription, GPU support
- **Plugins**: Create new orb styles, audio processors, integrations
- **UI/UX**: Improve orb animations, add settings panel
- **Testing**: Add more unit/integration tests

## License

MIT License - see [LICENSE](LICENSE) for details

## Acknowledgments

- **Whisper.cpp** - Fast local transcription by Georgi Gerganov
- **Tauri** - Lightweight desktop app framework
- **cpal** - Cross-platform audio library

## Support

- **Issues**: GitHub Issues
- **Discussions**: GitHub Discussions
- **Support**: use GitHub Issues after the repository is published

---

Made by the Zana team
