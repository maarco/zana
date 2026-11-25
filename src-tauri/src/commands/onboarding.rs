//! Onboarding Commands
//!
//! Tauri commands for first-run onboarding flow.

use crate::onboarding;
use serde::{Deserialize, Serialize};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

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

/// Check Screen Recording permission without prompting.
#[tauri::command]
pub fn check_screen_recording_permission() -> bool {
    onboarding::check_screen_recording()
}

/// Check Automation permission for the System Events actions used by Voicebot without prompting.
#[tauri::command]
pub fn check_automation_permission() -> bool {
    onboarding::check_automation()
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

/// Open System Settings to the Screen Recording pane.
#[tauri::command]
pub fn open_screen_recording_settings() {
    log::info!("[Onboarding] open_screen_recording_settings command called");
    onboarding::open_screen_recording_settings();
    log::info!("[Onboarding] open_screen_recording_settings completed");
}

/// Open System Settings to the Automation pane.
#[tauri::command]
pub fn open_automation_settings() {
    log::info!("[Onboarding] open_automation_settings command called");
    onboarding::open_automation_settings();
    log::info!("[Onboarding] open_automation_settings completed");
}

/// Open the macOS Microphone privacy settings pane.
#[tauri::command]
pub fn open_microphone_settings() {
    onboarding::open_microphone_settings();
}

/// Open Voicebot Preferences from the onboarding flow so the user can choose a chat model,
/// configure its API key, and set an optional wake word.
#[tauri::command]
pub fn open_preferences_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("preferences") {
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        "preferences",
        WebviewUrl::App("preferences.html".into()),
    )
    .title("Zana Preferences")
    .decorations(false)
    .transparent(true)
    .shadow(true)
    .inner_size(500.0, 680.0)
    .center()
    .resizable(false)
    .build()
    .map(|_| ())
    .map_err(|error| error.to_string())
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

/// Complete onboarding and restart into normal Voicebot startup.
///
/// Marks onboarding as complete, then restarts the application so the first-run gate is no longer
/// active and the Voicebot session can answer the user's first question.
#[tauri::command]
pub fn complete_onboarding_and_restart(
    app: tauri::AppHandle,
) -> Result<OnboardingResponse, String> {
    log::info!("Onboarding complete - restarting into Voicebot startup");

    // Mark onboarding complete
    if let Err(e) = onboarding::mark_onboarding_complete() {
        log::error!("Failed to mark onboarding complete: {}", e);
        return Ok(OnboardingResponse {
            success: false,
            error: Some(e.to_string()),
        });
    }

    // Give the invoke response time to reach the onboarding window before relaunching.
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        app.restart();
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
}
