//! Centralized error types for Zana
//!
//! This module provides structured, user-friendly error types using thiserror.
//! All errors include helpful messages explaining what went wrong and how to fix it.

use std::error::Error;
use std::path::PathBuf;

/// Zana result type
pub type Result<T> = std::result::Result<T, ZanaError>;

/// Main Zana error type
#[derive(Debug, thiserror::Error)]
pub enum ZanaError {
    /// Audio capture errors
    #[error("Audio Error: {message}")]
    Audio {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Whisper model errors
    #[error("Whisper Model Error: {message}")]
    Whisper {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// GPU initialization errors
    #[error("GPU Error: {message}")]
    Gpu {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Plugin loading errors
    #[error("Plugin Error: {message}")]
    Plugin {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Settings save errors
    #[error("Settings Error: {message}")]
    Settings {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

// ============================================================================
// Audio Capture Errors
// ============================================================================

/// Audio capture error with helpful user messages
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("No audio input devices found on your system.\n\nHow to fix:\n  - Connect a microphone or audio input device\n  - Check your system audio settings\n  - Ensure microphone permissions are granted")]
    NoDevicesFound,

    #[error("Failed to access audio device '{device_name}': {reason}\n\nHow to fix:\n  - Check if the device is connected and working\n  - Try selecting a different audio device in settings\n  - Restart the application")]
    DeviceAccessFailed {
        device_name: String,
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Requested audio device '{device_id}' not found.\n\nAvailable devices:\n{available}\n\nHow to fix:\n  - Select a different device from the list above\n  - Check if the device is properly connected")]
    DeviceNotFound {
        device_id: String,
        available: String,
    },

    #[error("Audio stream error: {reason}\n\nHow to fix:\n  - Check if another application is using the microphone\n  - Try restarting the application\n  - Verify your audio device is working properly")]
    StreamFailed {
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Unsupported audio format: {format}\n\nHow to fix:\n  - Your audio device may not be compatible\n  - Try using a different audio input device")]
    UnsupportedFormat { format: String },

    #[error("Audio thread not responding. The application may be in an unstable state.\n\nHow to fix:\n  - Restart the application")]
    AudioThreadDied,

    #[error("Recording already in progress. Stop the current recording first.")]
    AlreadyRecording,

    #[error("No recording in progress. Start a recording first.")]
    NotRecording,
}

impl From<AudioError> for ZanaError {
    fn from(err: AudioError) -> Self {
        ZanaError::Audio {
            message: err.to_string(),
            source: None,
        }
    }
}

// ============================================================================
// Whisper Model Errors
// ============================================================================

/// Whisper model error with helpful user messages
#[derive(Debug, thiserror::Error)]
pub enum WhisperError {
    #[error("Model '{model}' not found at: {path}\n\nHow to fix:\n  - Download the model first (approx {size_mb} MB)\n  - Or use Settings to download it automatically\n  - Model URL: {url}")]
    ModelNotFound {
        model: String,
        path: PathBuf,
        size_mb: u64,
        url: String,
    },

    #[error("Failed to download model '{model}' ({size_mb} MB): {reason}\n\nHow to fix:\n  - Check your internet connection\n  - Ensure you have enough disk space (need ~{size_mb} MB free)\n  - Try again later\n  - Or manually download from: {url}")]
    DownloadFailed {
        model: String,
        size_mb: u64,
        reason: String,
        url: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Download interrupted: {reason}\n\nHow to fix:\n  - Check your internet connection\n  - Try downloading again")]
    DownloadInterrupted { reason: String },

    #[error("Failed to save model file to {path}: {reason}\n\nHow to fix:\n  - Ensure you have write permissions to the models directory\n  - Check available disk space\n  - Verify the directory exists: {dir}")]
    SaveFailed {
        path: PathBuf,
        dir: PathBuf,
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Failed to create models directory: {path}\n\nHow to fix:\n  - Ensure you have write permissions\n  - Check available disk space")]
    ModelsDirCreationFailed {
        path: PathBuf,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Failed to load Whisper model '{model}': {reason}\n\nPossible causes:\n  - Model file is corrupted (try re-downloading)\n  - Incompatible model version\n  - Invalid model format\n\nHow to fix:\n  - Delete the model file and download again\n  - Ensure the model is compatible with this version")]
    ModelLoadFailed {
        model: String,
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Failed to create Whisper context: {reason}\n\nHow to fix:\n  - Ensure the model file is valid\n  - Try re-downloading the model\n  - Check if you have enough memory")]
    ContextCreationFailed {
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Transcription failed: {reason}\n\nHow to fix:\n  - Ensure audio quality is good\n  - Try a shorter recording\n  - Check if the model is loaded correctly")]
    TranscriptionFailed {
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("WAV file not found: {path}\n\nHow to fix:\n  - Check if the file path is correct\n  - Ensure the file exists")]
    FileNotFound { path: String },

    #[error("Invalid WAV file format: {reason}\n\nHow to fix:\n  - Ensure the file is a valid WAV file\n  - Try converting to a standard WAV format (16-bit, mono or stereo)")]
    InvalidWavFormat {
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl From<WhisperError> for ZanaError {
    fn from(err: WhisperError) -> Self {
        ZanaError::Whisper {
            message: err.to_string(),
            source: None,
        }
    }
}

// ============================================================================
// GPU Initialization Errors
// ============================================================================

/// GPU initialization error with helpful user messages
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[error("No compatible GPU found.\n\nRequirements:\n  - A graphics card supporting Vulkan, Metal, or DirectX 12\n  - Up-to-date graphics drivers\n\nHow to fix:\n  - Update your graphics drivers\n  - On Windows: Update from GPU manufacturer (NVIDIA/AMD/Intel)\n  - On macOS: Ensure macOS is up to date\n  - On Linux: Install Mesa drivers or proprietary GPU drivers")]
    NoAdapterFound,

    #[error("GPU initialization failed: {reason}\n\nHow to fix:\n  - Update your graphics drivers\n  - Ensure your GPU supports the required features\n  - Try restarting the application")]
    InitFailed {
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Failed to create GPU device: {reason}\n\nYour GPU may not support the required features.\n\nHow to fix:\n  - Update your graphics drivers\n  - Check if your GPU is compatible\n  - Try updating your operating system")]
    DeviceCreationFailed {
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Shader compilation failed: {shader}\n\n{error}\n\nHow to fix:\n  - This may indicate a corrupted installation\n  - Try reinstalling the application")]
    ShaderCompilationFailed {
        shader: String,
        error: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Shader file not found: {path}\n\nHow to fix:\n  - Reinstall the application\n  - Ensure shader files are present")]
    ShaderNotFound { path: String },

    #[error("GPU feature not supported: {feature}\n\nYour graphics card does not support this feature.\n\nHow to fix:\n  - Update your graphics drivers\n  - Consider upgrading your GPU\n  - The application will attempt to use a fallback mode")]
    FeatureNotSupported { feature: String },

    #[error("GPU memory insufficient. Required: {required_mb} MB, Available: {available_mb} MB\n\nHow to fix:\n  - Close other applications using GPU\n  - Reduce graphics quality settings\n  - Consider upgrading your GPU")]
    InsufficientMemory {
        required_mb: u64,
        available_mb: u64,
    },

    #[error("GPU lost connection. The graphics driver may have crashed.\n\nHow to fix:\n  - Restart the application\n  - Update your graphics drivers\n  - Check if your GPU is overheating")]
    DeviceLost,
}

impl From<GpuError> for ZanaError {
    fn from(err: GpuError) -> Self {
        ZanaError::Gpu {
            message: err.to_string(),
            source: None,
        }
    }
}

// ============================================================================
// Settings Save Errors
// ============================================================================

/// Settings save error with helpful user messages
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("Permission denied: Cannot write to {path}\n\nHow to fix:\n  - Ensure you have write permissions to the config directory\n  - On Linux/macOS: Check directory permissions with `ls -la`\n  - On Windows: Run as administrator if needed\n  - Check if the file is read-only")]
    PermissionDenied {
        path: PathBuf,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Config directory does not exist and cannot be created: {path}\n\nHow to fix:\n  - Ensure you have write permissions to the parent directory\n  - Check available disk space\n  - Manually create the directory if needed")]
    DirectoryCreationFailed {
        path: PathBuf,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Disk full: Cannot save settings. Need at least {required_bytes} bytes.\n\nHow to fix:\n  - Free up disk space\n  - Clear temporary files\n  - Move your user directory to a drive with more space")]
    DiskFull {
        required_bytes: u64,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Failed to serialize settings: {reason}\n\nHow to fix:\n  - This may indicate corrupted settings\n  - Try resetting to default settings\n  - Report this issue if it persists")]
    SerializationFailed {
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Failed to parse existing settings file: {reason}\n\nThe settings file may be corrupted.\n\nHow to fix:\n  - Delete the settings file to reset to defaults\n  - File location: {path}")]
    ParseFailed {
        reason: String,
        path: PathBuf,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Failed to read settings file: {reason}\n\nHow to fix:\n  - Ensure the file exists and is readable\n  - Check file permissions\n  - The application will use default settings")]
    ReadFailed {
        path: PathBuf,
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Failed to write settings file: {path}\n\nHow to fix:\n  - Check available disk space\n  - Ensure you have write permissions\n  - Verify the directory exists")]
    WriteFailed {
        path: PathBuf,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl From<SettingsError> for ZanaError {
    fn from(err: SettingsError) -> Self {
        ZanaError::Settings {
            message: err.to_string(),
            source: None,
        }
    }
}

// ============================================================================
// Plugin Loading Errors (enhanced version of existing PluginLoadError)
// ============================================================================

/// Plugin loading error with helpful user messages
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin directory not found: {path}\n\nHow to fix:\n  - Create the plugins directory\n  - Reinstall the application")]
    DirectoryNotFound { path: PathBuf },

    #[error("Failed to read plugin directory {path}: {reason}\n\nHow to fix:\n  - Check directory permissions\n  - Ensure the directory exists")]
    DirectoryReadError {
        path: PathBuf,
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Plugin '{plugin_id}': Manifest not found at {path}\n\nHow to fix:\n  - Ensure plugin.toml exists in the plugin directory\n  - Verify the plugin is properly installed")]
    ManifestNotFound {
        plugin_id: String,
        path: PathBuf,
    },

    #[error("Plugin '{plugin_id}': Failed to parse manifest: {reason}\n\nHow to fix:\n  - Check that plugin.toml is valid TOML format\n  - Verify all required fields are present\n  - See the plugin documentation for the correct format")]
    ManifestParseError {
        plugin_id: String,
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Plugin '{plugin_id}': Invalid manifest: {reason}\n\nHow to fix:\n  - Fix the validation error in plugin.toml\n  - Ensure all required fields have valid values")]
    ValidationError {
        plugin_id: String,
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Plugin '{plugin_id}' is disabled.\n\nHow to fix:\n  - Enable the plugin in settings\n  - Or remove the plugin from the plugins directory")]
    PluginDisabled { plugin_id: String },

    #[error("Plugin '{plugin_id}': Dependencies not met: {dependencies}\n\nHow to fix:\n  - Install the required dependencies\n  - Check the plugin documentation for requirements")]
    DependenciesNotMet {
        plugin_id: String,
        dependencies: String,
    },

    #[error("Plugin '{plugin_id}': Failed to instantiate: {reason}\n\nHow to fix:\n  - Ensure the plugin is compatible with this version\n  - Check the plugin logs for more details\n  - Contact the plugin author")]
    InstantiationFailed {
        plugin_id: String,
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Plugin '{plugin_id}': Initialization failed: {reason}\n\nHow to fix:\n  - Check the plugin configuration\n  - See the plugin documentation for setup instructions\n  - Review the application logs for details")]
    InitFailed {
        plugin_id: String,
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Plugin '{plugin_id}': Runtime error: {reason}\n\nHow to fix:\n  - Check the plugin configuration\n  - Review the application logs for details\n  - Report this to the plugin author")]
    RuntimeError {
        plugin_id: String,
        reason: String,
        #[source] source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl From<PluginError> for ZanaError {
    fn from(err: PluginError) -> Self {
        ZanaError::Plugin {
            message: err.to_string(),
            source: None,
        }
    }
}

// ============================================================================
// Helper macros for error creation with context
// ============================================================================

/// Helper macro to create an AudioError with context
#[macro_export]
macro_rules! audio_err {
    ($variant:ident, $($arg:tt)*) => {
        $crate::error::AudioError::$variant {
            $($arg)*
        }.into()
    };
}

/// Helper macro to create a WhisperError with context
#[macro_export]
macro_rules! whisper_err {
    ($variant:ident, $($arg:tt)*) => {
        $crate::error::WhisperError::$variant {
            $($arg)*
        }.into()
    };
}

/// Helper macro to create a GpuError with context
#[macro_export]
macro_rules! gpu_err {
    ($variant:ident, $($arg:tt)*) => {
        $crate::error::GpuError::$variant {
            $($arg)*
        }.into()
    };
}

/// Helper macro to create a SettingsError with context
#[macro_export]
macro_rules! settings_err {
    ($variant:ident, $($arg:tt)*) => {
        $crate::error::SettingsError::$variant {
            $($arg)*
        }.into()
    };
}

/// Helper macro to create a PluginError with context
#[macro_export]
macro_rules! plugin_err {
    ($variant:ident, $($arg:tt)*) => {
        $crate::error::PluginError::$variant {
            $($arg)*
        }.into()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_error_messages() {
        let err = AudioError::NoDevicesFound;
        assert!(err.to_string().contains("No audio input devices found"));
        assert!(err.to_string().contains("How to fix"));
    }

    #[test]
    fn test_whisper_error_messages() {
        let err = WhisperError::ModelNotFound {
            model: "tiny".to_string(),
            path: PathBuf::from("/tmp/model.bin"),
            size_mb: 39,
            url: "https://example.com/model.bin".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("not found"));
        assert!(msg.contains("39 MB"));
        assert!(msg.contains("How to fix"));
    }

    #[test]
    fn test_gpu_error_messages() {
        let err = GpuError::NoAdapterFound;
        assert!(err.to_string().contains("No compatible GPU found"));
        assert!(err.to_string().contains("Requirements"));
        assert!(err.to_string().contains("How to fix"));
    }

    #[test]
    fn test_settings_error_messages() {
        let err = SettingsError::PermissionDenied {
            path: PathBuf::from("/config/settings.json"),
            source: None,
        };
        assert!(err.to_string().contains("Permission denied"));
        assert!(err.to_string().contains("How to fix"));
    }

    #[test]
    fn test_plugin_error_messages() {
        let err = PluginError::ManifestNotFound {
            plugin_id: "test-plugin".to_string(),
            path: PathBuf::from("/plugins/test"),
        };
        assert!(err.to_string().contains("test-plugin"));
        assert!(err.to_string().contains("Manifest not found"));
        assert!(err.to_string().contains("How to fix"));
    }

    #[test]
    fn test_error_chain() {
        let inner = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = SettingsError::WriteFailed {
            path: PathBuf::from("/test.json"),
            source: Some(Box::new(inner)),
        };
        assert!(err.source().is_some());
    }
}
