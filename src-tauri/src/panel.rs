//! macOS NSPanel Definition
//!
//! NSPanel is required because NSWindow cannot appear above fullscreen apps.
//! Uses tauri-nspanel crate for the implementation.

#![allow(dead_code)]
#![allow(unused_doc_comments)]

#[cfg(target_os = "macos")]
use tauri::Manager;
#[cfg(target_os = "macos")]
use tauri_nspanel::tauri_panel;

/// Define OrbPanel as an NSPanel
/// - can_become_key_window: true allows the panel to receive keyboard input
/// - is_floating_panel: true makes it float above other windows
#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(OrbPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })
}
