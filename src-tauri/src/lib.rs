//! Zana's Tauri backend for the local-first Voicebot desktop app.

#![allow(non_snake_case)]

pub mod commands;
pub mod onboarding;
pub mod state;
pub mod voicebot;

pub use state::{AppState, Settings};
