# kVoice

**Voice-to-text transcription powered by local Whisper AI**

kVoice is a macOS menubar app that provides instant voice transcription using the Fn key. Press Fn to record, release to transcribe and paste - all processed locally with no cloud dependency.

![kVoice Demo](docs/demo.gif)

## Features

- **Fn Key Control**: Press and hold Fn to record, release to transcribe
- **Double-Tap Toggle**: Double-tap Fn to start recording, tap once to stop (hands-free mode)
- **Floating Orb**: Beautiful animated orb shows recording status
- **Works Everywhere**: Captures audio in any app, pastes transcription at cursor
- **Local Processing**: Uses Whisper.cpp - your audio never leaves your Mac
- **Plugin System**: Extend with custom orb styles and audio processors
- **Fullscreen Support**: Orb appears over fullscreen apps

## System Requirements

- **macOS**: 26.3+ (macOS Sequoia or newer)
- **Processor**: Apple Silicon (M1/M2/M3) or Intel
- **RAM**: 8GB minimum (16GB recommended for larger Whisper models)
- **Storage**: ~2GB for app + Whisper models

## Installation

### Option 1: Download Pre-built Binary (Coming Soon)

1. Download `kVoice.dmg` from [Releases](https://github.com/kvoice/kvoice/releases)
2. Open the DMG and drag kVoice to Applications
3. Launch kVoice from Applications folder
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
git clone https://github.com/kvoice/kvoice.git
cd kVoice

# Build the app (debug mode - faster compile)
cargo build -p kvoice-app

# Or build optimized release
cargo build -p kvoice-app --release

# Run the app
cargo run -p kvoice-app
```

The compiled binary will be at:
- Debug: `target/debug/kvoice-app`
- Release: `target/release/kvoice-app`

## First Run Setup

### 1. Grant Accessibility Permissions

kVoice needs accessibility access to monitor the Fn key globally.

**On first launch, you'll see:**
```
Accessibility permissions not granted - Fn key monitoring disabled
Grant access in System Settings > Privacy & Security > Accessibility
```

**To grant access:**
1. Open **System Settings** > **Privacy & Security** > **Accessibility**
2. Click the **lock icon** and authenticate
3. Find **kvoice-app** in the list and toggle it **ON**
4. Restart kVoice

### 2. Download Whisper Model

On first transcription, kVoice will automatically download the Whisper model:
- **Small model** (~500MB) - Default, best balance of speed and accuracy
- **Tiny model** (~75MB) - Faster, less accurate
- **Base/Medium/Large** - Available via settings (coming soon)

Models are cached in: `~/Library/Application Support/kvoice/models/`

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
kVoice
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
2. Restart kVoice after granting accessibility permissions
3. Check logs: `tail -f /tmp/kvoice-run.log`

### Orb disappears immediately after double-tap

**Expected behavior.** macOS may be intercepting the Fn key for dictation. Try:
- System Settings > Keyboard > Dictation
- Change shortcut from "Press Fn Key Twice" to something else

### Audio not recording

1. Check microphone permissions:
   - System Settings > Privacy & Security > Microphone
   - Enable for kvoice-app
2. Check audio device in logs

### Transcription is inaccurate

- Default model is "Small" (~94% accuracy)
- For better accuracy, use Medium or Large model (coming soon)
- Ensure good audio quality (quiet environment, close to mic)

### App crashes on hide

**Fixed in v0.1.0** - Panel now moves offscreen instead of hiding to prevent WebKit throttling crash.

## Development

### Running in Development

```bash
# Run with info logs
cargo run -p kvoice-app

# Run with debug logs (shows all events)
RUST_LOG=debug cargo run -p kvoice-app

# Run in tmux for background operation
tmux new-session -d -s kvoice "cargo run -p kvoice-app 2>&1 | tee /tmp/kvoice-run.log"
tmux attach -t kvoice
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
cargo build -p kvoice-app --release

# Create macOS app bundle
cargo tauri build

# Build universal binary (Intel + Apple Silicon)
./scripts/build-macos.sh --universal

# Sign and notarize for distribution
./scripts/sign-and-notarize.sh
```

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
- [ ] Model selection UI
- [ ] Settings panel
- [ ] Keyboard shortcuts customization
- [ ] Multi-language support
- [ ] GPU acceleration for Whisper
- [ ] Cloud backup (optional)
- [x] Signed macOS app bundle
- [ ] Auto-update
- [x] System tray icon
- [x] App menus
- [x] About window

## Contributing

Contributions welcome! Please see [CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.

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

- **Issues**: [GitHub Issues](https://github.com/kvoice/kvoice/issues)
- **Discussions**: [GitHub Discussions](https://github.com/kvoice/kvoice/discussions)
- **Email**: support@kvoice.dev

---

Made with ❤️ by the kVoice team
