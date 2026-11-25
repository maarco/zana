//! First-run onboarding detection and setup
//!
//! This module provides utilities for detecting first-run state,
//! managing onboarding completion markers, and checking system
//! accessibility permissions.

use std::fs;
use std::io;
use std::path::PathBuf;

pub const ACCESSIBILITY_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
pub const SCREEN_RECORDING_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
pub const AUTOMATION_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation";

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct AppleEventDescriptor {
    _descriptor_type: u32,
    _data_handle: *mut std::ffi::c_void,
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AECreateDesc(
        type_code: u32,
        data_ptr: *const std::ffi::c_void,
        data_size: std::os::raw::c_long,
        result: *mut AppleEventDescriptor,
    ) -> i16;
    fn AEDisposeDesc(desc: *mut AppleEventDescriptor) -> i16;
    fn AEDeterminePermissionToAutomateTarget(
        target: *const AppleEventDescriptor,
        event_class: u32,
        event_id: u32,
        ask_user_if_needed: u8,
    ) -> i32;
}

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

/// Check Screen Recording without prompting the user.
pub fn check_screen_recording() -> bool {
    #[cfg(target_os = "macos")]
    {
        // CGPreflightScreenCaptureAccess is deliberately non-prompting. A denied TCC entry
        // must be repaired from System Settings, not by repeatedly asking the user.
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Check whether Zana may send the representative Apple Event used by System Events.
///
/// The check does not prompt. The actual first typing, pasting, or AppleScript action still
/// goes through `osascript`, which is what lets macOS present its Automation consent prompt.
pub fn check_automation() -> bool {
    #[cfg(target_os = "macos")]
    {
        const TYPE_APPLICATION_BUNDLE_ID: u32 = u32::from_be_bytes(*b"bund");
        const K_AE_CORE_SUITE: u32 = u32::from_be_bytes(*b"core");
        const K_AE_GET_DATA: u32 = u32::from_be_bytes(*b"getd");
        let target_bundle_id = b"com.apple.systemevents";
        let mut target = AppleEventDescriptor {
            _descriptor_type: 0,
            _data_handle: std::ptr::null_mut(),
        };

        let create_status = unsafe {
            AECreateDesc(
                TYPE_APPLICATION_BUNDLE_ID,
                target_bundle_id.as_ptr().cast(),
                target_bundle_id.len() as std::os::raw::c_long,
                &mut target,
            )
        };
        if create_status != 0 {
            log::warn!(
                "[Onboarding] Could not create the System Events Apple Event target: status {}",
                create_status
            );
            return false;
        }

        // ask_user_if_needed = false keeps onboarding status checks from consuming the first
        // Automation prompt. The real osascript action remains the consent boundary.
        let status = unsafe {
            AEDeterminePermissionToAutomateTarget(&target, K_AE_CORE_SUITE, K_AE_GET_DATA, 0)
        };
        let _ = unsafe { AEDisposeDesc(&mut target) };
        status == 0
    }

    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open the platform's Microphone privacy settings pane where supported.
pub fn open_microphone_settings() {
    #[cfg(target_os = "macos")]
    {
        let url = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
        if let Err(error) = std::process::Command::new("open").arg(url).spawn() {
            log::error!("Failed to open Microphone privacy settings: {error}");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Err(error) = std::process::Command::new("cmd")
            .args(["/C", "start", "", "ms-settings:privacy-microphone"])
            .spawn()
        {
            log::error!("Failed to open Microphone privacy settings: {error}");
        }
    }
}

/// Open System Settings to the Accessibility pane (macOS only).
///
/// On macOS, opens System Settings directly to the Accessibility pane where users can grant
/// accessibility permissions. On other platforms, this does nothing.
pub fn open_accessibility_settings() {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        log::info!("[Onboarding] Executing open command for accessibility settings");

        // This URL scheme works across all recent macOS versions
        match Command::new("open")
            .arg(ACCESSIBILITY_SETTINGS_URL)
            .output()
        {
            Ok(output) => {
                log::info!(
                    "[Onboarding] Open command executed, status: {}",
                    output.status
                );
                if !output.stderr.is_empty() {
                    log::warn!(
                        "[Onboarding] stderr: {:?}",
                        String::from_utf8_lossy(&output.stderr)
                    );
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

/// Open System Settings to the Screen Recording pane (macOS only).
pub fn open_screen_recording_settings() {
    #[cfg(target_os = "macos")]
    {
        if let Err(error) = std::process::Command::new("open")
            .arg(SCREEN_RECORDING_SETTINGS_URL)
            .spawn()
        {
            log::error!("Failed to open Screen Recording privacy settings: {error}");
        }
    }
}

/// Open System Settings to the Automation pane (macOS only).
pub fn open_automation_settings() {
    #[cfg(target_os = "macos")]
    {
        if let Err(error) = std::process::Command::new("open")
            .arg(AUTOMATION_SETTINGS_URL)
            .spawn()
        {
            log::error!("Failed to open Automation privacy settings: {error}");
        }
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

    #[test]
    fn privacy_settings_urls_target_the_expected_panes() {
        assert!(ACCESSIBILITY_SETTINGS_URL.ends_with("Privacy_Accessibility"));
        assert!(SCREEN_RECORDING_SETTINGS_URL.ends_with("Privacy_ScreenCapture"));
        assert!(AUTOMATION_SETTINGS_URL.ends_with("Privacy_Automation"));
    }
}
