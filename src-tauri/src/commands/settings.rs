//! Settings Commands
//!
//! Tauri commands for app settings like orb style.

use crate::state::{AppState, Settings};
use crate::stt::WhisperModel;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::time::{timeout, Duration};

/// Response for settings operations
#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response for testing the configured AI rewrite provider.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTestResponse {
    pub success: bool,
    pub message: String,
}

/// Transcript history item shown in Preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptHistoryItem {
    pub raw_text: String,
    pub final_text: String,
    pub used_rewrite: bool,
    pub timestamp: String,
}

/// Preferences edited in the Preferences window.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub cloud_rewrite_enabled: bool,
    pub whisper_model: String,
    pub language: String,
    pub audio_input: String,
    pub double_tap_enabled: bool,
    pub min_hold_duration: u64,
    pub trigger_key: String,
    pub global_shortcut: String,
    pub orb_style: String,
    pub show_in_menu_bar: bool,
    pub rewrite_api_key: String,
    pub rewrite_model: String,
    pub rewrite_api_url: String,
    pub rewrite_timeout_ms: u64,
    pub rewrite_include_screenshot: bool,
    pub writing_purpose: String,
    pub writing_tone: String,
    pub writing_format: String,
}

impl Preferences {
    fn from_settings(settings: &Settings) -> Self {
        Self {
            cloud_rewrite_enabled: settings.cloud_rewrite_enabled,
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
            writing_purpose: settings.writing_profile.purpose.clone(),
            writing_tone: settings.writing_profile.tone.clone(),
            writing_format: settings.writing_profile.format.clone(),
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
            rewrite_api_key: settings.cloud_rewrite.api_key.clone(),
            rewrite_model: settings.cloud_rewrite.model.clone(),
            rewrite_api_url: settings.cloud_rewrite.api_url.clone(),
            rewrite_timeout_ms: settings.cloud_rewrite.timeout_ms,
            rewrite_include_screenshot: settings.cloud_rewrite.include_screenshot,
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

        if self.writing_purpose.trim().is_empty() {
            return Err("System prompt cannot be empty".to_string());
        }

        if self.writing_tone.trim().is_empty() {
            return Err("Prompt template cannot be empty".to_string());
        }

        if !self.writing_tone.contains("{captured}") {
            return Err("User prompt must include {captured}".to_string());
        }

        if self.cloud_rewrite_enabled
            && self.rewrite_api_key.trim().is_empty()
            && !is_local_rewrite_url(&self.rewrite_api_url)
        {
            return Err("Rewrite API key is required when Writing Polish is enabled".to_string());
        }

        if self.rewrite_model.trim().is_empty() {
            return Err("Rewrite model cannot be empty".to_string());
        }

        if !is_allowed_rewrite_url(&self.rewrite_api_url) {
            return Err(
                "Rewrite API URL must use https unless it is localhost or 127.0.0.1".to_string(),
            );
        }

        if self.rewrite_timeout_ms == 0 {
            return Err("Rewrite timeout must be greater than zero".to_string());
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
        settings.cloud_rewrite_enabled = self.cloud_rewrite_enabled;
        settings.cloud_rewrite = crate::state::CloudRewriteSettings {
            api_key: self.rewrite_api_key.clone(),
            model: self.rewrite_model.clone(),
            api_url: self.rewrite_api_url.clone(),
            timeout_ms: self.rewrite_timeout_ms,
            include_screenshot: self.rewrite_include_screenshot,
        };
        settings.double_tap_enabled = self.double_tap_enabled;
        settings.min_hold_duration_ms = self.min_hold_duration;
        settings.writing_profile = crate::state::WritingProfile {
            purpose: self.writing_purpose.clone(),
            tone: self.writing_tone.clone(),
            format: self.writing_format.clone(),
        };
        settings.trigger_key = Some(self.trigger_key.clone());
        settings.global_shortcut = Some(self.global_shortcut.clone());
        settings.orb_style = Some(self.orb_style.clone());
        settings.show_in_menu_bar = self.show_in_menu_bar;
    }

    fn validate_ai_test(&self) -> Result<(), String> {
        if self.rewrite_model.trim().is_empty() {
            return Err("Rewrite model cannot be empty".to_string());
        }

        if !is_allowed_rewrite_url(&self.rewrite_api_url) {
            return Err(
                "Rewrite API URL must use https unless it is localhost or 127.0.0.1".to_string(),
            );
        }

        if !is_local_rewrite_url(&self.rewrite_api_url) && self.rewrite_api_key.trim().is_empty() {
            return Err("Rewrite API key is required for remote providers".to_string());
        }

        if self.rewrite_timeout_ms == 0 {
            return Err("Rewrite timeout must be greater than zero".to_string());
        }

        Ok(())
    }
}

fn is_local_rewrite_url(url: &str) -> bool {
    url.starts_with("http://localhost:")
        || url.starts_with("http://127.0.0.1:")
        || url.starts_with("http://[::1]:")
}

fn is_allowed_rewrite_url(url: &str) -> bool {
    url.starts_with("https://") || is_local_rewrite_url(url)
}

fn normalize_ai_test_url(url: &str) -> String {
    let trimmed = url.trim();
    if !is_local_rewrite_url(trimmed) {
        return trimmed.to_string();
    }

    match reqwest::Url::parse(trimmed) {
        Ok(mut parsed) if parsed.path() == "/" || parsed.path().is_empty() => {
            parsed.set_path("/v1/chat/completions");
            parsed.to_string()
        }
        _ => trimmed.to_string(),
    }
}

fn transcript_history_items(settings: &Settings) -> Vec<TranscriptHistoryItem> {
    settings
        .transcript_history
        .iter()
        .rev()
        .map(|entry| TranscriptHistoryItem {
            raw_text: entry.raw_text.clone(),
            final_text: entry.final_text.clone(),
            used_rewrite: entry.used_rewrite,
            timestamp: entry.timestamp.clone(),
        })
        .collect()
}

/// Load preferences for the Preferences window.
#[tauri::command]
pub async fn get_preferences(state: State<'_, AppState>) -> Result<Preferences, String> {
    let settings = state.settings.read().await;
    Ok(Preferences::from_settings(&settings))
}

/// Load recent locally saved transcriptions for the Preferences window.
#[tauri::command]
pub async fn get_transcript_history(
    state: State<'_, AppState>,
) -> Result<Vec<TranscriptHistoryItem>, String> {
    let settings = state.settings.read().await;
    Ok(transcript_history_items(&settings))
}

/// Open (or focus) the standalone Transcriptions window.
#[tauri::command]
pub async fn open_transcriptions_window(app: AppHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    if let Some(window) = app.get_webview_window("transcriptions") {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        "transcriptions",
        WebviewUrl::App("transcriptions.html".into()),
    )
    .title("Zana Transcriptions")
    .inner_size(560.0, 640.0)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
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

/// Test the AI rewrite provider using the current Preferences form values.
#[tauri::command]
pub async fn test_ai_connection(preferences: Preferences) -> Result<AiTestResponse, String> {
    if let Err(error) = preferences.validate_ai_test() {
        return Ok(AiTestResponse {
            success: false,
            message: error,
        });
    }

    let request_body = json!({
        "model": preferences.rewrite_model.trim(),
        "messages": [
            {
                "role": "system",
                "content": "Reply with exactly: Zana AI test ok"
            },
            {
                "role": "user",
                "content": "Zana AI connection test."
            }
        ],
        "temperature": 0,
        "max_tokens": 16
    });

    let client = reqwest::Client::new();
    let url = normalize_ai_test_url(&preferences.rewrite_api_url);
    let call = async {
        let mut request = client.post(&url).json(&request_body);

        if !preferences.rewrite_api_key.trim().is_empty() {
            request = request.bearer_auth(preferences.rewrite_api_key.trim());
        }

        let response = request
            .send()
            .await
            .map_err(|error| format!("AI test request failed: {}", error))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let detail = body.trim();
            let message = if detail.is_empty() {
                format!("AI test failed with status {}", status)
            } else {
                format!("AI test failed with status {}: {}", status, detail)
            };
            return Err(message);
        }

        response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| format!("AI test response parse failed: {}", error))
    };

    match timeout(Duration::from_millis(preferences.rewrite_timeout_ms), call).await {
        Ok(Ok(_payload)) => Ok(AiTestResponse {
            success: true,
            message: "AI provider responded".to_string(),
        }),
        Ok(Err(error)) => Ok(AiTestResponse {
            success: false,
            message: error,
        }),
        Err(_) => Ok(AiTestResponse {
            success: false,
            message: "AI test timed out".to_string(),
        }),
    }
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

#[cfg(test)]
mod tests {
    use super::{transcript_history_items, Preferences};
    use crate::state::{Settings, TranscriptHistoryEntry};

    fn test_preferences() -> Preferences {
        Preferences {
            cloud_rewrite_enabled: true,
            whisper_model: "small".to_string(),
            language: "auto".to_string(),
            audio_input: "default".to_string(),
            double_tap_enabled: true,
            min_hold_duration: 300,
            trigger_key: "fn".to_string(),
            global_shortcut: "Cmd+Shift+Space".to_string(),
            orb_style: "fire-v8".to_string(),
            show_in_menu_bar: true,
            rewrite_api_key: "sk-test".to_string(),
            rewrite_model: "gpt-4o-mini".to_string(),
            rewrite_api_url: "https://api.openai.com/v1/chat/completions".to_string(),
            rewrite_timeout_ms: 15_000,
            rewrite_include_screenshot: false,
            writing_purpose: "Write polished text".to_string(),
            writing_tone: "Rewrite this:\n{captured}".to_string(),
            writing_format: String::new(),
        }
    }

    #[test]
    fn transcript_history_items_are_newest_first() {
        let settings = Settings {
            transcript_history: vec![
                TranscriptHistoryEntry {
                    raw_text: "first raw".to_string(),
                    final_text: "first final".to_string(),
                    used_rewrite: true,
                    timestamp: "2026-05-28T17:00:00Z".to_string(),
                },
                TranscriptHistoryEntry {
                    raw_text: "second raw".to_string(),
                    final_text: "second raw".to_string(),
                    used_rewrite: false,
                    timestamp: "2026-05-28T17:05:00Z".to_string(),
                },
            ],
            ..Default::default()
        };

        let history = transcript_history_items(&settings);

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].raw_text, "second raw");
        assert_eq!(history[0].final_text, "second raw");
        assert!(!history[0].used_rewrite);
        assert_eq!(history[1].raw_text, "first raw");
        assert!(history[1].used_rewrite);
    }

    #[test]
    fn ai_test_requires_remote_api_key() {
        let mut prefs = test_preferences();
        prefs.rewrite_api_key.clear();

        let error = prefs.validate_ai_test().unwrap_err();

        assert_eq!(error, "Rewrite API key is required for remote providers");
    }

    #[test]
    fn enabled_rewrite_requires_captured_variable_in_user_prompt() {
        let mut prefs = test_preferences();
        prefs.writing_tone = "clear and concise".to_string();

        let error = prefs.validate().unwrap_err();

        assert_eq!(error, "User prompt must include {captured}");
    }

    #[test]
    fn ai_test_allows_local_url_without_api_key() {
        let mut prefs = test_preferences();
        prefs.rewrite_api_key.clear();
        prefs.rewrite_api_url = "http://localhost:11434/v1/chat/completions".to_string();

        assert!(prefs.validate_ai_test().is_ok());
    }

    #[test]
    fn ai_test_rejects_insecure_remote_url() {
        let mut prefs = test_preferences();
        prefs.rewrite_api_url = "http://example.com/v1/chat/completions".to_string();

        let error = prefs.validate_ai_test().unwrap_err();

        assert_eq!(
            error,
            "Rewrite API URL must use https unless it is localhost or 127.0.0.1"
        );
    }

    #[test]
    fn ai_test_expands_local_server_root() {
        assert_eq!(
            super::normalize_ai_test_url("http://localhost:11434"),
            "http://localhost:11434/v1/chat/completions"
        );
    }
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
