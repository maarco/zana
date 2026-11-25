//! Tauri commands for the live Preferences window.

use crate::state::{AppState, Settings};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, State};
use tokio::time::{timeout, Duration};

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTestResponse {
    pub success: bool,
    pub message: String,
}

/// Preferences edited in the live Preferences window.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub rewrite_api_key: String,
    pub rewrite_model: String,
    pub rewrite_api_url: String,
    #[serde(default = "default_rewrite_timeout_ms")]
    pub rewrite_timeout_ms: u64,
    #[serde(default)]
    pub model_reasoning: String,
    #[serde(default)]
    pub vision_model: String,
}

fn default_rewrite_timeout_ms() -> u64 {
    15_000
}

impl Preferences {
    fn from_settings(settings: &Settings) -> Self {
        Self {
            rewrite_api_key: settings.cloud_rewrite.api_key.clone(),
            rewrite_model: settings.cloud_rewrite.model.clone(),
            rewrite_api_url: settings.cloud_rewrite.api_url.clone(),
            rewrite_timeout_ms: settings.cloud_rewrite.timeout_ms,
            model_reasoning: settings.cloud_rewrite.reasoning.clone(),
            vision_model: settings.cloud_rewrite.vision_model.clone(),
        }
    }

    fn validate(&self) -> Result<(), String> {
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

    fn apply_to_settings(&self, settings: &mut Settings) {
        settings.cloud_rewrite = crate::state::CloudRewriteSettings {
            api_key: self.rewrite_api_key.clone(),
            model: self.rewrite_model.trim().to_string(),
            api_url: self.rewrite_api_url.trim().to_string(),
            timeout_ms: self.rewrite_timeout_ms,
            reasoning: self.model_reasoning.clone(),
            vision_model: self.vision_model.trim().to_string(),
        };
    }

    fn validate_ai_test(&self) -> Result<(), String> {
        self.validate()
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

/// Load the provider fields shown in Preferences > Model.
#[tauri::command]
pub async fn get_preferences(state: State<'_, AppState>) -> Result<Preferences, String> {
    let settings = state.settings.read().await;
    Ok(Preferences::from_settings(&settings))
}

/// Persist the provider fields shown in Preferences > Model.
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

    let (settings, model_changed) = {
        let mut settings = state.settings.write().await;
        let before = settings.cloud_rewrite.clone();
        preferences.apply_to_settings(&mut settings);
        let after = &settings.cloud_rewrite;
        let changed = before.api_url != after.api_url
            || before.model != after.model
            || before.api_key != after.api_key
            || before.reasoning != after.reasoning
            || before.vision_model != after.vision_model;
        (settings.clone(), changed)
    };

    if let Err(error) = state.save_settings().await {
        return Ok(SettingsResponse {
            success: false,
            error: Some(error.to_string()),
        });
    }

    if let Err(error) = crate::voicebot::write_agent_config(&settings) {
        log::warn!("Could not update Voicebot's config: {error}");
        return Ok(SettingsResponse {
            success: false,
            error: Some(format!(
                "Saved, but Voicebot's settings file could not be written: {error}"
            )),
        });
    }

    if model_changed {
        let _ = app.emit("voicebot-config-changed", ());
    }
    log::info!("Preferences saved");

    Ok(SettingsResponse {
        success: true,
        error: None,
    })
}

/// Test the configured Voicebot provider using the current Preferences values.
#[tauri::command]
pub async fn test_ai_connection(preferences: Preferences) -> Result<AiTestResponse, String> {
    if let Err(error) = preferences.validate_ai_test() {
        return Ok(AiTestResponse {
            success: false,
            message: error,
        });
    }

    let mut request_body = json!({
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
        "max_tokens": 64
    });

    if let (Some(body), serde_json::Value::Object(extra)) = (
        request_body.as_object_mut(),
        crate::voicebot::llm_extra_body(&preferences.rewrite_api_url, &preferences.model_reasoning),
    ) {
        body.extend(extra);
    }

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
            .map_err(|error| format!("AI test request failed: {error}"))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let detail = body.trim();
            let message = if detail.is_empty() {
                format!("AI test failed with status {status}")
            } else {
                format!("AI test failed with status {status}: {detail}")
            };
            return Err(message);
        }

        response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| format!("AI test response parse failed: {error}"))
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

#[cfg(test)]
mod tests {
    use super::Preferences;

    fn test_preferences() -> Preferences {
        Preferences {
            rewrite_api_key: "sk-test".to_string(),
            rewrite_model: "gpt-4o-mini".to_string(),
            rewrite_api_url: "https://api.openai.com/v1/chat/completions".to_string(),
            rewrite_timeout_ms: 15_000,
            model_reasoning: "low".to_string(),
            vision_model: String::new(),
        }
    }

    #[test]
    fn remote_provider_requires_an_api_key() {
        let mut preferences = test_preferences();
        preferences.rewrite_api_key.clear();

        assert_eq!(
            preferences.validate().unwrap_err(),
            "Rewrite API key is required for remote providers"
        );
    }

    #[test]
    fn local_provider_does_not_require_an_api_key() {
        let mut preferences = test_preferences();
        preferences.rewrite_api_key.clear();
        preferences.rewrite_api_url = "http://localhost:11434/v1/chat/completions".to_string();

        assert!(preferences.validate_ai_test().is_ok());
    }

    #[test]
    fn insecure_remote_url_is_rejected() {
        let mut preferences = test_preferences();
        preferences.rewrite_api_url = "http://example.com/v1/chat/completions".to_string();

        assert_eq!(
            preferences.validate_ai_test().unwrap_err(),
            "Rewrite API URL must use https unless it is localhost or 127.0.0.1"
        );
    }

    #[test]
    fn local_server_root_is_expanded_for_connection_test() {
        assert_eq!(
            super::normalize_ai_test_url("http://localhost:11434"),
            "http://localhost:11434/v1/chat/completions"
        );
    }
}
