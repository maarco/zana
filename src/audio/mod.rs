//! Audio Module
//!
//! Audio capture and processing for kVoice.

mod capture;

pub use capture::{AudioCapture, AudioDevice, AudioMetrics, CapturedAudio};
