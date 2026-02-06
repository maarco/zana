# kVoice User Guide

Welcome to kVoice - a beautiful, extensible speech-to-text application with GPU-accelerated audio visualizations.

---

## Table of Contents

1. [Getting Started](#getting-started)
2. [Quick Start Tutorial](#quick-start-tutorial)
3. [Recording Audio](#recording-audio)
4. [Transcription](#transcription)
5. [Visualizations](#visualizations)
6. [Settings](#settings)
7. [Plugins](#plugins)
8. [Troubleshooting](#troubleshooting)
9. [Tips and Tricks](#tips-and-tricks)

---

## Getting Started

### System Requirements

**Minimum:**
- OS: macOS 11+, Ubuntu 20.04+, Windows 10+
- RAM: 4GB
- Storage: 2GB free
- Microphone

**Recommended:**
- OS: macOS 13+, Ubuntu 22.04+, Windows 11+
- RAM: 8GB+
- Storage: 5GB+ free
- GPU with OpenGL 3.3+ or Vulkan 1.1+

### Installation

#### Option 1: Build from Source (Recommended for Developers)

**Prerequisites:**
- Rust 1.70 or later
- Git
- Platform-specific dependencies (see below)

**Install Rust:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**Platform Dependencies:**

macOS:
```bash
# Install Xcode Command Line Tools
xcode-select --install
```

Ubuntu/Debian:
```bash
sudo apt update
sudo apt install -y \
    pkg-config \
    libx11-dev \
    libasound2-dev \
    libudev-dev
```

Fedora:
```bash
sudo dnf install \
    alsa-lib-devel \
    libX11-devel \
    libudev-devel
```

Windows:
```bash
# Install Visual Studio Community (includes C++ build tools)
# Download from: https://visualstudio.microsoft.com/downloads/

# Or install build-tools via winget
winget install Microsoft.VisualStudio.2022.BuildTools
```

**Clone and Build:**
```bash
# Clone repository
git clone https://github.com/kvoice/kvoice.git
cd kvoice

# Build release version
cargo build --release

# Run application
./target/release/kvoice
```

#### Option 2: Install from Crate (Linux/macOS)

```bash
cargo install kvoice
```

This installs kVoice in `~/.cargo/bin/`. Make sure this directory is in your PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Add this to your `~/.bashrc` or `~/.zshrc` to make it permanent.

#### Option 3: Download Binary (Windows)

1. Visit [kvoice.app](https://kvoice.app)
2. Download the Windows installer
3. Run the installer
4. Launch kVoice from Start Menu

### First Launch Setup

#### Step 1: Grant Microphone Permissions

**macOS:**
1. Launch kVoice
2. System will prompt: "kvoice" would like to access the microphone
3. Click "OK"
4. If prompted, open System Preferences > Privacy & Security > Microphone
5. Ensure kVoice is checked

**Linux:**
1. Launch kVoice from terminal
2. If microphone doesn't work, check PulseAudio settings:
   ```bash
   pactl load-module module-pipe-source
   pavucontrol  # Open sound settings
   ```
3. In pavucontrol, go to "Recording" tab and select your microphone

**Windows:**
1. Launch kVoice
2. Windows Security dialog will appear
3. Click "Yes" to allow microphone access
4. Or manually: Settings > Privacy > Microphone > Allow desktop apps to access microphone

#### Step 2: Download Whisper Model

On first launch, kVoice will prompt you to download a speech recognition model.

**Recommended Model: Small (244MB)**
- Best balance of speed and accuracy
- Good for most use cases

**To Download:**
1. Click "Settings" (gear icon)
2. Under "Whisper Model", select "Small"
3. Click "Download Model" button
4. Wait for download to complete (progress bar shown)
5. Model is cached locally for future use

**Model Options:**

| Model | Size | Speed | Accuracy | Download Time* | Best For |
|-------|------|-------|----------|----------------|----------|
| Tiny | 39MB | Fastest | Good | ~30 seconds | Quick notes, real-time |
| Base | 74MB | Fast | Better | ~1 minute | Dictation |
| Small | 244MB | Medium | Great | ~3 minutes | General use (recommended) |
| Medium | 769MB | Slow | Excellent | ~10 minutes | Professional |
| Large v3 | 1.5GB | Slowest | Best | ~20 minutes | High accuracy needs |

*Download times vary by internet connection

#### Step 3: Select Audio Device

1. Click "Settings" (gear icon)
2. Under "Audio Device", click the dropdown
3. Select your microphone from the list
4. Speak into your microphone - the level indicator should move
5. If no movement, try a different device or check system settings

#### Step 4: Test Your Setup

1. Click "Record" button (microphone icon)
2. Speak a few sentences
3. The orb should pulse with your voice
4. Click "Stop"
5. Click "Transcribe"
6. Your transcription should appear in the panel

---

## Quick Start Tutorial

### Your First Transcription (5 Minutes)

**1. Launch kVoice**
Double-click the kVoice icon or run `kvoice` from terminal

**2. Check Audio Level**
- Look at the level indicator at the bottom
- Speak - it should show activity
- If not, go to Settings > Audio Device and select your microphone

**3. Record**
- Click the red "Record" button
- Speak clearly: "Hello, this is a test of kVoice speech recognition"
- The orb will pulse with your voice
- Click "Stop" when finished

**4. Transcribe**
- Click the "Transcribe" button
- Wait for processing (first time may take longer)
- Text appears in the transcription panel

**5. Export or Copy**
- Select the text in the transcription panel
- Copy with Cmd/Ctrl+C
- Paste into any application

### Tips for Best Results

**Microphone Placement:**
- Desktop mic: 6-12 inches away
- Headset mic: Position near mouth corner
- Built-in laptop mic: Speak directly into it

**Environment:**
- Quiet room works best
- Close windows to reduce street noise
- Turn off fans or HVAC if possible

**Speaking Style:**
- Natural conversational tone
- Moderate speaking pace
- Clear pronunciation but don't over-enunciate
- Brief pauses are fine

---

## Recording Audio

### Basic Recording

**To Record:**
1. Click the **Record** button (or press Space)
2. Recording indicator appears (red dot)
3. Speak into your microphone
4. Click **Stop** when finished (or press Space again)

**Visual Feedback:**
- **Orb pulses**: Audio level is being detected
- **Level indicator**: Shows input strength at bottom
- **Recording indicator**: Red dot appears when recording

### Audio Device Selection

**To Change Microphone:**
1. Open Settings (gear icon or Cmd/Ctrl+,)
2. Find "Audio Device" section
3. Click the dropdown menu
4. Select your preferred microphone
5. Test by speaking - level indicator should move

**Available Devices:**
- Built-in microphone
- USB microphone
- Bluetooth headset
- Audio interface input

**Audio Level Indicator:**

```
Level: [==========     ] 65%
        ^Good zone     ^
```

- **0-40%**: Too quiet - move closer or increase gain
- **40-80%**: Perfect - good for transcription
- **80-100%**: Too loud - may distort, move back

### Recording Tips

**For Dictation:**
- Speak in complete sentences
- Pause briefly between thoughts
- Punctuation is added automatically
- Say "new paragraph" to start new section

**For Meetings:**
- Place microphone near speaker
- Use a dedicated microphone if possible
- Consider Medium or Large model for accuracy
- Record in 5-10 minute segments

**For Interviews:**
- Position microphone between speakers
- Ensure equal distance from both people
- Test levels before starting
- Use Medium model for best accuracy

---

## Transcription

### Automatic vs Manual

**Automatic (Default):**
- Transcription starts immediately after recording stops
- Toggle in Settings: "Auto-transcribe after recording"
- Best for: Quick dictation, voice notes

**Manual:**
1. Record audio
2. Click "Transcribe" button when ready
3. Can transcribe same audio multiple times with different models
- Best for: Batch processing, trying different models

### Model Selection

**Choosing the Right Model:**

| Use Case | Recommended Model | Why |
|----------|------------------|-----|
| Voice notes | Tiny or Base | Fast, good enough for personal use |
| Meetings/lectures | Small | Balanced speed and accuracy |
| Professional work | Medium | Higher accuracy for important content |
| Research/legal | Large v3 | Best accuracy, slower |
| Real-time captioning | Tiny | Fastest processing |

**To Change Model:**
1. Open Settings
2. Under "Whisper Model", select desired model
3. Download if not already present (button appears)
4. Model is used for future transcriptions

### Transcription Panel

**What You See:**
- Full transcription text
- Processing time (e.g., "Processed in 3.2 seconds")
- Word count

**Using Transcription:**
- Select text with mouse
- Copy with Cmd/Ctrl+C
- Paste into documents, emails, etc.
- Panel stays open for multiple transcriptions

**Transcription Speed:**

| Model | Realtime Factor | Example: 5 min audio |
|-------|----------------|---------------------|
| Tiny | ~32x | ~10 seconds |
| Base | ~16x | ~20 seconds |
| Small | ~6x | ~50 seconds |
| Medium | ~2x | ~2.5 minutes |
| Large v3 | ~1x | ~5 minutes |

### Language Selection

**Auto-Detect (Default):**
- Works best for English
- Can detect other languages
- May be less accurate for non-English

**Specific Language:**
1. Open Settings
2. Under "Language", select your language
3. Improves accuracy for that language

**Supported Languages:**
- English, Spanish, French, German, Italian
- Chinese, Japanese, Korean
- And 90+ more languages

---

## Visualizations

### The Orb

The central visualization responds to your voice in real-time:

**What It Shows:**
- **Pulsing**: Audio volume/level
- **Colors**: Frequency content (bass, mid, treble)
- **Particles**: FFT frequency data around the orb
- **Glow**: Peak audio levels with slow decay

**Color Schemes:**

| Style | Colors | Description |
|-------|--------|-------------|
| Nebula Aura (default) | Purple, violet | Cosmic nebula effect |
| Cyan | Cyan, blue, teal | Cool ocean tones |
| Fire | Red, orange, yellow | Flame-like |
| Aurora | Green, blue, purple | Northern lights |
| Cosmic | Deep space colors | Starfield effect |

**To Change Style:**
1. Open Settings
2. Under "Orb Style", select desired style
3. Change is instant

### Customizing Visualization

**Quality Settings:**
- **Low**: Best performance on older hardware
- **Medium**: Balanced (default)
- **High**: Best visuals, may need GPU

**Transparency:**
- **On**: Window background is transparent (default)
- **Off**: Solid background (use if transparency issues)

**Animation Speed:**
- Controls how fast animations cycle
- Range: 0.5x (slow) to 2.0x (fast)
- Default: 1.0x (normal)

### Performance Tips

**If animation is choppy:**
- Reduce quality to Low
- Try a simpler orb style
- Close other applications
- Disable transparency

**To check GPU acceleration:**
- Check logs for "wgpu adapter" message
- Should show your GPU model
- If showing "fallback adapter", GPU isn't being used

---

## Settings

### Audio Settings

**Audio Device:**
- Select input microphone
- Auto-detects available devices
- Shows device name and capabilities

**Sample Rate:**
- Default: 16000 Hz (recommended for Whisper)
- Higher rates may improve quality slightly
- Lower rates save disk space

### Transcription Settings

**Whisper Model:**
- Choose model size (see Model Selection section)
- Download button if not present
- Shows current model size on disk

**Language:**
- Auto-detect (default)
- Or select specific language
- Improves accuracy for non-English

**Auto-transcribe:**
- Enabled by default
- Automatically transcribe after recording
- Disable for manual control

### Visualization Settings

**Orb Style:**
- Select visualization plugin
- See preview thumbnails
- Built-in styles: Nebula Aura GPU

**Quality:**
- Low: Best performance
- Medium: Balanced (default)
- High: Best visuals

**Transparency:**
- Enable for floating orb effect
- Disable on systems with transparency issues

### Advanced Settings

**Debug Mode:**
- Enables detailed logging
- Useful for troubleshooting
- Logs saved to `~/.kvoice/logs/`

**Always on Top:**
- Keep window above other applications
- Useful while referencing other windows

**Window Size:**
- Default: 500x500 pixels
- Adjustable by dragging window edge

### Resetting Settings

If something isn't working, reset to defaults:

**macOS/Linux:**
```bash
rm -rf ~/.kvoice
kvoice
```

**Windows:**
```
1. Close kVoice
2. Delete: %APPDATA%\kvoice
3. Restart kVoice
```

---

## Plugins

### Installing Plugins

**From Built-in Selection:**
1. Open Settings > Plugins tab
2. Browse available plugins
3. Click "Enable" on desired plugin
4. Plugin activates immediately

**From Marketplace:**
1. Open Settings > Plugins > Marketplace
2. Browse available plugins
3. Click "Install" on desired plugin
4. Wait for download and installation
5. Plugin appears in your plugin list

**Manual Installation:**
1. Download `.kvoice` plugin file
2. Settings > Plugins > Install from File
3. Select downloaded file
4. Plugin installs and activates

### Plugin Types

**Orb Styles:**
- Audio visualizations
- Replace the central orb display
- Examples: Frequency Bars, Waveform, Particles

**Audio Processors:**
- Modify audio before transcription
- Examples: Noise reduction, EQ, gain boost

**Post-Processors:**
- Modify transcription output
- Examples: Auto-punctuation, capitalization, formatting

**Integrations:**
- Connect to external services
- Examples: Save to cloud, send to API, notifications

### Managing Plugins

**Enable/Disable:**
1. Settings > Plugins
2. Find plugin in list
3. Toggle switch on/off

**Uninstall:**
1. Settings > Plugins
2. Click "Remove" on plugin
3. Confirm removal

**Check for Updates:**
1. Settings > Plugins
2. Click "Check Updates"
3. Update available plugins

### Creating Plugins

For documentation on creating plugins, see:
- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md)
- [API Reference](./API.md)

---

## Troubleshooting

### Audio Issues

**Problem: No sound detected**

**Solutions:**
1. Check microphone permissions in system settings
2. Verify correct audio device selected in kVoice Settings
3. Test microphone in another application
4. Check cable connections (external mics)
5. Restart kVoice

**macOS:**
- System Preferences > Security & Privacy > Privacy > Microphone
- Ensure kVoice is checked
- Uncheck and recheck if needed

**Linux:**
```bash
# Test microphone
arecord -f cd -d 5 test.wav
aplay test.wav

# Check PulseAudio
pavucontrol &
```

**Windows:**
- Settings > Privacy > Microphone
- Enable "Allow apps to access your microphone"
- Enable "Allow desktop apps to access your microphone"

**Problem: Low audio level**

**Solutions:**
1. Increase microphone gain in system settings
2. Move closer to microphone
3. Check for obstructions (pop filter, foam)
4. Use different microphone

**Problem: Distortion/clipping**

**Solutions:**
1. Reduce microphone gain
2. Move further from microphone
3. Disable automatic gain control if enabled
4. Aim for 60-80% on level indicator

### Transcription Issues

**Problem: Poor accuracy**

**Solutions:**
1. Try a larger Whisper model
2. Ensure clear audio quality
3. Minimize background noise
4. Select specific language instead of auto-detect
5. Speak clearly at moderate pace
6. Use external microphone

**Problem: Slow transcription**

**Solutions:**
1. Use a smaller model (Tiny or Base)
2. Close other applications
3. Check if GPU acceleration is available
4. For Large model, ensure adequate RAM (16GB+)

**Problem: Model download fails**

**Solutions:**
1. Check internet connection
2. Verify disk space (need up to 1.5GB for Large)
3. Try downloading again
4. Check firewall settings
5. Download manually from Hugging Face

**Manual Model Download:**
1. Visit [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp)
2. Download model files (ggml-*.bin)
3. Place in `~/.kvoice/models/` (or `%APPDATA%\kvoice\models\` on Windows)

### Visualization Issues

**Problem: Orb not animating**

**Solutions:**
1. Check if audio is being detected (level indicator)
2. Try a different orb style
3. Reduce quality setting
4. Update GPU drivers
5. Restart kVoice

**Problem: Poor performance**

**Solutions:**
1. Reduce visualization quality to Low
2. Try a simpler orb style
3. Close other applications
4. Disable transparency
5. Check if GPU acceleration is working

**Problem: Transparency issues (Linux)**

**Solutions:**
1. Enable compositing in desktop environment
2. Try disabling transparency in kVoice settings
3. Check window manager compatibility
4. Use different window manager (GNOME, KDE)

**Checking GPU Status:**
```bash
# View logs
cat ~/.kvoice/logs/kvoice.log | grep -i gpu

# Should see something like:
# "Using wgpu adapter: Your GPU Model"
```

### Application Issues

**Problem: Crash on startup**

**Solutions:**
1. Check system logs for error messages
2. Verify all dependencies are installed
3. Try running from terminal to see error output:
   ```bash
   kvoice --debug
   ```
4. Delete configuration and restart:
   ```bash
   rm -rf ~/.kvoice
   kvoice
   ```

**Problem: Settings not saving**

**Solutions:**
1. Check write permissions for `~/.kvoice`
2. Ensure disk is not full
3. Restart application
4. Check logs for errors

**Problem: Can't quit application**

**Solutions:**
1. Press Cmd/Ctrl+Q
2. Right-click dock icon and select Quit
3. Use Activity Monitor/Task Manager to force quit
4. Stop any ongoing recordings first

### Platform-Specific Issues

**macOS:**

**Problem: "App can't be opened"**
- Right-click kVoice, select "Open"
- Or: System Preferences > Security > "Open Anyway"

**Problem: Poor performance**
- Disable "App Nap" for kVoice
- Use full-screen mode for best experience
- Close heavy applications (browsers, editors)

**Linux:**

**Problem: No audio devices found**
```bash
# Install PulseAudio
sudo apt install pulseaudio pulseaudio-utils

# Restart PulseAudio
pulseaudio --kill
pulseaudio --start
```

**Problem: Window won't show**
- Ensure compositing is enabled
- Try different window manager
- Run with `--no-transparent` flag

**Windows:**

**Problem: Microphone not working**
- Run as Administrator
- Disable "Game Mode" for better audio quality
- Update audio drivers from manufacturer website
- Ensure Windows is up to date

**Problem: Visual glitches**
- Update GPU drivers
- Disable hardware acceleration in settings
- Try compatibility mode

### Getting Help

**Before Requesting Help:**
1. Check this guide thoroughly
2. Search existing issues: https://github.com/kvoice/kvoice/issues
3. Gather this information:
   - kVoice version (run `kvoice --version`)
   - Operating system and version
   - Steps to reproduce
   - Error messages or logs

**Debug Logs:**
```bash
# View logs
cat ~/.kvoice/logs/kvoice.log

# Or run with debug output
RUST_LOG=debug kvoice
```

**Reporting Issues:**
- GitHub: https://github.com/kvoice/kvoice/issues
- Include error logs
- Describe what you expected vs what happened
- Include system information

**Community:**
- Discord: https://discord.gg/kvoice
- Forums: https://kvoice.app/forum
- Email: support@kvoice.app

---

## Tips and Tricks

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Space | Start/Stop recording |
| T | Transcribe |
| S | Open/Close settings |
| Cmd/Ctrl+Q | Quit |
| Cmd/Ctrl+, | Preferences |
| Escape | Close panel |

### Workflow Tips

**For Dictation:**
1. Use Tiny or Base model for speed
2. Enable auto-transcribe
3. Keep transcription panel open
4. Copy text as you go

**For Meetings:**
1. Use Small or Medium model
2. Record in 5-10 minute segments
3. Take notes during recording
4. Transcribe during breaks

**For Interviews:**
1. Use Medium or Large model
2. Place microphone centrally
3. Record entire interview
4. Transcribe afterward

**For Notes:**
1. Use Tiny model (fastest)
2. Quick recordings as you think
3. Auto-transcribe saves time
4. Copy to note-taking app

### Performance Optimization

**To Improve Speed:**
- Use smaller models (Tiny/Base)
- Reduce visualization quality
- Close unnecessary applications
- Enable GPU acceleration

**To Improve Accuracy:**
- Use larger models (Small/Medium)
- Ensure quiet environment
- Use quality microphone
- Speak clearly and consistently

**To Save Resources:**
- Disable when not in use
- Use Low quality setting
- Disable transparency
- Close transcription panel

### Integration Tips

**With Text Editors:**
1. Transcribe to panel
2. Select and copy text
3. Paste directly into editor
4. Some editors support auto-paste

**With Note Apps:**
1. Use for voice memos
2. Transcribe and copy to notes
3. Works with Notion, Obsidian, etc.

**With Messaging:**
1. Quick voice-to-text for messages
2. Transcribe and paste into chat
3. Faster than typing for long messages

### Accessibility

**For Hearing Impaired:**
- Real-time transcription of conversations
- Use in meetings, lectures, videos
- Larger text available in transcription panel

**For Voice Disorders:**
- Type-to-speech alternatives
- Record short segments if needed
- Use pause and resume frequently

**For Mobility Issues:**
- Hands-free operation
- Keyboard shortcuts available
- Voice control of computer

---

## Advanced Features

### Multiple Languages

kVoice supports 90+ languages via Whisper:

**To Change Language:**
1. Settings > Language
2. Select your language from dropdown
3. Improves accuracy significantly

**Common Languages:**
- English (default)
- Spanish (es)
- French (fr)
- German (de)
- Italian (it)
- Chinese (zh)
- Japanese (ja)
- Korean (ko)

**Full Language List:**
See: https://github.com/openai/whisper/blob/main/README.md

### Custom Configurations

Edit configuration file directly for advanced settings:

**Location:**
- macOS/Linux: `~/.kvoice/config.toml`
- Windows: `%APPDATA%\kvoice\config.toml`

**Example:**
```toml
[audio]
sample_rate = 16000
device = "default"

[transcription]
model = "small"
language = "auto"
auto_transcribe = true

[visualization]
style = "nebula-aura-gpu"
quality = "medium"
transparent = true

[window]
always_on_top = true
width = 500
height = 500
```

### Export Transcriptions

**Current Version:**
- Select text in panel
- Copy with Cmd/Ctrl+C
- Paste into any application

**Future Enhancements:**
- Export to TXT file
- Export to Markdown
- Export to JSON (with timestamps)
- Auto-save to location

---

## FAQ

**Q: Is my audio uploaded anywhere?**
A: No. All processing happens locally on your computer. Your audio never leaves your device.

**Q: Can I use kVoice offline?**
A: Yes, once the model is downloaded, kVoice works completely offline.

**Q: How much disk space do models need?**
A: Models range from 39MB (Tiny) to 1.5GB (Large v3). You can delete unused models.

**Q: Can I transcribe existing audio files?**
A: Not directly in current version. Record the audio playing on your computer as a workaround.

**Q: What audio formats are supported?**
A: kVoice works with microphone input. For file support, check for future updates.

**Q: Is transcription real-time?**
A: Near real-time with Tiny model (~3x faster than realtime). Larger models are slower but more accurate.

**Q: Can I train my own model?**
A: kVoice uses Whisper models. Custom training requires separate tools from OpenAI.

**Q: Does kVoice work with Bluetooth headphones?**
A: Yes, if your system recognizes the headphones' microphone.

**Q: Can I use multiple microphones?**
A: kVoice uses one at a time. Switch in Settings > Audio Device.

**Q: Is there a mobile version?**
A: Not currently. kVoice is desktop-only.

**Q: Can I use kVoice commercially?**
A: Yes, kVoice is released under MIT license. See LICENSE file.

---

## Changelog

See [CHANGELOG.md](https://github.com/kvoice/kvoice/blob/main/CHANGELOG.md) for version history.

---

## License

kVoice is released under the MIT License.

---

## Support

- Website: https://kvoice.app
- Documentation: https://kvoice.app/docs
- GitHub: https://github.com/kvoice/kvoice
- Discord: https://discord.gg/kvoice
- Email: support@kvoice.app

---

**Enjoy using kVoice!**
