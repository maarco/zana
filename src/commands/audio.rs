//! Audio Commands
//!
//! Tauri commands for audio capture and device management.

use crate::audio::{AudioCapture, AudioDevice, AudioMetrics};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

/// Response for list devices
#[derive(Debug, Serialize, Deserialize)]
pub struct DevicesResponse {
    pub success: bool,
    pub devices: Vec<AudioDevice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response for recording operations
#[derive(Debug, Serialize, Deserialize)]
pub struct RecordingResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response for stop recording
#[derive(Debug, Serialize, Deserialize)]
pub struct StopRecordingResponse {
    pub success: bool,
    pub duration_ms: u64,
    pub sample_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// List available audio input devices
#[tauri::command]
pub async fn list_audio_devices() -> Result<DevicesResponse, String> {
    match AudioCapture::list_devices() {
        Ok(devices) => Ok(DevicesResponse {
            success: true,
            devices,
            error: None,
        }),
        Err(e) => Ok(DevicesResponse {
            success: false,
            devices: vec![],
            error: Some(e.to_string()),
        }),
    }
}

/// Start audio recording
#[tauri::command]
pub async fn start_recording(
    state: State<'_, AppState>,
    device_id: Option<String>,
) -> Result<RecordingResponse, String> {
    let capture = state.audio_capture.lock().await;

    match capture.start(device_id.as_deref()).await {
        Ok(()) => Ok(RecordingResponse {
            success: true,
            error: None,
        }),
        Err(e) => Ok(RecordingResponse {
            success: false,
            error: Some(e.to_string()),
        }),
    }
}

/// Stop audio recording
#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> Result<StopRecordingResponse, String> {
    let capture = state.audio_capture.lock().await;

    match capture.stop().await {
        Ok(audio) => {
            // Store captured audio for transcription
            *state.captured_audio.lock().await = Some(audio.clone());

            Ok(StopRecordingResponse {
                success: true,
                duration_ms: audio.duration_ms,
                sample_count: audio.samples.len(),
                error: None,
            })
        }
        Err(e) => Ok(StopRecordingResponse {
            success: false,
            duration_ms: 0,
            sample_count: 0,
            error: Some(e.to_string()),
        }),
    }
}

/// Get current audio metrics (for visualization)
#[tauri::command]
pub async fn get_audio_metrics(state: State<'_, AppState>) -> Result<AudioMetrics, String> {
    let capture = state.audio_capture.lock().await;
    Ok(capture.get_metrics().await)
}

/// Check if recording is in progress
#[tauri::command]
pub async fn is_recording(state: State<'_, AppState>) -> Result<bool, String> {
    let capture = state.audio_capture.lock().await;
    Ok(capture.is_recording())
}
