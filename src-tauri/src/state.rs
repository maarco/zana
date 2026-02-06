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

/// User settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Selected Whisper model
    pub whisper_model: Option<String>,
    /// Selected audio device
    pub audio_device: Option<String>,
    /// Selected orb style plugin
    pub orb_style: Option<String>,
    /// Window always on top
    pub always_on_top: bool,
    /// Window width
    pub window_width: u32,
    /// Window height
    pub window_height: u32,
}

impl Settings {
    /// Load from disk or create default
    pub fn load() -> Self {
        let path = Self::settings_path();

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str(&content) {
                        Ok(settings) => return settings,
                        Err(e) => log::warn!("Failed to parse settings: {}", e),
                    }
                }
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
            .join("kvoice")
            .join("settings.json")
    }

    fn default_settings() -> Self {
        Self {
            whisper_model: Some("small".to_string()),
            audio_device: None,
            orb_style: Some("nebula-aura-gpu".to_string()),
            always_on_top: true,
            window_width: 500,
            window_height: 500,
        }
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
}

impl AppState {
    /// Create new application state
    pub fn new() -> anyhow::Result<Self> {
        let event_bus = Arc::new(EventBus::new());
        let audio_capture = AudioCapture::new(event_bus.clone());
        let whisper_engine = WhisperEngine::new(event_bus.clone())?;
        let settings = Settings::load();

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
                .join("kvoice")
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
        })
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
