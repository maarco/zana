//! Tauri Commands
//!
//! All commands exposed to the frontend.

mod diagnostics;
mod onboarding;
mod settings;
mod update;
mod voicebot;

pub use diagnostics::*;
pub use onboarding::*;
pub use settings::*;
pub use update::*;
pub use voicebot::*;
