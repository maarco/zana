//! Zana Main Entry Point
#![allow(dead_code)]
#![allow(unused_imports)]
//!
//! Cross-platform speech-to-text with beautiful GPU visualizations.
//! Built with egui + wgpu for pure Rust performance.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Core modules
mod audio;
mod errors;
mod fn_key_monitor;
mod gui;
mod hooks;
mod plugins;
mod state;
mod stt;

// std imports for platform detection
use std::env;

fn main() -> eframe::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("Starting Zana v0.1.0");
    log::info!("Platform: {} {}", env::consts::OS, env::consts::ARCH);
    log::debug!("Debug logging enabled");

    // Window options - floating overlay (always on top, no decorations, starts hidden)
    let window_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([300.0, 300.0])
            .with_min_inner_size([200.0, 200.0])
            .with_title("Zana")
            .with_always_on_top()
            .with_decorations(false)  // No title bar
            .with_visible(false),     // Start hidden, show on Fn key
        ..Default::default()
    };

    log::info!("Initializing main window");

    // Run the app
    let result = eframe::run_native(
        "Zana",
        window_options,
        Box::new(|cc| {
            // Configure egui with dark visuals
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            log::trace!("egui context configured with dark visuals");

            // Create the app (it creates its own tokio runtime)
            log::info!("Creating ZanaApp instance");
            Ok(Box::new(gui::ZanaApp::new(cc)))
        }),
    );

    // Log shutdown
    match &result {
        Ok(_) => log::info!("Zana shut down successfully"),
        Err(e) => log::error!("Zana shut down with error: {}", e),
    }

    result
}
