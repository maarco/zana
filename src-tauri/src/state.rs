//! Application State
//!
//! Global state shared across Tauri commands.

use crate::audio::{AudioCapture, CapturedAudio};
use crate::hooks::{EventBus, LoggingHandler, MetricsHandler, ValidationHandler};
use crate::plugins::{PluginManager, PluginRegistry};
use crate::stt::WhisperEngine;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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
    /// Selected writing style profile
    pub writing_profile: WritingProfile,
}

impl Settings {
    /// Load from disk or create default
    pub fn load() -> Self {
        let path = Self::settings_path();

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(settings) => return settings,
                    Err(e) => log::warn!("Failed to parse settings: {}", e),
                },
                Err(e) => log::warn!("Failed to read settings: {}", e),
            }
        }

        Self::default_settings()
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
            writing_profile: WritingProfile::default(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::default_settings()
    }
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
