//! Speech-to-Text Module
//!
//! Provides speech-to-text functionality using whisper.cpp.

mod downloader;
mod whisper;

pub use downloader::ModelDownloader;
pub use whisper::{TranscriptionResult, TranscriptionSegment, WhisperEngine, WhisperModel};
