//! First-run onboarding detection and setup
//!
//! This module provides utilities for detecting first-run state,
//! managing onboarding completion markers, and checking system
//! accessibility permissions.

use std::fs;
use std::io;
use std::path::PathBuf;

/// Get the Zana config directory
fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| panic!("Failed to get config directory"))
        .join("Zana")
}

/// Get the onboarding completion marker file path
fn marker_path() -> PathBuf {
    config_dir().join(".onboarding_complete")
}

/// Check if this is the first run of the application
///
/// Returns `true` if the onboarding has not been completed yet,
/// `false` if the marker file exists.
pub fn is_first_run() -> bool {
    !marker_path().exists()
}

/// Mark onboarding as complete
///
/// Creates the config directory if it doesn't exist and writes
/// an empty marker file to indicate onboarding is complete.
pub fn mark_onboarding_complete() -> io::Result<()> {
    let config_dir = config_dir();

    // Create config directory if it doesn't exist
    fs::create_dir_all(&config_dir)?;

    // Write empty marker file
    fs::write(marker_path(), "")?;

    Ok(())
}

/// Check accessibility permissions on macOS
///
/// On macOS, wraps the `AXIsProcessTrusted` function to check if
/// the application has accessibility permissions.
/// On other platforms, always returns `true`.
pub fn check_accessibility() -> bool {
    #[cfg(target_os = "macos")]
    {
        unsafe {
            extern "C" {
                fn AXIsProcessTrusted() -> bool;
            }
            AXIsProcessTrusted()
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Other platforms don't require accessibility checks
        true
    }
}

/// Open System Settings to the Accessibility pane (macOS only)
///
/// On macOS, opens System Settings directly to the Accessibility
/// pane where users can grant accessibility permissions.
/// On other platforms, does nothing.
pub fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        log::info!("[Onboarding] Executing open command for accessibility settings");

        // This URL scheme works across all recent macOS versions
        match Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .output()
        {
            Ok(output) => {
                log::info!("[Onboarding] Open command executed, status: {}", output.status);
                if !output.stderr.is_empty() {
                    log::warn!("[Onboarding] stderr: {:?}", String::from_utf8_lossy(&output.stderr));
                }
            }
            Err(e) => {
                log::error!("[Onboarding] Failed to execute open command: {}", e);
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        // No-op on other platforms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dir() {
        let dir = config_dir();
        assert!(dir.ends_with("Zana"));
    }

    #[test]
    fn test_marker_path() {
        let marker = marker_path();
        assert!(marker.ends_with(".onboarding_complete"));
        assert!(marker.ends_with("Zana/.onboarding_complete"));
    }
}
