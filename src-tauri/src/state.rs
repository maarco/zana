//! Application State
//!
//! Global state shared across Tauri commands.

use crate::audio::{AudioCapture, CapturedAudio};
use crate::hooks::{EventBus, LoggingHandler, MetricsHandler, ValidationHandler};
use crate::plugins::{PluginManager, PluginRegistry};
use crate::stt::WhisperEngine;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WritingProfile {
    /// Why and where the rewritten text will be used
    pub purpose: String,
    /// Tone for rewritten output
    pub tone: String,
    /// Output shape/structure expectation
    pub format: String,
}

impl Default for WritingProfile {
    fn default() -> Self {
        Self {
            purpose: "Produce clear, useful text".to_string(),
            tone: "clear and concise".to_string(),
            format: "one short paragraph".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CloudRewriteSettings {
    /// API key for the chat-completions compatible rewrite provider
    pub api_key: String,
    /// Model name sent to the rewrite provider
    pub model: String,
    /// HTTPS chat-completions compatible endpoint
    pub api_url: String,
    /// Cloud rewrite timeout in milliseconds
    pub timeout_ms: u64,
    /// Attach a screenshot to the rewrite request when the provider supports vision
    pub include_screenshot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DictionaryReplacement {
    pub from: String,
    pub to: String,
    pub enabled: bool,
    pub reason: Option<String>,
}

impl DictionaryReplacement {
    pub fn enabled(from: &str, to: &str, reason: Option<String>) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            enabled: true,
            reason,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TranscriptHistoryEntry {
    pub raw_text: String,
    pub final_text: String,
    pub used_rewrite: bool,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StyleMemory {
    pub rule: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProjectMemory {
    pub key: String,
    pub value: String,
    pub reason: Option<String>,
}

impl Default for CloudRewriteSettings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
            api_url: "https://api.openai.com/v1/chat/completions".to_string(),
            timeout_ms: 15_000,
            include_screenshot: false,
        }
    }
}

/// User settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Selected Whisper model
    pub whisper_model: Option<String>,
    /// Selected audio device
    pub audio_device: Option<String>,
    /// Selected orb style plugin
    pub orb_style: Option<String>,
    /// Preferred transcription language
    pub language: Option<String>,
    /// Whether double-tapping the trigger key latches recording
    pub double_tap_enabled: bool,
    /// Minimum trigger hold duration in milliseconds
    pub min_hold_duration_ms: u64,
    /// Recording trigger key
    pub trigger_key: Option<String>,
    /// Alternative global shortcut
    pub global_shortcut: Option<String>,
    /// Whether to show the tray/menu-bar icon
    pub show_in_menu_bar: bool,
    /// Window always on top
    pub always_on_top: bool,
    /// Window width
    pub window_width: u32,
    /// Window height
    pub window_height: u32,
    /// Enable cloud rewrite after local transcription
    pub cloud_rewrite_enabled: bool,
    /// Cloud rewrite provider config
    pub cloud_rewrite: CloudRewriteSettings,
    /// Selected writing style profile
    pub writing_profile: WritingProfile,
    /// Post-transcription phrase corrections for names, tools, and jargon
    pub dictionary_replacements: Vec<DictionaryReplacement>,
    /// Recent local transcript/rewrite history used as rewrite context
    pub transcript_history: Vec<TranscriptHistoryEntry>,
    /// Learned user style preferences
    pub style_memories: Vec<StyleMemory>,
    /// Learned project terms and context
    pub project_memories: Vec<ProjectMemory>,
}

impl Settings {
    /// Load from disk or create default
    pub fn load() -> Self {
        let path = Self::settings_path();

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<Settings>(&content) {
                    Ok(mut settings) => {
                        settings.hydrate_learned_defaults();
                        return settings;
                    }
                    Err(e) => log::warn!("Failed to parse settings: {}", e),
                },
                Err(e) => log::warn!("Failed to read settings: {}", e),
            }
        }

        Self::default_settings()
    }

    fn hydrate_learned_defaults(&mut self) {
        if self.dictionary_replacements.is_empty() {
            self.dictionary_replacements = default_dictionary_replacements();
        }
    }

    /// Save to disk
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::settings_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;

        log::info!("Settings saved to {:?}", path);
        Ok(())
    }

    fn settings_path() -> std::path::PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Zana")
            .join("settings.json")
    }

    fn default_settings() -> Self {
        Self {
            whisper_model: Some("small".to_string()),
            audio_device: None,
            orb_style: Some("fire-v8".to_string()),
            language: Some("auto".to_string()),
            double_tap_enabled: true,
            min_hold_duration_ms: 300,
            trigger_key: Some("fn".to_string()),
            global_shortcut: Some("Cmd+Shift+Space".to_string()),
            show_in_menu_bar: true,
            always_on_top: true,
            window_width: 500,
            window_height: 500,
            cloud_rewrite_enabled: false,
            cloud_rewrite: CloudRewriteSettings::default(),
            writing_profile: WritingProfile::default(),
            dictionary_replacements: default_dictionary_replacements(),
            transcript_history: Vec::new(),
            style_memories: Vec::new(),
            project_memories: Vec::new(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::default_settings()
    }
}

fn default_dictionary_replacements() -> Vec<DictionaryReplacement> {
    vec![
        DictionaryReplacement::enabled(
            "cloud code",
            "claude code",
            Some("Developer vocabulary correction".to_string()),
        ),
        DictionaryReplacement::enabled(
            "cloud.markdown",
            "claude.md",
            Some("Developer filename correction".to_string()),
        ),
        DictionaryReplacement::enabled(
            "cloud markdown",
            "claude.md",
            Some("Developer filename correction".to_string()),
        ),
    ]
}

/// Global application state
pub struct AppState {
    /// Event bus for hooks
    pub event_bus: Arc<EventBus>,
    /// Plugin registry
    pub plugin_registry: Arc<RwLock<PluginRegistry>>,
    /// Plugin manager
    pub plugin_manager: Arc<Mutex<PluginManager>>,
    /// Audio capture engine
    pub audio_capture: Mutex<AudioCapture>,
    /// Whisper STT engine
    pub whisper_engine: Mutex<WhisperEngine>,
    /// Last captured audio (for transcription)
    pub captured_audio: Mutex<Option<CapturedAudio>>,
    /// User settings
    pub settings: RwLock<Settings>,
    /// Clipboard text captured at record start for rewrite context
    pub recording_clipboard: Mutex<Option<String>>,
    /// Screenshot captured at record release for vision rewrite context
    pub recording_screenshot: Mutex<Option<String>>,
}

impl AppState {
    /// Create new application state
    pub fn new() -> anyhow::Result<Self> {
        let event_bus = Arc::new(EventBus::new());
        let audio_capture = AudioCapture::new(event_bus.clone());
        let whisper_engine = WhisperEngine::new(event_bus.clone())?;
        let settings = Settings::load();

        // Preload whisper model in background
        let whisper_engine_clone = whisper_engine.clone();
        let preload_model = settings.whisper_model.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                if let Some(model_str) = preload_model {
                    if let Ok(model) = model_str.parse::<crate::stt::WhisperModel>() {
                        log::info!("Preloading whisper model: {}", model_str);
                        if let Err(e) = whisper_engine_clone.preload_model(model).await {
                            log::warn!("Failed to preload whisper model: {}", e);
                        }
                    }
                }
            });
        });

        // Create plugin registry
        let plugin_registry = Arc::new(RwLock::new(PluginRegistry::new()));

        // Determine plugins directory
        let plugins_dir = if cfg!(debug_assertions) {
            // Development: use ./plugins in the project root
            PathBuf::from("plugins")
        } else {
            // Production: use $APP_DATA/plugins
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Zana")
                .join("plugins")
        };

        // Create plugin manager
        let plugin_manager = Arc::new(Mutex::new(PluginManager::new(
            plugin_registry.clone(),
            event_bus.clone(),
            plugins_dir,
        )));

        // Register system handlers
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            // Logging handler - logs all events at DEBUG level
            let logging_handler = Arc::new(LoggingHandler);
            event_bus.register(logging_handler).await?;

            // Metrics handler - tracks event counts/timing
            let metrics_handler = Arc::new(MetricsHandler::new());
            event_bus.register(metrics_handler).await?;

            // Validation handler - validates event data
            let validation_handler = Arc::new(ValidationHandler::new());
            event_bus.register(validation_handler).await?;

            log::info!("System handlers registered: logging, metrics, validation");
            anyhow::Ok(())
        })?;

        Ok(Self {
            event_bus,
            plugin_registry,
            plugin_manager,
            audio_capture: Mutex::new(audio_capture),
            whisper_engine: Mutex::new(whisper_engine),
            captured_audio: Mutex::new(None),
            settings: RwLock::new(settings),
            recording_clipboard: Mutex::new(None),
            recording_screenshot: Mutex::new(None),
        })
    }

    /// Capture clipboard text for context before transcription.
    pub async fn capture_recording_clipboard(&self) {
        let snapshot = arboard::Clipboard::new()
            .ok()
            .and_then(|mut clipboard| clipboard.get_text().ok());

        *self.recording_clipboard.lock().await = snapshot;
    }

    /// Take and clear the current clipboard context snapshot.
    pub async fn take_recording_clipboard(&self) -> Option<String> {
        self.recording_clipboard.lock().await.take()
    }

    /// Capture screen context at recording release for vision-capable rewrite providers.
    pub async fn capture_recording_screenshot_if_enabled(&self) {
        let include_screenshot = {
            let settings = self.settings.read().await;
            settings.cloud_rewrite.include_screenshot
        };

        if !include_screenshot {
            *self.recording_screenshot.lock().await = None;
            return;
        }

        let screenshot = match capture_screenshot_data_url().await {
            Ok(screenshot) => Some(screenshot),
            Err(error) => {
                log::warn!("Screenshot context unavailable: {}", error);
                None
            }
        };

        *self.recording_screenshot.lock().await = screenshot;
    }

    /// Take and clear the current release-time screenshot context snapshot.
    pub async fn take_recording_screenshot(&self) -> Option<String> {
        self.recording_screenshot.lock().await.take()
    }

    /// Clear all per-recording context so failed/local-only flows cannot leak later.
    pub async fn clear_recording_context(&self) {
        *self.recording_clipboard.lock().await = None;
        *self.recording_screenshot.lock().await = None;
    }

    /// Save settings
    pub async fn save_settings(&self) -> anyhow::Result<()> {
        let settings = self.settings.read().await;
        settings.save()
    }

    /// Load all plugins from the plugins directory
    pub async fn load_plugins(&self) -> anyhow::Result<usize> {
        let manager = self.plugin_manager.lock().await;
        let count = manager.load_all().await?;
        log::info!("Loaded {} plugin(s)", count);
        Ok(count)
    }
}

async fn capture_screenshot_data_url() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        tokio::task::spawn_blocking(capture_macos_screenshot_data_url)
            .await
            .map_err(|error| format!("Screenshot capture task failed: {error}"))?
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Screenshot context is only implemented on macOS".to_string())
    }
}

#[cfg(target_os = "macos")]
fn capture_macos_screenshot_data_url() -> Result<String, String> {
    let path = std::env::temp_dir().join(format!(
        "qvoice-rewrite-context-{}-{}.jpg",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    ));

    let output = Command::new("/usr/sbin/screencapture")
        .arg("-x")
        .arg("-t")
        .arg("jpg")
        .arg(&path)
        .output()
        .map_err(|error| format!("Failed to run screencapture: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_file(&path);
        return Err(format!("screencapture failed: {}", stderr.trim()));
    }

    let bytes = std::fs::read(&path).map_err(|error| {
        let _ = std::fs::remove_file(&path);
        format!("Failed to read screenshot: {error}")
    })?;
    let _ = std::fs::remove_file(&path);

    if bytes.is_empty() {
        return Err("Screenshot capture returned an empty image".to_string());
    }

    Ok(format!("data:image/jpeg;base64,{}", BASE64.encode(bytes)))
}
