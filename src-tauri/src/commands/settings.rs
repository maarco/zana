//! Settings Commands
//!
//! Tauri commands for app settings like orb style.

use crate::state::{AppState, Settings};
use crate::stt::WhisperModel;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

/// Response for settings operations
#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Preferences edited in the Preferences window.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub whisper_model: String,
    pub language: String,
    pub audio_input: String,
    pub double_tap_enabled: bool,
    pub min_hold_duration: u64,
    pub trigger_key: String,
    pub global_shortcut: String,
    pub orb_style: String,
    pub show_in_menu_bar: bool,
}

impl Preferences {
    fn from_settings(settings: &Settings) -> Self {
        Self {
            whisper_model: settings
                .whisper_model
                .clone()
                .unwrap_or_else(|| "small".to_string()),
            language: settings
                .language
                .clone()
                .unwrap_or_else(|| "auto".to_string()),
            audio_input: settings
                .audio_device
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            double_tap_enabled: settings.double_tap_enabled,
            min_hold_duration: settings.min_hold_duration_ms,
            trigger_key: settings
                .trigger_key
                .clone()
                .unwrap_or_else(|| "fn".to_string()),
            global_shortcut: settings
                .global_shortcut
                .clone()
                .unwrap_or_else(|| "Cmd+Shift+Space".to_string()),
            orb_style: settings
                .orb_style
                .clone()
                .unwrap_or_else(|| "fire-v8".to_string()),
            show_in_menu_bar: settings.show_in_menu_bar,
        }
    }

    fn validate(&self) -> Result<(), String> {
        self.whisper_model
            .parse::<WhisperModel>()
            .map(|_| ())
            .map_err(|_| format!("Unknown model: {}", self.whisper_model))?;

        if self.min_hold_duration == 0 {
            return Err("Minimum hold duration must be greater than zero".to_string());
        }

        Ok(())
    }

    fn apply_to_settings(&self, settings: &mut Settings) {
        settings.whisper_model = Some(self.whisper_model.clone());
        settings.language = Some(self.language.clone());
        settings.audio_device = match self.audio_input.as_str() {
            "" | "default" => None,
            device => Some(device.to_string()),
        };
        settings.double_tap_enabled = self.double_tap_enabled;
        settings.min_hold_duration_ms = self.min_hold_duration;
        settings.trigger_key = Some(self.trigger_key.clone());
        settings.global_shortcut = Some(self.global_shortcut.clone());
        settings.orb_style = Some(self.orb_style.clone());
        settings.show_in_menu_bar = self.show_in_menu_bar;
    }
}

/// Load preferences for the Preferences window.
#[tauri::command]
pub async fn get_preferences(state: State<'_, AppState>) -> Result<Preferences, String> {
    let settings = state.settings.read().await;
    Ok(Preferences::from_settings(&settings))
}

/// Persist all preferences edited in the Preferences window.
#[tauri::command]
pub async fn save_preferences(
    app: AppHandle,
    state: State<'_, AppState>,
    preferences: Preferences,
) -> Result<SettingsResponse, String> {
    if let Err(error) = preferences.validate() {
        return Ok(SettingsResponse {
            success: false,
            error: Some(error),
        });
    }

    {
        let mut settings = state.settings.write().await;
        preferences.apply_to_settings(&mut settings);
    }

    if let Err(error) = state.save_settings().await {
        return Ok(SettingsResponse {
            success: false,
            error: Some(error.to_string()),
        });
    }

    let _ = app.emit("orb-style-changed", &preferences.orb_style);
    log::info!("Preferences saved");

    Ok(SettingsResponse {
        success: true,
        error: None,
    })
}

/// Set the orb visual style
///
/// Saves the style to settings and notifies the orb window to reload.
/// On macOS the orb runs in an NSPanel so we emit an internal event
/// that main.rs relays via eval_js_in_panel.
#[tauri::command]
pub async fn set_orb_style(
    app: AppHandle,
    state: State<'_, AppState>,
    style: String,
) -> Result<SettingsResponse, String> {
    // Save to settings
    {
        let mut settings = state.settings.write().await;
        settings.orb_style = Some(style.clone());
    }

    // Persist to disk
    if let Err(e) = state.save_settings().await {
        log::warn!("Failed to persist orb style setting: {}", e);
    }

    // Emit event for main.rs to relay to the orb panel/window
    let _ = app.emit("orb-style-changed", &style);
    log::info!("Orb style changed to: {}", style);

    Ok(SettingsResponse {
        success: true,
        error: None,
    })
}

/// Get the current orb style
#[tauri::command]
pub async fn get_orb_style(state: State<'_, AppState>) -> Result<String, String> {
    let settings = state.settings.read().await;
    Ok(settings
        .orb_style
        .clone()
        .unwrap_or_else(|| "fire-v8".to_string()))
}
