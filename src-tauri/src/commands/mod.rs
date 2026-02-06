//! Tauri Commands
//!
//! All commands exposed to the frontend.

mod audio;
mod onboarding;
mod plugins;
mod transcription;

pub use audio::*;
pub use onboarding::*;
pub use plugins::*;
pub use transcription::*;
