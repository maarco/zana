//! Application state shared by the live Voicebot and Preferences commands.

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CloudRewriteSettings {
    /// API key for the chat-completions compatible Voicebot provider.
    pub api_key: String,
    /// Model name sent to the Voicebot provider.
    pub model: String,
    /// HTTPS chat-completions compatible endpoint.
    pub api_url: String,
    /// Timeout used by the Preferences provider connection test.
    pub timeout_ms: u64,
    /// How much the model reasons before answering.
    pub reasoning: String,
    /// Model that reads screenshots; empty means the chat model.
    pub vision_model: String,
}

impl Default for CloudRewriteSettings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
            api_url: "https://api.openai.com/v1/chat/completions".to_string(),
            timeout_ms: 15_000,
            reasoning: "low".to_string(),
            vision_model: String::new(),
        }
    }
}

/// User settings used by the live Voicebot application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Voicebot's chat model provider configuration.
    pub cloud_rewrite: CloudRewriteSettings,
    /// Give Voicebot's agent a terminal tool.
    pub voicebot_terminal: bool,
    /// Optional zsh command whose output Voicebot gets as screen history.
    pub voicebot_context_command: String,
    /// Wake word; empty means Voicebot always answers.
    pub voicebot_wake_word: String,
    /// Voicebot's voice; empty means the default voice.
    pub voicebot_voice: String,
    /// Thinking sounds while Voicebot works out a reply.
    pub voicebot_fillers: bool,
    /// Thinking sound lines; empty means the agent's built-in set.
    pub voicebot_filler_phrases: Vec<String>,
    /// Days before the thinking sounds are made again.
    pub voicebot_filler_days: u32,
    /// Minutes without microphone sound before the orb pulses red.
    pub voicebot_mic_check_minutes: u32,
    /// The orb's visual design.
    pub voicebot_orb_design: String,
    /// Minutes Voicebot waits between mood changes of its orb.
    pub voicebot_mood_minutes: u32,
}

impl Settings {
    /// Load settings from disk or return the live defaults.
    pub fn load() -> Self {
        let path = Self::settings_path();

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<Settings>(&content) {
                    Ok(settings) => return settings,
                    Err(error) => log::warn!("Failed to parse settings: {error}"),
                },
                Err(error) => log::warn!("Failed to read settings: {error}"),
            }
        }

        Self::default()
    }

    /// Save settings to the local application settings file.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::settings_path();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        log::info!("Settings saved to {path:?}");
        Ok(())
    }

    fn settings_path() -> std::path::PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Zana")
            .join("settings.json")
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cloud_rewrite: CloudRewriteSettings::default(),
            voicebot_terminal: false,
            voicebot_context_command: String::new(),
            voicebot_wake_word: String::new(),
            voicebot_voice: String::new(),
            voicebot_fillers: true,
            voicebot_filler_phrases: Vec::new(),
            voicebot_filler_days: 7,
            voicebot_mic_check_minutes: 2,
            voicebot_orb_design: "original".to_string(),
            voicebot_mood_minutes: 3,
        }
    }
}

/// Global state for the live Tauri commands.
pub struct AppState {
    pub settings: RwLock<Settings>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            settings: RwLock::new(Settings::load()),
        })
    }

    pub async fn save_settings(&self) -> anyhow::Result<()> {
        let settings = self.settings.read().await;
        settings.save()
    }
}
