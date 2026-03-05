//! Zana - Cross-platform Speech-to-Text with Beautiful Visualizations
//!
//! Zana is a desktop application that provides:
//! - Local speech-to-text using whisper.cpp (no cloud required)
//! - Beautiful, customizable audio visualizations (GPU-accelerated)
//! - Extensible plugin system
//! - Cross-platform support (Windows, macOS, Linux)
//!
//! # Architecture
//!
//! Zana is built on a hook-based architecture that allows plugins to
//! intercept and respond to events throughout the application.
//!
//! ## Core Modules
//!
//! - [`hooks`]: Event system for extensibility
//! - [`plugins`]: Plugin loading and management
//! - [`stt`]: Speech-to-text engine (whisper.cpp)
//! - [`audio`]: Audio capture and processing
//! - [`commands`]: Tauri commands for frontend
//! - [`state`]: Application state management
//!
//! # Example
//!
//! ```rust,ignore
//! use Zana::hooks::EventBus;
//! use Zana::stt::WhisperEngine;
//! use Zana::audio::AudioCapture;
//! use std::sync::Arc;
//!
//! // Create event bus
//! let event_bus = Arc::new(EventBus::new());
//!
//! // Create audio capture
//! let capture = AudioCapture::new(event_bus.clone());
//!
//! // Start recording
//! capture.start(None).await?;
//!
//! // ... wait for user to finish speaking ...
//!
//! // Stop and get samples
//! let audio = capture.stop().await?;
//!
//! // Transcribe
//! let engine = WhisperEngine::new(event_bus.clone())?;
//! let result = engine.transcribe(&audio.samples, WhisperModel::Small).await?;
//! println!("Transcription: {}", result.text);
//! ```

pub mod audio;
pub mod commands;
pub mod hooks;
pub mod onboarding;
pub mod plugins;
pub mod state;
pub mod stt;

// Re-exports for convenience
pub use audio::{AudioCapture, AudioDevice, AudioMetrics, CapturedAudio};
pub use hooks::{EventBus, HookEvent, HookEventType, HookHandler, HookResult};
pub use plugins::{Plugin, PluginLoadError, PluginManager, PluginManifest, PluginRegistry};
pub use state::{AppState, Settings};
pub use stt::{TranscriptionResult, WhisperEngine, WhisperModel};
