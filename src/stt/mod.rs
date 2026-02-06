//! Speech-to-Text Module
//!
//! Provides speech-to-text functionality using whisper.cpp.

mod whisper;

pub use whisper::{TranscriptionResult, WhisperEngine, WhisperModel};
