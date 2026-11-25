//! Diagnostics Commands
//!
//! Health check and troubleshooting commands.

use crate::onboarding;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

/// Diagnostic check result
#[derive(Debug, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub name: String,
    pub status: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_action: Option<String>,
}

/// Run full diagnostics
///
/// Checks accessibility, Voicebot and platform state.
#[tauri::command]
pub async fn run_diagnostics(
    _state: State<'_, AppState>,
) -> Result<HashMap<String, DiagnosticCheck>, String> {
    let mut checks = HashMap::new();

    // 1. Accessibility (macOS)
    #[cfg(target_os = "macos")]
    {
        let has_accessibility = onboarding::check_accessibility();
        checks.insert(
            "accessibility".to_string(),
            DiagnosticCheck {
                name: "Accessibility Permissions".to_string(),
                status: if has_accessibility {
                    "ok".to_string()
                } else {
                    "error".to_string()
                },
                message: if has_accessibility {
                    "App has accessibility permissions".to_string()
                } else {
                    "Voicebot needs accessibility permission to type, paste and press keys"
                        .to_string()
                },
                fix: if !has_accessibility {
                    Some("Open System Settings".to_string())
                } else {
                    None
                },
                fix_action: if !has_accessibility {
                    Some("openAccessibilitySettings()".to_string())
                } else {
                    None
                },
            },
        );
    }

    #[cfg(not(target_os = "macos"))]
    {
        checks.insert(
            "accessibility".to_string(),
            DiagnosticCheck {
                name: "Accessibility Permissions".to_string(),
                status: "ok".to_string(),
                message: "Not required on this platform".to_string(),
                fix: None,
                fix_action: None,
            },
        );
    }

    // 2. Voicebot
    let voicebot = crate::voicebot::status();
    checks.insert(
        "voicebot".to_string(),
        DiagnosticCheck {
            name: "Voicebot".to_string(),
            status: match voicebot.stage.as_str() {
                "ready" => "ok",
                "error" | "off" => "error",
                _ => "warning",
            }
            .to_string(),
            message: voicebot.detail,
            fix: None,
            fix_action: None,
        },
    );

    // 3. Platform info
    checks.insert(
        "platform".to_string(),
        DiagnosticCheck {
            name: "Platform".to_string(),
            status: "ok".to_string(),
            message: format!("Running on {}", std::env::consts::OS),
            fix: None,
            fix_action: None,
        },
    );

    Ok(checks)
}
