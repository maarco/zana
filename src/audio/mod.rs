//! Audio Module
//!
//! Audio capture and processing for Zana.

mod capture;

pub use capture::{AudioCapture, AudioDevice, AudioMetrics, CapturedAudio};
