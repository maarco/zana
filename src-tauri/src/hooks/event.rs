//! Hook Event Types
//!
//! Defines all events that flow through the kVoice hook system.
//! Every significant operation emits events that plugins can intercept.

use serde::{Deserialize, Serialize};

/// All event types in the kVoice system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum HookEvent {
    // =========================================================================
    // Audio Events
    // =========================================================================
    /// Audio capture has started
    AudioCaptureStart {
        device_id: String,
        sample_rate: u32,
        channels: u16,
    },

    /// Audio capture has stopped
    AudioCaptureStop { duration_ms: u64 },

    /// Audio level changed (emitted frequently during recording)
    AudioLevelChange {
        /// Average level (0.0 - 1.0)
        level: f32,
        /// Peak level (0.0 - 1.0)
        peak: f32,
    },

    /// FFT data available (for visualizations)
    AudioFftReady {
        /// Frequency bin magnitudes (0.0 - 1.0)
        bins: Vec<f32>,
        /// Number of bins
        bin_count: usize,
    },

    /// Raw audio buffer ready for processing
    AudioBufferReady {
        /// Sample count
        sample_count: usize,
        /// Sample rate
        sample_rate: u32,
        /// Channel count
        channels: u16,
    },

    // =========================================================================
    // Transcription Events
    // =========================================================================
    /// Transcription has started
    TranscriptionStart {
        model: String,
        audio_duration_ms: u64,
    },

    /// Transcription progress update
    TranscriptionProgress {
        /// Progress percentage (0.0 - 100.0)
        percent: f32,
    },

    /// A transcription segment is available
    TranscriptionSegment {
        /// Start time in milliseconds
        start_ms: i64,
        /// End time in milliseconds
        end_ms: i64,
        /// Transcribed text
        text: String,
    },

    /// Transcription completed successfully
    TranscriptionComplete {
        /// Full transcribed text
        text: String,
        /// All segments
        segments: Vec<TranscriptionSegmentData>,
        /// Processing duration in milliseconds
        processing_ms: u64,
    },

    /// Transcription failed
    TranscriptionError { error: String },

    // =========================================================================
    // Plugin Events
    // =========================================================================
    /// Plugin was loaded successfully
    PluginLoaded {
        id: String,
        name: String,
        version: String,
        plugin_type: PluginType,
    },

    /// Plugin was unloaded
    PluginUnloaded { id: String },

    /// Plugin encountered an error
    PluginError { id: String, error: String },

    /// Plugin configuration changed
    PluginConfigChanged {
        id: String,
        key: String,
        value: serde_json::Value,
    },

    /// Plugin was enabled
    PluginEnabled { id: String },

    /// Plugin was disabled
    PluginDisabled { id: String },

    // =========================================================================
    // UI Events
    // =========================================================================
    /// Orb style was changed
    OrbStyleChanged {
        previous_style: Option<String>,
        new_style: String,
    },

    /// Theme was changed
    ThemeChanged { theme: Theme },

    /// Window was resized
    WindowResized { width: u32, height: u32 },

    /// Recording button was pressed
    RecordButtonPressed,

    /// Settings panel opened
    SettingsOpened,

    /// Settings panel closed
    SettingsClosed,

    // =========================================================================
    // Settings Events
    // =========================================================================
    /// A setting was changed
    SettingChanged {
        key: String,
        old_value: Option<serde_json::Value>,
        new_value: serde_json::Value,
    },

    /// Transcription profile was changed
    ProfileChanged {
        previous_profile: Option<String>,
        new_profile: String,
    },

    /// Whisper model was changed
    ModelChanged {
        previous_model: Option<String>,
        new_model: String,
    },

    // =========================================================================
    // System Events
    // =========================================================================
    /// Application started
    AppStarted,

    /// Application is shutting down
    AppShutdown,

    /// Error occurred
    Error { code: String, message: String },
}

/// Event type identifier (for subscription filtering)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEventType {
    // Audio
    AudioCaptureStart,
    AudioCaptureStop,
    AudioLevelChange,
    AudioFftReady,
    AudioBufferReady,

    // Transcription
    TranscriptionStart,
    TranscriptionProgress,
    TranscriptionSegment,
    TranscriptionComplete,
    TranscriptionError,

    // Plugin
    PluginLoaded,
    PluginUnloaded,
    PluginError,
    PluginConfigChanged,
    PluginEnabled,
    PluginDisabled,

    // UI
    OrbStyleChanged,
    ThemeChanged,
    WindowResized,
    RecordButtonPressed,
    SettingsOpened,
    SettingsClosed,

    // Settings
    SettingChanged,
    ProfileChanged,
    ModelChanged,

    // System
    AppStarted,
    AppShutdown,
    Error,

    // Wildcard - matches all events
    All,
}

impl HookEvent {
    /// Get the event type for this event
    pub fn event_type(&self) -> HookEventType {
        match self {
            HookEvent::AudioCaptureStart { .. } => HookEventType::AudioCaptureStart,
            HookEvent::AudioCaptureStop { .. } => HookEventType::AudioCaptureStop,
            HookEvent::AudioLevelChange { .. } => HookEventType::AudioLevelChange,
            HookEvent::AudioFftReady { .. } => HookEventType::AudioFftReady,
            HookEvent::AudioBufferReady { .. } => HookEventType::AudioBufferReady,

            HookEvent::TranscriptionStart { .. } => HookEventType::TranscriptionStart,
            HookEvent::TranscriptionProgress { .. } => HookEventType::TranscriptionProgress,
            HookEvent::TranscriptionSegment { .. } => HookEventType::TranscriptionSegment,
            HookEvent::TranscriptionComplete { .. } => HookEventType::TranscriptionComplete,
            HookEvent::TranscriptionError { .. } => HookEventType::TranscriptionError,

            HookEvent::PluginLoaded { .. } => HookEventType::PluginLoaded,
            HookEvent::PluginUnloaded { .. } => HookEventType::PluginUnloaded,
            HookEvent::PluginError { .. } => HookEventType::PluginError,
            HookEvent::PluginConfigChanged { .. } => HookEventType::PluginConfigChanged,
            HookEvent::PluginEnabled { .. } => HookEventType::PluginEnabled,
            HookEvent::PluginDisabled { .. } => HookEventType::PluginDisabled,

            HookEvent::OrbStyleChanged { .. } => HookEventType::OrbStyleChanged,
            HookEvent::ThemeChanged { .. } => HookEventType::ThemeChanged,
            HookEvent::WindowResized { .. } => HookEventType::WindowResized,
            HookEvent::RecordButtonPressed => HookEventType::RecordButtonPressed,
            HookEvent::SettingsOpened => HookEventType::SettingsOpened,
            HookEvent::SettingsClosed => HookEventType::SettingsClosed,

            HookEvent::SettingChanged { .. } => HookEventType::SettingChanged,
            HookEvent::ProfileChanged { .. } => HookEventType::ProfileChanged,
            HookEvent::ModelChanged { .. } => HookEventType::ModelChanged,

            HookEvent::AppStarted => HookEventType::AppStarted,
            HookEvent::AppShutdown => HookEventType::AppShutdown,
            HookEvent::Error { .. } => HookEventType::Error,
        }
    }
}

/// Transcription segment data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegmentData {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// Plugin type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginType {
    OrbStyle,
    AudioProcessor,
    PostProcessor,
    Integration,
}

/// Theme enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
    System,
    Custom(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_mapping() {
        let event = HookEvent::AudioCaptureStart {
            device_id: "test".to_string(),
            sample_rate: 48000,
            channels: 1,
        };
        assert_eq!(event.event_type(), HookEventType::AudioCaptureStart);
    }

    #[test]
    fn test_event_serialization() {
        let event = HookEvent::TranscriptionComplete {
            text: "Hello world".to_string(),
            segments: vec![TranscriptionSegmentData {
                start_ms: 0,
                end_ms: 1000,
                text: "Hello world".to_string(),
            }],
            processing_ms: 500,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("TranscriptionComplete"));
        assert!(json.contains("Hello world"));
    }
}
