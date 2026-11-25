//! Tauri Commands
//!
//! All commands exposed to the frontend.

mod audio;
mod cleanup;
mod diagnostics;
mod onboarding;
mod plugins;
mod settings;
mod transcription;
mod update;
mod voicebot;

pub use audio::*;
pub use cleanup::*;
pub use diagnostics::*;
pub use onboarding::*;
pub use plugins::*;
pub use settings::*;
pub use transcription::*;
pub use update::*;
pub use voicebot::*;
