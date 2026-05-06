//! Transcription Commands
//!
//! Tauri commands for speech-to-text using Whisper.

use crate::state::{AppState, WritingProfile};
use crate::stt::WhisperModel;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tauri::{Emitter, State};
use tokio::time::timeout;

/// Model info for UI
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u64,
    pub downloaded: bool,
}

/// Response for model operations
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub success: bool,
    pub models: Vec<ModelInfo>,
    pub current_model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response for transcription
#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewritten_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewrite_error: Option<String>,
    pub used_cloud_rewrite: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<SegmentInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub processing_ms: u64,
}

/// Segment info for UI
#[derive(Debug, Serialize, Deserialize)]
pub struct SegmentInfo {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// Download progress event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub model: String,
    pub downloaded: u64,
    pub total: u64,
    pub percent: f32,
}

/// List available Whisper models
#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<ModelsResponse, String> {
    let engine = state.whisper_engine.lock().await;

    let all_models = [
        WhisperModel::Tiny,
        WhisperModel::Base,
        WhisperModel::Small,
        WhisperModel::Medium,
        WhisperModel::Large,
    ];

    let models: Vec<ModelInfo> = all_models
        .iter()
        .map(|m| ModelInfo {
            id: format!("{:?}", m).to_lowercase(),
            name: m.name().to_string(),
            size_mb: m.size_mb(),
            downloaded: engine.is_model_downloaded(*m),
        })
        .collect();

    // Get current model from settings (default to Small)
    let current_model = state
        .settings
        .read()
        .await
        .whisper_model
        .clone()
        .unwrap_or_else(|| "small".to_string());

    Ok(ModelsResponse {
        success: true,
        models,
        current_model,
        error: None,
    })
}

/// Download a Whisper model
#[tauri::command]
pub async fn download_model(
    state: State<'_, AppState>,
    window: tauri::Window,
    model_id: String,
) -> Result<crate::commands::audio::RecordingResponse, String> {
    let model = model_id
        .parse::<WhisperModel>()
        .map_err(|_| format!("Unknown model: {}", model_id))?;

    let engine = state.whisper_engine.lock().await;

    // Progress callback that emits events to frontend
    let window_clone = window.clone();
    let model_name = model.name().to_string();

    let progress_callback = move |downloaded: u64, total: u64| {
        let percent = if total > 0 {
            (downloaded as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        let _ = window_clone.emit(
            "download-progress",
            DownloadProgress {
                model: model_name.clone(),
                downloaded,
                total,
                percent,
            },
        );
    };

    match engine.download_model(model, Some(progress_callback)).await {
        Ok(_path) => {
            log::info!("Model {} downloaded successfully", model_id);
            Ok(crate::commands::audio::RecordingResponse {
                success: true,
                error: None,
            })
        }
        Err(e) => {
            log::error!("Failed to download model {}: {}", model_id, e);
            Ok(crate::commands::audio::RecordingResponse {
                success: false,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Set the active Whisper model
#[tauri::command]
pub async fn set_model(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<crate::commands::audio::RecordingResponse, String> {
    // Validate model ID
    if model_id.parse::<WhisperModel>().is_err() {
        return Ok(crate::commands::audio::RecordingResponse {
            success: false,
            error: Some(format!("Unknown model: {}", model_id)),
        });
    }

    // Update settings
    {
        let mut settings = state.settings.write().await;
        settings.whisper_model = Some(model_id.clone());
    }

    if let Err(error) = state.save_settings().await {
        return Ok(crate::commands::audio::RecordingResponse {
            success: false,
            error: Some(error.to_string()),
        });
    }

    log::info!("Set active model to: {}", model_id);

    Ok(crate::commands::audio::RecordingResponse {
        success: true,
        error: None,
    })
}

/// Internal transcription payload used by multiple entry paths.
#[derive(Debug)]
struct CloudRewriteConfig {
    model: String,
    api_key: String,
    url: String,
    timeout_ms: u64,
}

/// Transcribe the last captured audio
#[tauri::command]
pub async fn transcribe(state: State<'_, AppState>) -> Result<TranscriptionResponse, String> {
    transcribe_captured_audio(&state).await
}

pub async fn transcribe_captured_audio(state: &AppState) -> Result<TranscriptionResponse, String> {
    let start = Instant::now();

    // Take captured audio (moves it out, freeing the memory after transcription)
    let audio = {
        let mut audio = state.captured_audio.lock().await;
        audio.take()
    };

    let audio = match audio {
        Some(a) => a,
        None => {
            return Ok(TranscriptionResponse {
                success: false,
                text: None,
                raw_text: None,
                rewritten_text: None,
                rewrite_error: None,
                used_cloud_rewrite: false,
                segments: None,
                processing_ms: 0,
                error: Some("No audio captured. Record first.".to_string()),
            });
        }
    };

    let model_id = {
        let settings = state.settings.read().await;
        settings
            .whisper_model
            .clone()
            .unwrap_or_else(|| "small".to_string())
    };

    let model = model_id
        .parse::<WhisperModel>()
        .unwrap_or(WhisperModel::Small);

    // Check if model is downloaded
    {
        let engine = state.whisper_engine.lock().await;
        if !engine.is_model_downloaded(model) {
            return Ok(TranscriptionResponse {
                success: false,
                text: None,
                raw_text: None,
                rewritten_text: None,
                rewrite_error: None,
                used_cloud_rewrite: false,
                segments: None,
                processing_ms: 0,
                error: Some(format!(
                    "Model '{}' not downloaded. Please download it first.",
                    model.name()
                )),
            });
        }
    }

    // Run local transcription
    let engine = state.whisper_engine.lock().await;
    let local_result = match engine.transcribe(&audio.samples, model).await {
        Ok(result) => result,
        Err(e) => {
            return Ok(TranscriptionResponse {
                success: false,
                text: None,
                raw_text: None,
                rewritten_text: None,
                rewrite_error: None,
                used_cloud_rewrite: false,
                segments: None,
                processing_ms: start.elapsed().as_millis() as u64,
                error: Some(e.to_string()),
            })
        }
    };

    let segments: Vec<SegmentInfo> = local_result
        .segments
        .iter()
        .map(|s| SegmentInfo {
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            text: s.text.clone(),
        })
        .collect();

    let mut final_text = local_result.text.clone();
    let mut raw_text = Some(local_result.text.clone());
    let mut rewritten_text = None;
    let mut rewrite_error = None;
    let mut used_cloud_rewrite = false;

    let should_rewrite = {
        let settings = state.settings.read().await;
        settings.cloud_rewrite_enabled
    };

    if should_rewrite {
        match run_cloud_rewrite(state, &local_result.text).await {
            Ok((rewritten, raw)) => {
                rewritten_text = Some(rewritten.clone());
                raw_text = Some(raw);
                final_text = rewritten;
                used_cloud_rewrite = true;
            }
            Err(error) => {
                rewrite_error = Some(error);
            }
        }
    }

    Ok(TranscriptionResponse {
        success: true,
        text: Some(final_text),
        raw_text,
        rewritten_text,
        rewrite_error,
        used_cloud_rewrite,
        segments: Some(segments),
        error: None,
        processing_ms: start.elapsed().as_millis() as u64,
    })
}

/// Transcribe and return immediately (for quick preview)
#[tauri::command]
pub async fn transcribe_preview(state: State<'_, AppState>) -> Result<String, String> {
    let response = transcribe(state).await?;

    if response.success {
        Ok(response.text.unwrap_or_default())
    } else {
        Err(response
            .error
            .unwrap_or_else(|| "Unknown error".to_string()))
    }
}

fn build_rewrite_request(
    clipboard: Option<String>,
    profile: &WritingProfile,
    text: &str,
) -> String {
    let mut prompt = String::new();

    if let Some(previous_clipboard) = clipboard {
        if !previous_clipboard.trim().is_empty() {
            prompt.push_str("Context (last copied text):\n");
            prompt.push_str(&previous_clipboard);
            prompt.push('\n');
            prompt.push('\n');
        }
    }

    prompt.push_str("Rewrite the following transcription for readability and polish:\n");
    prompt.push_str("- Keep all original meaning.\n");
    prompt.push_str("- Preserve names, numbers, and action items.\n");
    prompt.push_str(&format!("- Tone: {}\n", profile.tone));
    prompt.push_str(&format!("- Purpose: {}\n", profile.purpose));
    prompt.push_str(&format!("- Format: {}\n", profile.format));
    prompt.push('\n');
    prompt.push_str("Input:\n");
    prompt.push_str(text);

    prompt
}

async fn run_cloud_rewrite(state: &AppState, transcript: &str) -> Result<(String, String), String> {
    let clipboard = state.take_recording_clipboard().await;
    let settings = state.settings.read().await;
    let profile = settings.writing_profile.clone();
    let rewrite_settings = settings.cloud_rewrite.clone();
    drop(settings);

    let config = cloud_rewrite_config(&rewrite_settings)?;

    let request_body = serde_json::json!({
        "model": config.model,
        "messages": [
            {
                "role": "system",
                "content": "You are a concise writing assistant for improving transcribed speech."
            },
            {
                "role": "user",
                "content": build_rewrite_request(clipboard.clone(), &profile, transcript)
            }
        ],
        "temperature": 0.3
    });

    let client = reqwest::Client::new();
    let call = async {
        let response = client
            .post(&config.url)
            .bearer_auth(&config.api_key)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Cloud rewrite request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("Cloud rewrite failed with status {}", status));
        }

        #[derive(Deserialize)]
        struct CloudResponse {
            choices: Vec<CloudChoice>,
        }
        #[derive(Deserialize)]
        struct CloudChoice {
            message: CloudMessage,
        }
        #[derive(Deserialize)]
        struct CloudMessage {
            content: Option<String>,
        }

        let payload: CloudResponse = response
            .json()
            .await
            .map_err(|e| format!("Cloud rewrite response parse failed: {}", e))?;

        let content = payload
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .ok_or_else(|| "Cloud rewrite response missing content".to_string())?;

        Ok::<String, String>(content.trim().to_string())
    };

    let rewritten = match timeout(Duration::from_millis(config.timeout_ms), call).await {
        Ok(result) => result?,
        Err(_) => return Err("Cloud rewrite timed out".to_string()),
    };

    if rewritten.trim().is_empty() {
        return Err("Cloud rewrite returned empty text".to_string());
    }

    Ok((rewritten, transcript.to_string()))
}

fn cloud_rewrite_config(
    settings: &crate::state::CloudRewriteSettings,
) -> Result<CloudRewriteConfig, String> {
    let api_key = if settings.api_key.trim().is_empty() {
        std::env::var("ZANA_REWRITE_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| {
                "Cloud rewrite enabled but no rewrite API key is configured".to_string()
            })?
    } else {
        settings.api_key.clone()
    };

    let url = if settings.api_url.trim().is_empty() {
        std::env::var("ZANA_REWRITE_API_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".to_string())
    } else {
        settings.api_url.clone()
    };

    if !url.starts_with("https://") {
        return Err("Cloud rewrite URL must use https".to_string());
    }

    let model = if settings.model.trim().is_empty() {
        std::env::var("ZANA_REWRITE_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string())
    } else {
        settings.model.clone()
    };

    let timeout_ms = if settings.timeout_ms == 0 {
        std::env::var("ZANA_REWRITE_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(15_000)
    } else {
        settings.timeout_ms
    };

    Ok(CloudRewriteConfig {
        model,
        api_key,
        url,
        timeout_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::{build_rewrite_request, cloud_rewrite_config};
    use crate::state::{CloudRewriteSettings, WritingProfile};

    #[test]
    fn build_rewrite_request_includes_clipboard_and_profile() {
        let profile = WritingProfile {
            purpose: "send a follow-up email".to_string(),
            tone: "warm".to_string(),
            format: "two sentences".to_string(),
        };

        let prompt = build_rewrite_request(
            Some("team status: launch next week".to_string()),
            &profile,
            "hey we shipped feature x yesterday",
        );

        assert!(prompt.contains("Context (last copied text):"));
        assert!(prompt.contains("team status: launch next week"));
        assert!(prompt.contains("Tone: warm"));
        assert!(prompt.contains("Purpose: send a follow-up email"));
        assert!(prompt.contains("Format: two sentences"));
        assert!(prompt.contains("Input:"));
        assert!(prompt.contains("hey we shipped feature x yesterday"));
    }

    #[test]
    fn cloud_rewrite_config_uses_saved_provider_settings() {
        let settings = CloudRewriteSettings {
            api_key: "sk-test".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_url: "https://example.com/v1/chat/completions".to_string(),
            timeout_ms: 12_000,
        };

        let config = cloud_rewrite_config(&settings).expect("saved provider config should load");

        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.url, "https://example.com/v1/chat/completions");
        assert_eq!(config.timeout_ms, 12_000);
    }

    #[test]
    fn cloud_rewrite_config_rejects_non_https_saved_url() {
        let settings = CloudRewriteSettings {
            api_key: "sk-test".to_string(),
            api_url: "http://example.com/v1/chat/completions".to_string(),
            ..CloudRewriteSettings::default()
        };

        let error = cloud_rewrite_config(&settings).unwrap_err();

        assert!(error.contains("https"));
    }
}
