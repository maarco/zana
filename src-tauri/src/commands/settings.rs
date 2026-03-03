//! Settings Commands
//!
//! Tauri commands for app settings like orb style.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

/// Response for settings operations
#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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
