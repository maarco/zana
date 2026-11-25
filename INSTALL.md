# Install Zana on macOS

This guide builds the current Zana app from source and sets up its Voicebot.
The old Fn-key dictation and Whisper workflow is retired.

## Requirements

- macOS 13 or newer on Apple silicon. Voicebot's local Parakeet speech
  recognition uses MLX.
- Xcode Command Line Tools, Homebrew, and a stable Rust toolchain.
- `uv` and the LiveKit server, installed below.
- A working OpenAI-compatible chat model endpoint. Hosted providers usually
  require an API key; local servers may have their own authentication settings.
  Speech recognition is local, but Voicebot needs a chat model to answer.
- An internet connection and several gigabytes of free disk space for the first
  setup. Voicebot installs about 1.5 GB of Python packages, then downloads its
  local speech models and voices.

## Install prerequisites

Install Apple's command-line build tools:

```sh
xcode-select --install
```

Install Homebrew from [brew.sh](https://brew.sh), then install the two runtime
dependencies:

```sh
brew install uv livekit
```

Install the Rust stable toolchain from [rustup.rs](https://rustup.rs):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable
```

## Build and start Zana

Clone the repository and start the Tauri app:

```sh
git clone https://github.com/maarco/zana.git
cd zana
cargo run -p Zana-app
```

On the first launch, Zana creates a Python 3.12 environment, installs Voicebot's
packages, and downloads the local Parakeet and VibeVoice models and voice assets.
This can take several minutes. Keep the app open until the Voicebot status is
Ready. Its files and setup logs are in
`~/Library/Application Support/Zana/voicebot/`.

## Where Zana stores data

On macOS, Zana's persistent, app-owned locations are:

- `~/Library/Application Support/Zana/`: settings, the local API key, the
  onboarding marker, plugins, legacy Whisper models, and the Voicebot folder.
- `~/Library/Application Support/Zana/voicebot/`: the transcript, prompt,
  MCP configuration, optional `.env`, voice print, voices, thinking sounds,
  Python environment, process logs, update state, and screenshots.
- `~/Library/Application Support/Zana/voicebot/cache/huggingface/`: the
  Hugging Face model cache used by Voicebot.
- `~/Library/Application Support/Zana/voicebot/cache/uv/`: the package cache
  used while Voicebot's environment is installed or refreshed.
- `~/Library/Application Support/app.zana/`: Tauri/WebView application data.
- `~/Library/Caches/app.zana/`: Tauri/WebView cache data.
- `~/Library/Logs/app.zana/`: Zana's Rust application logs.
- `~/.cache/huggingface/` and `~/Library/Caches/uv/`: legacy Zana cache
  locations that may be shared with other applications; current Zana does not
  write there.

Audio request files are created briefly in macOS's temporary directory and are
deleted after transcription. The current app scopes Hugging Face and uv to the
Voicebot folder so it does not claim ownership of shared caches. Older Zana
versions may have left data in `~/.cache/huggingface/` or
`~/Library/Caches/uv/`; those locations can be shared by other applications and
are deliberately not removed automatically.

Preferences > Data & Privacy > Remove All Zana Data asks for confirmation,
stops Zana, and moves the app-owned locations above to the user's Trash. The
API key and transcript remain recoverable until the Trash is emptied. Shared
legacy caches require separate, user-confirmed cleanup after checking their
other users.

## Configure an answering model

1. Open Preferences and select Model.
2. Enter an API key if your endpoint requires one, its model ID, and its
   OpenAI-compatible chat-completions API URL. The initial URL is OpenAI's
   `https://api.openai.com/v1/chat/completions`; change it if you use another
   provider or a local server. Zana stores the key in its local settings.
3. Leave Vision Model empty to reuse the chat model, or enter a vision-capable
   model ID. The chat model is also used for screenshots when no separate vision
   model is set.
4. Click Test AI to verify the connection, then save the preferences.

The default OpenAI endpoint has no API key set, so configure the credentials
required by your chosen endpoint before expecting an answer. If Test AI fails,
check the API URL, credentials, model ID, and provider access to that model.

## Allow macOS permissions as needed

- Microphone access lets Voicebot listen for speech and transcribe it locally.
  Allow it when macOS prompts; without it Voicebot cannot hear you.
- Accessibility access is used by Voicebot's type/paste and press-keys tools.
  It is not needed just to speak with Voicebot. Grant it only if you want those
  actions; manage it in System Settings > Privacy & Security > Accessibility.
- Screen Recording is needed only when Voicebot captures a screenshot after you
  ask it to look at your screen. It is not needed for voice conversations.
- macOS may also ask to let Zana control System Events for keyboard actions or
  reading the frontmost app and window title.

## Check the setup

The wake word is empty by default, which means Voicebot answers each recognized
turn. In Preferences > Voicebot, you can set a wake word; it will still save
transcripts while asleep and send the unread text to the model after it wakes.

After the status shows Ready and Test AI succeeds, say a simple prompt such as
“Tell me a short joke.” Voicebot should answer aloud. The microphone audio is
processed locally by default, but recognized text and tool results go to your
configured chat endpoint. Screen titles and screenshots are shared with the
configured model only when Voicebot uses those screen tools. See
[PRIVACY.md](PRIVACY.md) for details.

If Voicebot does not finish setup, open Preferences > Voicebot > Open Folder and
check `logs/install.log`. For permission issues, review System Settings > Privacy
& Security and make sure the correct Zana app has access.
