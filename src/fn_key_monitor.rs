//! macOS Fn Key Monitor
//!
//! Detects press/release of the Fn/Globe key using native NSEvent APIs.
//! Adapted from kollabor-app-v1 for use with egui/eframe.

#![allow(unexpected_cfgs)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

/// Track if Fn key is currently pressed (atomic for thread safety)
static FN_KEY_PRESSED: AtomicBool = AtomicBool::new(false);

/// Fn key keyCode on macOS
#[cfg(target_os = "macos")]
const FN_KEY_CODE: u16 = 63;

/// Events from the Fn key monitor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FnKeyEvent {
    Pressed,
    Released,
}

/// Accessibility permission status
#[derive(Debug, Clone, PartialEq)]
pub enum AccessibilityStatus {
    Granted,
    Denied,
    Unknown,
}

/// Check if Accessibility permissions are granted (macOS only)
#[cfg(target_os = "macos")]
pub fn check_accessibility_permissions() -> AccessibilityStatus {
    unsafe {
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }

        let trusted = AXIsProcessTrusted();

        if trusted {
            log::info!("Accessibility permissions are granted");
            AccessibilityStatus::Granted
        } else {
            log::warn!("Accessibility permissions are NOT granted");
            AccessibilityStatus::Denied
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn check_accessibility_permissions() -> AccessibilityStatus {
    AccessibilityStatus::Granted
}

/// Open System Settings to Accessibility permissions pane (macOS only)
#[cfg(target_os = "macos")]
pub fn open_accessibility_settings() -> Result<(), String> {
    use std::process::Command;

    log::info!("Opening System Settings to Accessibility permissions");

    let result = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();

    match result {
        Ok(_) => {
            log::info!("System Settings opened");
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to open System Settings: {}", e);
            log::error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn open_accessibility_settings() -> Result<(), String> {
    Ok(())
}

/// Setup Fn key monitoring and return a receiver for events
///
/// Returns a channel receiver that will receive FnKeyEvent::Pressed and FnKeyEvent::Released
/// events when the Fn key is pressed/released.
#[cfg(target_os = "macos")]
pub fn setup_fn_key_monitor() -> Result<mpsc::Receiver<FnKeyEvent>, String> {
    use cocoa::base::{id, nil};
    use objc::rc::autoreleasepool;
    use objc::{class, msg_send, sel, sel_impl};

    log::info!("Setting up Fn key monitor using NSEvent");

    // Check permissions first
    let permission_status = check_accessibility_permissions();
    match permission_status {
        AccessibilityStatus::Granted => {
            log::info!("Accessibility permissions verified");
        }
        AccessibilityStatus::Denied => {
            let error_msg = "Accessibility permissions required. Please grant in System Settings > Privacy & Security > Accessibility".to_string();
            log::error!("{}", error_msg);
            return Err(error_msg);
        }
        AccessibilityStatus::Unknown => {
            log::warn!("Unable to verify permissions, proceeding anyway");
        }
    }

    let (tx, rx) = mpsc::channel();
    let tx_global = tx.clone();
    let tx_local = tx;

    // NSFlagsChangedMask = 1 << 12 = 4096
    let flags_changed_mask: u64 = 1 << 12;

    unsafe {
        autoreleasepool(|| {
            // Create global monitor block
            let block = block::ConcreteBlock::new(move |event: id| {
                let key_code: u16 = msg_send![event, keyCode];
                let flags: u64 = msg_send![event, modifierFlags];
                // NSEventModifierFlagFunction = 1 << 23 = 8388608
                let fn_flag: u64 = 1 << 23;
                let fn_is_pressed = (flags & fn_flag) != 0;

                let was_pressed = FN_KEY_PRESSED.load(Ordering::SeqCst);

                // Detect Fn key PRESS: flag goes from false to true
                if fn_is_pressed && !was_pressed && key_code == FN_KEY_CODE {
                    FN_KEY_PRESSED.store(true, Ordering::SeqCst);
                    log::info!("Fn key PRESSED (keyCode {}, flags {:x})", key_code, flags);
                    let _ = tx_global.send(FnKeyEvent::Pressed);
                }
                // Detect Fn key RELEASE: flag goes from true to false
                else if !fn_is_pressed && was_pressed {
                    FN_KEY_PRESSED.store(false, Ordering::SeqCst);
                    log::info!("Fn key RELEASED (keyCode {}, flags {:x})", key_code, flags);
                    let _ = tx_global.send(FnKeyEvent::Released);
                }
            });

            let block = block.copy();

            // Add GLOBAL monitor (when app is NOT focused)
            let global_monitor: id = msg_send![
                class!(NSEvent),
                addGlobalMonitorForEventsMatchingMask:flags_changed_mask
                handler:block
            ];

            if global_monitor == nil {
                log::error!("Failed to create global NSEvent monitor");
            } else {
                std::mem::forget(global_monitor); // Keep monitor alive
                log::info!("Global Fn key monitor active");
            }

            // Create LOCAL monitor block
            let block_local = block::ConcreteBlock::new(move |event: id| -> id {
                let key_code: u16 = msg_send![event, keyCode];
                let flags: u64 = msg_send![event, modifierFlags];
                let fn_flag: u64 = 1 << 23;
                let fn_is_pressed = (flags & fn_flag) != 0;
                let was_pressed = FN_KEY_PRESSED.load(Ordering::SeqCst);

                if fn_is_pressed && !was_pressed && key_code == FN_KEY_CODE {
                    FN_KEY_PRESSED.store(true, Ordering::SeqCst);
                    log::info!("Fn key PRESSED [LOCAL] (keyCode {}, flags {:x})", key_code, flags);
                    let _ = tx_local.send(FnKeyEvent::Pressed);
                } else if !fn_is_pressed && was_pressed {
                    FN_KEY_PRESSED.store(false, Ordering::SeqCst);
                    log::info!("Fn key RELEASED [LOCAL] (keyCode {}, flags {:x})", key_code, flags);
                    let _ = tx_local.send(FnKeyEvent::Released);
                }

                event // Return event unmodified
            });

            let block_local = block_local.copy();

            // Add LOCAL monitor (when app IS focused)
            let local_monitor: id = msg_send![
                class!(NSEvent),
                addLocalMonitorForEventsMatchingMask:flags_changed_mask
                handler:block_local
            ];

            if local_monitor == nil {
                log::error!("Failed to create local NSEvent monitor");
            } else {
                std::mem::forget(local_monitor); // Keep monitor alive
                log::info!("Local Fn key monitor active");
            }

            log::info!("NSEvent Fn key monitors active (keyCode {})", FN_KEY_CODE);
        });
    }

    Ok(rx)
}

#[cfg(not(target_os = "macos"))]
pub fn setup_fn_key_monitor() -> Result<mpsc::Receiver<FnKeyEvent>, String> {
    log::info!("Fn key monitoring is only available on macOS");
    let (_tx, rx) = mpsc::channel();
    Ok(rx) // Return empty channel on non-macOS
}

/// Paste text to the currently focused input field
///
/// Saves clipboard, sets text, simulates Cmd+V, restores clipboard.
#[cfg(target_os = "macos")]
pub fn paste_text(text: &str) -> Result<(), String> {
    use arboard::Clipboard;
    use enigo::{Enigo, Key, Keyboard, Settings};

    log::info!("Pasting {} chars to active input", text.len());

    // Save original clipboard
    let original = {
        let mut clipboard = Clipboard::new()
            .map_err(|e| format!("Clipboard init failed: {}", e))?;
        clipboard.get_text().ok()
    };

    // Write text to clipboard
    {
        let mut clipboard = Clipboard::new()
            .map_err(|e| format!("Clipboard init failed: {}", e))?;
        clipboard.set_text(text)
            .map_err(|e| format!("Clipboard write failed: {}", e))?;
    }

    // Small delay for clipboard to propagate
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Simulate Cmd+V
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Enigo init failed: {}", e))?;

    enigo.key(Key::Meta, enigo::Direction::Press)
        .map_err(|e| format!("Key press failed: {}", e))?;
    enigo.key(Key::Unicode('v'), enigo::Direction::Click)
        .map_err(|e| format!("Key click failed: {}", e))?;
    enigo.key(Key::Meta, enigo::Direction::Release)
        .map_err(|e| format!("Key release failed: {}", e))?;

    // Restore original clipboard after delay (in background thread)
    if let Some(original_text) = original {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Ok(mut clipboard) = Clipboard::new() {
                let _ = clipboard.set_text(&original_text);
            }
        });
    }

    log::info!("Paste operation completed");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn paste_text(text: &str) -> Result<(), String> {
    use arboard::Clipboard;
    use enigo::{Enigo, Key, Keyboard, Settings};

    // Save original clipboard
    let original = {
        let mut clipboard = Clipboard::new()
            .map_err(|e| format!("Clipboard init failed: {}", e))?;
        clipboard.get_text().ok()
    };

    // Write text to clipboard
    {
        let mut clipboard = Clipboard::new()
            .map_err(|e| format!("Clipboard init failed: {}", e))?;
        clipboard.set_text(text)
            .map_err(|e| format!("Clipboard write failed: {}", e))?;
    }

    std::thread::sleep(std::time::Duration::from_millis(50));

    // Simulate Ctrl+V on non-macOS
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Enigo init failed: {}", e))?;

    enigo.key(Key::Control, enigo::Direction::Press)
        .map_err(|e| format!("Key press failed: {}", e))?;
    enigo.key(Key::Unicode('v'), enigo::Direction::Click)
        .map_err(|e| format!("Key click failed: {}", e))?;
    enigo.key(Key::Control, enigo::Direction::Release)
        .map_err(|e| format!("Key release failed: {}", e))?;

    // Restore original clipboard
    if let Some(original_text) = original {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if let Ok(mut clipboard) = Clipboard::new() {
                let _ = clipboard.set_text(&original_text);
            }
        });
    }

    Ok(())
}

/// Check if Fn key is currently pressed
pub fn is_fn_key_pressed() -> bool {
    FN_KEY_PRESSED.load(Ordering::SeqCst)
}

/// Configure window to overlay fullscreen apps on macOS
/// Must be called after the window is created and visible
#[cfg(target_os = "macos")]
pub fn configure_overlay_window() {
    use cocoa::base::nil;
    use objc::{class, msg_send, sel, sel_impl};

    log::info!("Configuring window for fullscreen overlay...");

    unsafe {
        // Get the shared NSApplication
        let ns_app: cocoa::base::id = msg_send![class!(NSApplication), sharedApplication];
        if ns_app == nil {
            log::error!("Failed to get NSApplication");
            return;
        }

        // Get all windows
        let windows: cocoa::base::id = msg_send![ns_app, windows];
        let count: usize = msg_send![windows, count];

        log::info!("Found {} windows", count);

        for i in 0..count {
            let window: cocoa::base::id = msg_send![windows, objectAtIndex: i];
            if window == nil {
                continue;
            }

            // Set window level to 1000 (same as kollabor - above fullscreen)
            let level: i64 = 1000;
            let _: () = msg_send![window, setLevel: level];

            // NSWindowCollectionBehavior flags:
            // MoveToActiveSpace = 1 << 1 = 2
            // FullScreenAuxiliary = 1 << 8 = 256
            // Combined: allows panel to appear alongside fullscreen apps
            let collection_behavior: u64 = (1 << 1) | (1 << 8); // MoveToActiveSpace | FullScreenAuxiliary
            let _: () = msg_send![window, setCollectionBehavior: collection_behavior];

            // Don't hide when app loses focus
            let _: () = msg_send![window, setHidesOnDeactivate: false];

            // Bring to front without stealing focus
            let _: () = msg_send![window, orderFrontRegardless];

            log::info!("Window configured: level=1000, FullScreenAuxiliary enabled");
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn configure_overlay_window() {
    // No-op on other platforms
}
