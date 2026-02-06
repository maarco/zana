//! Onboarding Commands
//!
//! Tauri commands for first-run onboarding flow.

use crate::onboarding;
use crate::stt::WhisperModel;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State, Window};

/// Download progress event for onboarding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingDownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub percent: f32,
}

/// Response for onboarding operations
#[derive(Debug, Serialize, Deserialize)]
pub struct OnboardingResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Check if this is the first run of the application
///
/// Returns `true` if onboarding has not been completed yet.
#[tauri::command]
pub fn is_first_run() -> bool {
    onboarding::is_first_run()
}

/// Check accessibility permission status
///
/// On macOS, checks if the app has accessibility permissions.
/// On other platforms, always returns `true`.
#[tauri::command]
pub fn check_accessibility_permission() -> bool {
    onboarding::check_accessibility()
}

/// Open System Settings to Accessibility pane (macOS only)
///
/// On macOS, opens System Settings directly to the Accessibility pane.
/// On other platforms, does nothing.
#[tauri::command]
pub fn open_accessibility_settings() {
    log::info!("[Onboarding] open_accessibility_settings command called");
    onboarding::open_accessibility_settings();
    log::info!("[Onboarding] open_accessibility_settings completed");
}

/// Download the default Whisper model for onboarding
///
/// Downloads the Small English model (ggml-small.en.bin) with progress updates.
/// Emits `download-progress` events during download.
/// Returns success if model is already cached or download completes.
#[tauri::command]
pub async fn download_whisper_model(
    state: State<'_, AppState>,
    window: Window,
) -> Result<OnboardingResponse, String> {
    let engine = state.whisper_engine.lock().await;

    // Use Small model for onboarding (good balance of speed/accuracy)
    let model = WhisperModel::Small;

    // Check if already downloaded
    if engine.is_model_downloaded(model) {
        log::info!("Whisper model already downloaded, skipping");
        // Emit 100% progress so frontend knows download is complete
        let _ = window.emit(
            "download-progress",
            OnboardingDownloadProgress {
                downloaded: 100,
                total: 100,
                percent: 100.0,
            },
        );
        return Ok(OnboardingResponse {
            success: true,
            error: None,
        });
    }

    // Progress callback that emits events to frontend
    let window_clone = window.clone();
    let progress_callback = move |downloaded: u64, total: u64| {
        let percent = if total > 0 {
            (downloaded as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        let _ = window_clone.emit(
            "download-progress",
            OnboardingDownloadProgress {
                downloaded,
                total,
                percent,
            },
        );
    };

    // Download the model
    match engine
        .download_model(model, Some(progress_callback))
        .await
    {
        Ok(_path) => {
            log::info!("Whisper model downloaded successfully for onboarding");
            Ok(OnboardingResponse {
                success: true,
                error: None,
            })
        }
        Err(e) => {
            log::error!("Failed to download Whisper model for onboarding: {}", e);
            Ok(OnboardingResponse {
                success: false,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Mark onboarding as complete
///
/// Creates the onboarding completion marker file.
#[tauri::command]
pub fn mark_onboarding_complete() -> Result<OnboardingResponse, String> {
    match onboarding::mark_onboarding_complete() {
        Ok(_) => {
            log::info!("Onboarding marked as complete");
            Ok(OnboardingResponse {
                success: true,
                error: None,
            })
        }
        Err(e) => {
            log::error!("Failed to mark onboarding complete: {}", e);
            Ok(OnboardingResponse {
                success: false,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Complete onboarding and exit app
///
/// Marks onboarding as complete and exits the application.
/// User should restart the app to use kVoice normally.
#[tauri::command]
pub fn complete_onboarding_and_exit(app: tauri::AppHandle) -> Result<OnboardingResponse, String> {
    log::info!("Onboarding complete - exiting application");

    // Mark onboarding complete
    if let Err(e) = onboarding::mark_onboarding_complete() {
        log::error!("Failed to mark onboarding complete: {}", e);
        return Ok(OnboardingResponse {
            success: false,
            error: Some(e.to_string()),
        });
    }

    // Exit the app - user will restart to use kVoice
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        app.exit(0);
    });

    Ok(OnboardingResponse {
        success: true,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_onboarding_response_serialization() {
        let response = OnboardingResponse {
            success: true,
            error: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
    }

    #[test]
    fn test_download_progress_serialization() {
        let progress = OnboardingDownloadProgress {
            downloaded: 100,
            total: 1000,
            percent: 10.0,
        };
        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"downloaded\":100"));
        assert!(json.contains("\"percent\":10.0"));
    }
}
