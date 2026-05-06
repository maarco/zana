# qVoice Installation Guide

Quick guide to get qVoice running on your Mac.

## Prerequisites

- macOS 26.3+ (Sequoia or newer)
- 8GB RAM minimum (16GB recommended)
- ~2GB free disk space

## Installation Steps

### 1. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the prompts, then restart your terminal or run:
```bash
source $HOME/.cargo/env
```

Verify installation:
```bash
rustc --version
# Should show: rustc 1.80.0 or newer
```

### 2. Install Xcode Command Line Tools

```bash
xcode-select --install
```

Click "Install" in the popup dialog.

### 3. Clone qVoice

```bash
git clone <repo-url>
cd qVoice
```

### 4. Build qVoice

**Quick build (debug mode, faster compile):**
```bash
cargo build -p Zana-app
```

**Optimized build (release mode, slower compile but faster runtime):**
```bash
cargo build -p Zana-app --release
```

Build time: 5-15 minutes depending on your Mac.

### 5. Run qVoice

**Debug mode:**
```bash
cargo run -p Zana-app
```

**Release mode:**
```bash
cargo run -p Zana-app --release
```

### 6. Grant Accessibility Permissions

When you first run qVoice, you'll see this warning in the terminal:
```
Accessibility permissions not granted - Fn key monitoring disabled
Grant access in System Settings > Privacy & Security > Accessibility
```

**To grant access:**

1. Open **System Settings** (⚙️ in Dock or Apple menu > System Settings)
2. Go to **Privacy & Security** > **Accessibility**
3. Click the **lock icon** 🔒 at bottom left and enter your password
4. Find **Zana-app** in the list
5. Toggle the switch **ON** ✅
6. **Restart qVoice**

### 7. First Transcription

1. Click in any text field (Notes, Messages, etc.)
2. **Press and hold Fn key** → Orb appears
3. **Speak** → "Testing one two three"
4. **Release Fn key** → Text appears at cursor

First transcription will take longer (downloads Whisper model ~500MB).

## Troubleshooting

### Build fails with "linker not found"

**Fix:** Install Xcode Command Line Tools
```bash
xcode-select --install
```

### Build fails with "rustc not found"

**Fix:** Restart terminal after installing Rust, or run:
```bash
source $HOME/.cargo/env
```

### Fn key doesn't work

1. Check accessibility permissions (step 6 above)
2. Restart qVoice
3. Check if Fn is used for system functions:
   - System Settings > Keyboard > Dictation
   - Change "Press Fn Key Twice" to a different shortcut

### "failed to download Whisper model"

**Fix:** Check internet connection, model downloads from Hugging Face:
```
https://huggingface.co/ggerganov/whisper.cpp
```

Models are cached in:
```
~/Library/Application Support/Zana/models/
```

### Audio not recording

1. Grant microphone permissions:
   - System Settings > Privacy & Security > Microphone
   - Enable for Zana-app
2. Check default microphone in:
   - System Settings > Sound > Input

## Running in Background

To run qVoice in the background (keeps running after closing terminal):

```bash
# Install tmux if not installed
brew install tmux

# Run in tmux session
tmux new-session -d -s qVoice "cargo run -p Zana-app 2>&1 | tee /tmp/qVoice-run.log"

# View logs
tail -f /tmp/qVoice-run.log

# Attach to session
tmux attach -t qVoice

# Detach: Press Ctrl+B, then D

# Stop qVoice
tmux kill-session -t qVoice
```

## Updating qVoice

```bash
cd qVoice
git pull
cargo build -p Zana-app --release
```

## Uninstalling

```bash
# Remove qVoice checkout
rm -rf ~/path/to/qVoice

# Remove cached models (optional)
rm -rf ~/Library/Application\ Support/Zana

# Revoke accessibility permissions
# System Settings > Privacy & Security > Accessibility > Remove Zana-app
```

## Next Steps

- Read [README.md](README.md) for full documentation
- Try double-tap mode: Tap Fn twice quickly, speak, tap once to stop
- Customize orb: Edit `src-ui/orb_config.json`
- Explore plugins: Check `plugins/` directory

## Get Help

- **Issues**: GitHub Issues
- **Discussions**: GitHub Discussions
- **Email**: support@qvoice.app
