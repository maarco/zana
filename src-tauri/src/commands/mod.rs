//! Tauri Commands
//!
//! All commands exposed to the frontend.

mod audio;
mod diagnostics;
mod onboarding;
mod plugins;
mod settings;
mod transcription;

pub use audio::*;
pub use diagnostics::*;
pub use onboarding::*;
pub use plugins::*;
pub use settings::*;
pub use transcription::*;
