# kVoice First-Run Onboarding - Implementation Plan

## Vision

**Zero-friction onboarding**: User downloads app, launches it, completes 3-step wizard, starts transcribing. Total time: <2 minutes.

## User Flow

```
Download kVoice
    ↓
Launch App
    ↓
[First Run Wizard]
    ↓
Step 1: Welcome → Click "Get Started"
    ↓
Step 2: Accessibility → Click "Open Settings" → Grant Permission → Click "Continue"
    ↓
Step 3: Whisper Model → Auto-download with progress → "Ready!"
    ↓
Try It: "Press Fn to record your first transcription"
    ↓
Success! App is ready
```

---

## Implementation Plan

### Phase 1: Onboarding UI (Welcome Wizard)

**Location:** New window `onboarding.html` in `src-ui/`

**Tech Stack:**
- HTML/CSS/JS (vanilla, matches orb.html style)
- Tauri commands for backend communication
- CSS animations for transitions

**Screens:**

#### Screen 1: Welcome
```
┌─────────────────────────────────────┐
│                                     │
│         🎙️  Welcome to kVoice      │
│                                     │
│   Voice-to-text transcription      │
│   powered by local Whisper AI      │
│                                     │
│   ✓ Works offline                   │
│   ✓ Private (local processing)     │
│   ✓ Fast & accurate                │
│                                     │
│      [Get Started →]                │
│                                     │
└─────────────────────────────────────┘
```

#### Screen 2: Accessibility Permission
```
┌─────────────────────────────────────┐
│                                     │
│    🔐  Accessibility Permission     │
│                                     │
│  kVoice needs accessibility access  │
│  to monitor the Fn key globally.    │
│                                     │
│  This lets you record from any app. │
│                                     │
│  Status: [⚠️ Not Granted]           │
│                                     │
│      [Open Settings →]              │
│      [← Back]  [Skip]  [Continue →] │
│                                     │
└─────────────────────────────────────┘
```

**Auto-detection:**
- Poll `AXIsProcessTrusted()` every second
- When granted: Status changes to [✅ Granted], Continue button enables
- Show inline instructions: "Find kvoice-app in the list and toggle ON"

#### Screen 3: Download Whisper Model
```
┌─────────────────────────────────────┐
│                                     │
│      📥  Downloading Whisper AI     │
│                                     │
│  Model: ggml-small.en.bin (462 MB)  │
│                                     │
│  [████████████░░░░░░░░] 64%         │
│  238 MB / 462 MB                    │
│                                     │
│  Estimated time: 2 minutes          │
│                                     │
│  (Downloaded once, cached locally)  │
│                                     │
└─────────────────────────────────────┘
```

**Download Logic:**
- Check if model exists in cache first
- If exists: Skip download, show "Model ready ✓"
- If not: Stream download with progress updates
- Verify hash after download
- Show error with retry button on failure

#### Screen 4: Ready!
```
┌─────────────────────────────────────┐
│                                     │
│         🎉  You're All Set!         │
│                                     │
│  kVoice is ready to transcribe!     │
│                                     │
│  ━━━━━━━━━━━ How to Use ━━━━━━━━━━━ │
│                                     │
│  Press & Hold: Hold Fn to record    │
│  Double-Tap: Tap Fn twice for      │
│              hands-free mode        │
│                                     │
│      [Try Your First Recording →]   │
│                                     │
└─────────────────────────────────────┘
```

**Action:**
- Close onboarding window
- Show orb in "tutorial mode" with tooltips
- Wait for first Fn press

---

### Phase 2: Backend - First-Run Detection

**File:** `src-tauri/src/onboarding.rs` (NEW)

```rust
/// Check if this is the first run
pub fn is_first_run() -> bool {
    let config_dir = dirs::config_dir()
        .unwrap()
        .join("kvoice");

    let marker = config_dir.join(".onboarding_complete");
    !marker.exists()
}

/// Mark onboarding as complete
pub fn mark_onboarding_complete() -> Result<()> {
    let config_dir = dirs::config_dir()
        .unwrap()
        .join("kvoice");

    fs::create_dir_all(&config_dir)?;
    fs::write(config_dir.join(".onboarding_complete"), "")?;
    Ok(())
}

/// Check accessibility permissions
pub fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        unsafe {
            extern "C" {
                fn AXIsProcessTrusted() -> bool;
            }
            AXIsProcessTrusted()
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open System Settings to Accessibility pane
pub fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}
```

---

### Phase 3: Backend - Model Download

**File:** `src-tauri/src/stt/downloader.rs` (NEW)

```rust
use reqwest::Client;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

pub struct ModelDownloader {
    client: Client,
    model_dir: PathBuf,
}

impl ModelDownloader {
    pub fn new() -> Self {
        let model_dir = dirs::data_dir()
            .unwrap()
            .join("kvoice")
            .join("models");

        Self {
            client: Client::new(),
            model_dir,
        }
    }

    /// Check if model is already downloaded
    pub fn is_model_cached(&self, model_name: &str) -> bool {
        self.model_dir.join(model_name).exists()
    }

    /// Download model with progress callback
    pub async fn download_model<F>(
        &self,
        model_name: &str,
        url: &str,
        progress_callback: F,
    ) -> Result<PathBuf>
    where
        F: Fn(u64, u64) + Send + 'static,
    {
        fs::create_dir_all(&self.model_dir)?;

        let response = self.client.get(url).send().await?;
        let total_size = response.content_length().unwrap_or(0);

        let file_path = self.model_dir.join(model_name);
        let mut file = File::create(&file_path).await?;
        let mut downloaded = 0u64;

        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            progress_callback(downloaded, total_size);
        }

        file.flush().await?;
        Ok(file_path)
    }
}
```

---

### Phase 4: Tauri Commands

**File:** `src-tauri/src/commands/onboarding.rs` (NEW)

```rust
use tauri::State;
use crate::onboarding;
use crate::stt::downloader::ModelDownloader;

#[tauri::command]
pub fn is_first_run() -> bool {
    onboarding::is_first_run()
}

#[tauri::command]
pub fn check_accessibility_permission() -> bool {
    onboarding::check_accessibility()
}

#[tauri::command]
pub fn open_accessibility_settings() {
    onboarding::open_accessibility_settings()
}

#[tauri::command]
pub fn mark_onboarding_complete() -> Result<(), String> {
    onboarding::mark_onboarding_complete()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_whisper_model(
    window: tauri::Window,
) -> Result<String, String> {
    let downloader = ModelDownloader::new();

    let model_name = "ggml-small.en.bin";
    let url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin";

    // Check cache first
    if downloader.is_model_cached(model_name) {
        return Ok("Model already cached".to_string());
    }

    // Download with progress
    let path = downloader.download_model(model_name, url, move |downloaded, total| {
        let _ = window.emit("download-progress", DownloadProgress {
            downloaded,
            total,
            percent: (downloaded as f64 / total as f64 * 100.0) as u32,
        });
    }).await.map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}

#[derive(Clone, serde::Serialize)]
struct DownloadProgress {
    downloaded: u64,
    total: u64,
    percent: u32,
}
```

---

### Phase 5: Main App Integration

**File:** `src-tauri/src/main.rs`

```rust
// Add at startup (before showing main window)
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Check if first run
            if is_first_run() {
                // Show onboarding window instead of main UI
                let onboarding_window = tauri::WebviewWindowBuilder::new(
                    app,
                    "onboarding",
                    tauri::WebviewUrl::App("onboarding.html".into())
                )
                .title("Welcome to kVoice")
                .inner_size(600.0, 500.0)
                .center()
                .resizable(false)
                .decorations(true)
                .build()?;

                // Don't show main UI yet
                return Ok(());
            }

            // Normal startup (already completed onboarding)
            setup_fn_key_monitor(app)?;
            // ... rest of setup

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Existing commands
            commands::transcribe,
            // NEW onboarding commands
            commands::is_first_run,
            commands::check_accessibility_permission,
            commands::open_accessibility_settings,
            commands::download_whisper_model,
            commands::mark_onboarding_complete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

### Phase 6: Frontend - Onboarding Window

**File:** `src-ui/onboarding.html` (NEW)

```html
<!DOCTYPE html>
<html>
<head>
    <title>Welcome to kVoice</title>
    <style>
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }

        .wizard {
            background: white;
            border-radius: 16px;
            width: 600px;
            height: 500px;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3);
            overflow: hidden;
        }

        .screen {
            display: none;
            height: 100%;
            padding: 60px;
            flex-direction: column;
            align-items: center;
            text-align: center;
        }

        .screen.active {
            display: flex;
        }

        .icon {
            font-size: 80px;
            margin-bottom: 30px;
        }

        h1 {
            font-size: 32px;
            margin-bottom: 20px;
            color: #333;
        }

        p {
            font-size: 16px;
            color: #666;
            line-height: 1.6;
            margin-bottom: 40px;
        }

        button {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            border: none;
            padding: 14px 32px;
            border-radius: 8px;
            font-size: 16px;
            cursor: pointer;
            transition: transform 0.2s;
        }

        button:hover {
            transform: translateY(-2px);
        }

        .progress-bar {
            width: 100%;
            height: 8px;
            background: #e0e0e0;
            border-radius: 4px;
            overflow: hidden;
            margin: 20px 0;
        }

        .progress-fill {
            height: 100%;
            background: linear-gradient(90deg, #667eea 0%, #764ba2 100%);
            width: 0%;
            transition: width 0.3s;
        }

        .status {
            display: inline-flex;
            align-items: center;
            gap: 8px;
            padding: 12px 20px;
            border-radius: 8px;
            margin: 20px 0;
        }

        .status.pending {
            background: #fff3cd;
            color: #856404;
        }

        .status.success {
            background: #d4edda;
            color: #155724;
        }
    </style>
</head>
<body>
    <div class="wizard">
        <!-- Screen 1: Welcome -->
        <div class="screen active" id="screen-welcome">
            <div class="icon">🎙️</div>
            <h1>Welcome to kVoice</h1>
            <p>Voice-to-text transcription powered by local Whisper AI</p>
            <p>✓ Works offline<br>✓ Private (local processing)<br>✓ Fast & accurate</p>
            <button onclick="nextScreen()">Get Started →</button>
        </div>

        <!-- Screen 2: Accessibility -->
        <div class="screen" id="screen-accessibility">
            <div class="icon">🔐</div>
            <h1>Accessibility Permission</h1>
            <p>kVoice needs accessibility access to monitor the Fn key globally.</p>
            <div id="permission-status" class="status pending">
                ⚠️ Not Granted
            </div>
            <button onclick="openSettings()">Open Settings →</button>
            <button onclick="nextScreen()">Continue →</button>
        </div>

        <!-- Screen 3: Download -->
        <div class="screen" id="screen-download">
            <div class="icon">📥</div>
            <h1>Downloading Whisper AI</h1>
            <p>Model: ggml-small.en.bin (462 MB)</p>
            <div class="progress-bar">
                <div class="progress-fill" id="download-progress"></div>
            </div>
            <p id="download-status">Preparing download...</p>
        </div>

        <!-- Screen 4: Ready -->
        <div class="screen" id="screen-ready">
            <div class="icon">🎉</div>
            <h1>You're All Set!</h1>
            <p>kVoice is ready to transcribe!</p>
            <p><strong>How to Use:</strong></p>
            <p>Press & Hold: Hold Fn to record<br>Double-Tap: Tap Fn twice for hands-free mode</p>
            <button onclick="finishOnboarding()">Try Your First Recording →</button>
        </div>
    </div>

    <script>
        const { invoke, listen } = window.__TAURI__;

        let currentScreen = 0;
        const screens = ['welcome', 'accessibility', 'download', 'ready'];

        function nextScreen() {
            // Hide current
            document.getElementById(`screen-${screens[currentScreen]}`).classList.remove('active');

            // Show next
            currentScreen++;
            document.getElementById(`screen-${screens[currentScreen]}`).classList.add('active');

            // Special actions per screen
            if (screens[currentScreen] === 'accessibility') {
                checkAccessibilityLoop();
            } else if (screens[currentScreen] === 'download') {
                startDownload();
            }
        }

        function openSettings() {
            invoke('open_accessibility_settings');
        }

        async function checkAccessibilityLoop() {
            const check = async () => {
                const granted = await invoke('check_accessibility_permission');
                const status = document.getElementById('permission-status');

                if (granted) {
                    status.className = 'status success';
                    status.textContent = '✅ Granted';
                } else {
                    setTimeout(check, 1000); // Check again in 1s
                }
            };
            check();
        }

        async function startDownload() {
            // Listen for progress events
            await listen('download-progress', (event) => {
                const { downloaded, total, percent } = event.payload;
                const progressBar = document.getElementById('download-progress');
                const statusText = document.getElementById('download-status');

                progressBar.style.width = percent + '%';
                statusText.textContent = `${(downloaded / 1024 / 1024).toFixed(1)} MB / ${(total / 1024 / 1024).toFixed(1)} MB`;
            });

            try {
                await invoke('download_whisper_model');
                nextScreen(); // Go to ready screen
            } catch (error) {
                alert('Download failed: ' + error);
            }
        }

        async function finishOnboarding() {
            await invoke('mark_onboarding_complete');
            // Close onboarding window and start main app
            window.__TAURI__.window.getCurrent().close();
        }
    </script>
</body>
</html>
```

---

## Implementation Timeline

### Week 1: Core Backend
- [ ] Create `onboarding.rs` module
- [ ] Create `downloader.rs` module
- [ ] Add Tauri commands
- [ ] Test accessibility detection
- [ ] Test model download with progress

### Week 2: Frontend UI
- [ ] Design onboarding screens (Figma)
- [ ] Build `onboarding.html`
- [ ] Add CSS animations
- [ ] Wire up Tauri commands
- [ ] Test full flow

### Week 3: Integration & Polish
- [ ] Integrate with main app startup
- [ ] Add error handling (download fails, permissions denied)
- [ ] Add "Skip" option for advanced users
- [ ] Add "Try again" on errors
- [ ] Test on clean Mac (no accessibility, no model)

### Week 4: Testing & Release
- [ ] User testing with 5 people
- [ ] Fix bugs
- [ ] Record demo video
- [ ] Update README with screenshots
- [ ] Ship it! 🚀

---

## Success Metrics

- **Time to First Transcription**: <2 minutes from app launch
- **Completion Rate**: >95% of users complete onboarding
- **Error Rate**: <5% encounter download/permission errors
- **Support Tickets**: <10% of users need help with setup

---

## Future Enhancements

- **Model Selection**: Let user choose Tiny/Small/Medium in wizard
- **Keyboard Shortcut**: Show Fn key shortcut customization
- **Quick Test**: Add "Record Test" button in wizard to verify setup
- **Tutorial Video**: Embed 30-second demo video
- **Settings Import**: Detect macOS dictation settings and suggest migration

---

## Notes

- Keep wizard simple (4 screens max)
- Use native macOS design language
- Show progress, not spinners
- Auto-advance when possible (permissions granted, download complete)
- Allow skip for power users (but warn about consequences)
