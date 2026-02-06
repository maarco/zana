//! Application State
//!
//! Global state shared across Tauri commands.

use crate::audio::{AudioCapture, CapturedAudio};
use crate::errors::{Result, SettingsError};
use crate::hooks::EventBus;
use crate::stt::WhisperEngine;
use serde::{Deserialize, Serialize};
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

        log::debug!("Loading settings from {:?}", path);

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str::<Settings>(&content) {
                        Ok(settings) => {
                            log::info!("Settings loaded successfully from {:?}", path);
                            log::trace!("Loaded settings: model={:?}, device={:?}, orb_style={:?}",
                                settings.whisper_model, settings.audio_device, settings.orb_style);
                            return settings;
                        }
                        Err(e) => {
                            log::error!("Failed to parse settings from {:?}: {}", path, e);
                            log::warn!("Settings file may be corrupted. Using default settings. Error details available if needed.");
                            log::info!("Using default settings");
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to read settings from {:?}: {}", path, e);
                    log::info!("Using default settings");
                }
            }
        } else {
            log::info!("Settings file not found at {:?}, using defaults", path);
        }

        let defaults = Self::default_settings();
        log::debug!("Using default settings");
        defaults
    }

    /// Save to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::settings_path();

        log::debug!("Saving settings to {:?}", path);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                log::trace!("Creating settings directory: {:?}", parent);
                std::fs::create_dir_all(parent).map_err(|e| SettingsError::DirectoryCreationFailed {
                    path: parent.to_path_buf(),
                    source: Some(Box::new(e) as _),
                })?;
            }
        }

        let content = serde_json::to_string_pretty(self).map_err(|e| SettingsError::SerializationFailed {
            reason: "Failed to serialize settings".to_string(),
            source: Some(Box::new(e) as _),
        })?;

        std::fs::write(&path, &content).map_err(|e| {
            let kind = e.kind();
            match kind {
                std::io::ErrorKind::PermissionDenied => SettingsError::PermissionDenied {
                    path: path.clone(),
                    source: Some(Box::new(e) as _),
                },
                std::io::ErrorKind::StorageFull => SettingsError::DiskFull {
                    required_bytes: content.len() as u64,
                    source: Some(Box::new(e) as _),
                },
                _ => SettingsError::WriteFailed {
                    path: path.clone(),
                    source: Some(Box::new(e) as _),
                },
            }
        })?;

        log::info!("Settings saved to {:?} ({} bytes)", path, content.len());
        log::trace!("Saved settings: model={:?}, device={:?}, orb_style={:?}",
            self.whisper_model, self.audio_device, self.orb_style);
        Ok(())
    }

    pub fn settings_path() -> std::path::PathBuf {
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
    /// Audio capture engine
    pub audio_capture: Arc<Mutex<AudioCapture>>,
    /// Whisper STT engine
    pub whisper_engine: Arc<Mutex<WhisperEngine>>,
    /// Last captured audio (for transcription)
    pub captured_audio: Mutex<Option<CapturedAudio>>,
    /// User settings
    pub settings: RwLock<Settings>,
    /// Tokio runtime handle for spawning async tasks from GUI
    pub runtime_handle: std::sync::RwLock<Option<tokio::runtime::Handle>>,
}

impl AppState {
    /// Create new application state
    pub fn new() -> Result<Self> {
        log::info!("Initializing application state");

        let event_bus = Arc::new(EventBus::new());
        log::debug!("EventBus created");

        let audio_capture = AudioCapture::new(event_bus.clone());
        log::debug!("AudioCapture created");

        let whisper_engine = WhisperEngine::new(event_bus.clone())?;
        log::debug!("WhisperEngine created");

        let settings = Settings::load();
        log::info!("Settings loaded");

        Ok(Self {
            event_bus,
            audio_capture: Arc::new(Mutex::new(audio_capture)),
            whisper_engine: Arc::new(Mutex::new(whisper_engine)),
            captured_audio: Mutex::new(None),
            settings: RwLock::new(settings),
            runtime_handle: std::sync::RwLock::new(None),
        })
    }

    /// Save settings
    pub async fn save_settings(&self) -> Result<()> {
        let settings = self.settings.read().await;
        settings.save()
    }
}
