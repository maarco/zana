//! Transcription Commands
//!
//! Tauri commands for speech-to-text using Whisper.

use crate::commands::audio::RecordingResponse;
use crate::stt::WhisperModel;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

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
    pub segments: Option<Vec<SegmentInfo>>,
    pub processing_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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
) -> Result<RecordingResponse, String> {
    let model = model_id.parse::<WhisperModel>().map_err(|_| format!("Unknown model: {}", model_id))?;

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
            Ok(RecordingResponse {
                success: true,
                error: None,
            })
        }
        Err(e) => {
            log::error!("Failed to download model {}: {}", model_id, e);
            Ok(RecordingResponse {
                success: false,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Set the active Whisper model
#[tauri::command]
pub async fn set_model(state: State<'_, AppState>, model_id: String) -> Result<RecordingResponse, String> {
    // Validate model ID
    if model_id.parse::<WhisperModel>().is_err() {
        return Ok(RecordingResponse {
            success: false,
            error: Some(format!("Unknown model: {}", model_id)),
        });
    }

    // Update settings
    {
        let mut settings = state.settings.write().await;
        settings.whisper_model = Some(model_id.clone());
    }

    log::info!("Set active model to: {}", model_id);

    Ok(RecordingResponse {
        success: true,
        error: None,
    })
}

/// Transcribe the last recorded audio
#[tauri::command]
pub async fn transcribe(state: State<'_, AppState>) -> Result<TranscriptionResponse, String> {
    // Get captured audio
    let audio = {
        let audio = state.captured_audio.lock().await;
        audio.clone()
    };

    let audio = match audio {
        Some(a) => a,
        None => {
            return Ok(TranscriptionResponse {
                success: false,
                text: None,
                segments: None,
                processing_ms: 0,
                error: Some("No audio captured. Record first.".to_string()),
            });
        }
    };

    // Get model from settings
    let model_id = {
        let settings = state.settings.read().await;
        settings.whisper_model.clone().unwrap_or_else(|| "small".to_string())
    };

    let model = model_id.parse::<WhisperModel>()
        .ok()
        .unwrap_or(WhisperModel::Small);

    // Check if model is downloaded
    {
        let engine = state.whisper_engine.lock().await;
        if !engine.is_model_downloaded(model) {
            return Ok(TranscriptionResponse {
                success: false,
                text: None,
                segments: None,
                processing_ms: 0,
                error: Some(format!(
                    "Model '{}' not downloaded. Please download it first.",
                    model.name()
                )),
            });
        }
    }

    // Run transcription
    let engine = state.whisper_engine.lock().await;

    match engine.transcribe(&audio.samples, model).await {
        Ok(result) => {
            let segments: Vec<SegmentInfo> = result
                .segments
                .iter()
                .map(|s| SegmentInfo {
                    start_ms: s.start_ms,
                    end_ms: s.end_ms,
                    text: s.text.clone(),
                })
                .collect();

            Ok(TranscriptionResponse {
                success: true,
                text: Some(result.text),
                segments: Some(segments),
                processing_ms: result.processing_ms,
                error: None,
            })
        }
        Err(e) => Ok(TranscriptionResponse {
            success: false,
            text: None,
            segments: None,
            processing_ms: 0,
            error: Some(e.to_string()),
        }),
    }
}

/// Transcribe and return immediately (for quick preview)
#[tauri::command]
pub async fn transcribe_preview(state: State<'_, AppState>) -> Result<String, String> {
    let response = transcribe(state).await?;

    if response.success {
        Ok(response.text.unwrap_or_default())
    } else {
        Err(response.error.unwrap_or_else(|| "Unknown error".to_string()))
    }
}
