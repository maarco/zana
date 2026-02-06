# kVoice API Reference

This document describes the public API of kVoice for developers who want to use it as a library or extend it.

---

## Table of Contents

1. [Overview](#overview)
2. [Audio API](#audio-api)
3. [Speech-to-Text API](#speech-to-text-api)
4. [Event System API](#event-system-api)
5. [Plugin API](#plugin-api)
6. [GUI API](#gui-api)
7. [State Management API](#state-management-api)
8. [Type Definitions](#type-definitions)

---

## Overview

kVoice is organized into several modules, each with a specific responsibility:

```rust
use kvoice::{
    // Audio capture
    audio::{AudioCapture, AudioDevice, AudioMetrics, CapturedAudio},

    // GUI
    gui::{KVoiceApp, RecordingCommand, RecordingEvent, TranscriptionCommand, TranscriptionEvent},

    // Event system
    hooks::{EventBus, HookEvent, HookEventType, HookHandler, HookResult},

    // Plugins
    plugins::{Plugin, PluginManifest, PluginRegistry},

    // State
    state::{AppState, Settings},

    // Speech-to-text
    stt::{TranscriptionResult, WhisperEngine, WhisperModel},
};
```

---

## Audio API

### AudioCapture

Main audio capture engine using cpal for cross-platform audio input.

#### Constructor

```rust
impl AudioCapture {
    /// Create a new audio capture engine with default configuration
    pub fn new(event_bus: Arc<EventBus>) -> Self;

    /// Create with custom configuration
    pub fn with_config(event_bus: Arc<EventBus>, config: CaptureConfig) -> Self;
}
```

**Parameters**:
- `event_bus`: Event bus for emitting audio events
- `config`: Audio capture configuration (sample rate, channels, FFT size)

#### Methods

```rust
impl AudioCapture {
    /// List available input devices
    pub fn list_devices() -> Result<Vec<AudioDevice>>;

    /// Start capturing audio
    pub async fn start(&self, device_id: Option<&str>) -> Result<()>;

    /// Stop capturing and return samples
    pub async fn stop(&self) -> Result<CapturedAudio>;

    /// Get current audio metrics (for UI)
    pub async fn get_metrics(&self) -> AudioMetrics;

    /// Check if currently recording
    pub fn is_recording(&self) -> bool;
}
```

**Events Emitted**:
- `AudioCaptureStart { device_id, sample_rate, channels }`
- `AudioCaptureStop { duration_ms }`
- `AudioLevelChange { level, peak }` (~60fps during recording)
- `AudioFftReady { bins, bin_count }` (~60fps during recording)

### AudioDevice

Information about an audio input device.

```rust
pub struct AudioDevice {
    pub id: String,           // Device identifier
    pub name: String,         // Human-readable name
    pub is_default: bool,     // System default device
    pub sample_rate: Option<u32>,  // Supported sample rate
    pub channels: Option<u16>,     // Supported channel count
}
```

### AudioMetrics

Real-time audio metrics for visualization.

```rust
pub struct AudioMetrics {
    pub level: f32,           // Average level (0.0 - 1.0)
    pub peak: f32,            // Peak level (0.0 - 1.0)
    pub fft_bins: Vec<f32>,   // FFT frequency bins
    pub is_active: bool,      // Currently capturing
    pub duration_ms: u64,     // Duration captured
}
```

### CapturedAudio

Audio data ready for transcription.

```rust
pub struct CapturedAudio {
    pub samples: Vec<f32>,    // f32 samples (mono, 16kHz)
    pub sample_rate: u32,     // Always 16000 for Whisper
    pub channels: u16,        // Always 1 (mono)
    pub duration_ms: u64,     // Duration in milliseconds
}
```

---

## Speech-to-Text API

### WhisperEngine

Speech-to-text engine using whisper.cpp bindings.

#### Constructor

```rust
impl WhisperEngine {
    /// Create a new Whisper engine
    pub fn new(event_bus: Arc<EventBus>) -> anyhow::Result<Self>;

    /// Create with custom models directory
    pub fn with_models_dir(event_bus: Arc<EventBus>, dir: PathBuf) -> anyhow::Result<Self>;
}
```

#### Methods

```rust
impl WhisperEngine {
    /// Transcribe audio samples
    pub async fn transcribe(
        &self,
        samples: &[f32],
        model: WhisperModel
    ) -> anyhow::Result<TranscriptionResult>;

    /// Check if model is downloaded
    pub fn is_model_downloaded(&self, model: WhisperModel) -> bool;

    /// Download a model
    pub async fn download_model(
        &self,
        model: WhisperModel
    ) -> anyhow::Result<PathBuf>;

    /// Get models directory
    pub fn models_dir(&self) -> &Path;
}
```

**Events Emitted**:
- `TranscriptionStart { model }`
- `TranscriptionProgress { percent }`
- `TranscriptionComplete { text, segments }`
- `TranscriptionError { error }`

### WhisperModel

Available Whisper model sizes.

```rust
pub enum WhisperModel {
    Tiny,    // 39MB, fastest
    Base,    // 74MB, fast
    Small,   // 244MB, balanced (recommended)
    Medium,  // 769MB, accurate
    Large,   // 1.5GB, most accurate
}
```

**Methods**:
```rust
impl WhisperModel {
    pub fn name(&self) -> &str;
    pub fn size_mb(&self) -> u64;
    pub fn filename(&self) -> &str;
    pub fn from_str(s: &str) -> Option<Self>;
}
```

### TranscriptionResult

Result of transcription.

```rust
pub struct TranscriptionResult {
    pub text: String,                // Full transcription text
    pub processing_ms: u64,          // Processing time
    pub segments: Vec<Segment>,      // Time-stamped segments
}

pub struct Segment {
    pub start_ms: i64,               // Start time
    pub end_ms: i64,                 // End time
    pub text: String,                // Segment text
}
```

---

## Event System API

### EventBus

Central event hub for routing events to handlers and subscribers.

#### Constructor

```rust
impl EventBus {
    pub fn new() -> Self;
}
```

#### Methods

```rust
impl EventBus {
    /// Register a hook handler
    pub async fn register(&self, handler: Arc<dyn HookHandler>) -> anyhow::Result<()>;

    /// Unregister a handler
    pub async fn unregister(&self, handler_id: &str) -> anyhow::Result<()>;

    /// Emit an event through the hook pipeline
    pub async fn emit(&self, event: HookEvent) -> HookEvent;

    /// Subscribe to all events
    pub fn subscribe_all(&self) -> broadcast::Receiver<HookEvent>;

    /// Subscribe to specific event type
    pub async fn subscribe(&self, event_type: HookEventType) -> broadcast::Receiver<HookEvent>;

    /// Get statistics
    pub async fn stats(&self) -> EventBusStats;
}
```

### HookEvent

Events emitted by kVoice.

```rust
pub enum HookEvent {
    // Audio events
    AudioCaptureStart { device_id: String, sample_rate: u32, channels: u16 },
    AudioCaptureStop { duration_ms: u64 },
    AudioLevelChange { level: f32, peak: f32 },
    AudioFftReady { bins: Vec<f32>, bin_count: usize },

    // Transcription events
    TranscriptionStart { model: WhisperModel },
    TranscriptionProgress { percent: f32 },
    TranscriptionSegment { start_ms: i64, end_ms: i64, text: String },
    TranscriptionComplete { text: String, segments: Vec<Segment> },
    TranscriptionError { error: String },

    // Plugin events
    PluginLoaded { id: String, version: String },
    PluginUnloaded { id: String },
    PluginError { id: String, error: String },

    // UI events
    OrbStyleChanged { style_id: String },
    WindowResized { width: u32, height: u32 },

    // Settings events
    SettingChanged { key: String, value: serde_json::Value },

    // Application events
    AppStarted,
    AppShutdown,

    // Error events
    Error { code: String, message: String },
}
```

### HookHandler

Trait for implementing event handlers.

```rust
#[async_trait]
pub trait HookHandler: Send + Sync {
    /// Unique identifier
    fn id(&self) -> &str;

    /// Priority (lower = earlier execution)
    fn priority(&self) -> i32 { 100 }

    /// Subscribed event types
    fn subscribed_events(&self) -> Vec<HookEventType>;

    /// Handle an event
    async fn handle(&self, event: &mut HookEvent) -> HookResult;

    /// Enable/disable handler
    fn is_enabled(&self) -> bool { true }

    /// Called when registered
    async fn on_register(&self) -> anyhow::Result<()> { Ok(()) }

    /// Called when unregistered
    async fn on_unregister(&self) -> anyhow::Result<()> { Ok(()) }
}
```

### HookResult

Result of event handling.

```rust
pub enum HookResult {
    Continue,   // Continue to next handler
    Stop,       // Stop event propagation
    Modified,   // Event was modified, continue
    Skip,       // Handler declined to process
}
```

---

## Plugin API

### Plugin Trait

Base trait that all plugins must implement.

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin manifest
    fn manifest(&self) -> &PluginManifest;

    /// Initialize plugin
    async fn init(&mut self, context: PluginContext) -> anyhow::Result<()>;

    /// Shutdown plugin
    async fn shutdown(&mut self) -> anyhow::Result<()>;

    /// Handle configuration changes
    fn on_config_change(&mut self, config: &HashMap<String, JsonValue>);

    /// Get hook handler (optional)
    fn hook_handler(&self) -> Option<Arc<dyn HookHandler>> {
        None
    }

    /// Check capability
    fn has_capability(&self, cap: &str) -> bool;
}
```

### Plugin Types

#### OrbStylePlugin

Audio visualization plugins.

```rust
#[async_trait]
pub trait OrbStylePlugin: Plugin {
    /// Update animation state
    fn update(&mut self, ctx: &RenderContext);

    /// Get draw commands
    fn render(&self, ctx: &RenderContext) -> Vec<DrawCommand>;

    /// Handle resize
    fn on_resize(&mut self, width: u32, height: u32);
}
```

#### AudioProcessorPlugin

Modify audio before transcription.

```rust
#[async_trait]
pub trait AudioProcessorPlugin: Plugin {
    async fn process(&mut self, samples: &mut Vec<f32>, sample_rate: u32)
        -> anyhow::Result<()>;
}
```

#### PostProcessorPlugin

Modify transcription output.

```rust
#[async_trait]
pub trait PostProcessorPlugin: Plugin {
    async fn process(&mut self, text: &str) -> anyhow::Result<String>;
}
```

#### IntegrationPlugin

Connect to external services.

```rust
#[async_trait]
pub trait IntegrationPlugin: Plugin {
    async fn on_transcription(&mut self, text: &str) -> anyhow::Result<()>;
}
```

### PluginManifest

Plugin metadata.

```rust
pub struct PluginManifest {
    pub plugin: PluginMeta,
    pub plugin_type: PluginKind,
    pub capabilities: PluginCapabilities,
    pub ui: Option<PluginUiConfig>,
    pub config: Option<PluginConfigSchema>,
    pub marketplace: Option<MarketplaceMeta>,
}

pub struct PluginMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub license: String,
}

pub enum PluginKind {
    OrbStyle,
    AudioProcessor,
    PostProcessor,
    Integration,
}
```

### PluginRegistry

Registry of loaded plugins.

```rust
pub struct PluginRegistry {
    // Register plugin
    pub fn register(&mut self, plugin: Box<dyn Plugin>) -> anyhow::Result<()>;

    // Unregister plugin
    pub fn unregister(&mut self, id: &str) -> anyhow::Result<()>;

    // Get plugin by ID
    pub fn get(&self, id: &str) -> Option<&dyn Plugin>;

    // Get all orb style plugins
    pub fn orb_styles(&self) -> Vec<&dyn OrbStylePlugin>;

    // List all plugin IDs
    pub fn list_ids(&self) -> Vec<String>;
}
```

---

## GUI API

### KVoiceApp

Main egui application.

```rust
pub struct KVoiceApp {
    // Private fields
}

impl KVoiceApp {
    /// Create new app (called by eframe)
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self;
}

impl eframe::App for KVoiceApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame);
    fn save(&mut self, storage: &mut dyn eframe::Storage);
}
```

### Channel Types

For GUI to async communication.

```rust
/// Recording commands (GUI -> async)
pub enum RecordingCommand {
    Start { device_id: Option<String> },
    Stop,
    QueryStatus,
}

/// Recording events (async -> GUI)
pub enum RecordingEvent {
    Started { device_name: String, sample_rate: u32 },
    Stopped { sample_count: usize, duration_ms: u64 },
    MetricsUpdate { level: f32, peak: f32, fft_bins: Vec<f32> },
    Error(String),
}

/// Transcription commands (GUI -> async)
pub enum TranscriptionCommand {
    Transcribe { samples: Vec<f32>, model: String },
    Cancel,
}

/// Transcription events (async -> GUI)
pub enum TranscriptionEvent {
    Progress { progress: f32, message: String },
    Complete { text: String, duration_ms: u32 },
    Error(String),
}
```

---

## State Management API

### AppState

Global application state.

```rust
pub struct AppState {
    pub event_bus: Arc<EventBus>,
    pub audio_capture: Arc<Mutex<AudioCapture>>,
    pub whisper_engine: Arc<Mutex<WhisperEngine>>,
    pub captured_audio: Mutex<Option<CapturedAudio>>,
    pub settings: RwLock<Settings>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self>;
    pub async fn save_settings(&self) -> anyhow::Result<()>;
}
```

### Settings

User application settings.

```rust
pub struct Settings {
    pub whisper_model: Option<String>,
    pub audio_device: Option<String>,
    pub orb_style: Option<String>,
    pub always_on_top: bool,
    pub window_width: u32,
    pub window_height: u32,
}

impl Settings {
    pub fn load() -> Self;
    pub fn save(&self) -> anyhow::Result<()>;
}
```

---

## Type Definitions

### Color

```rust
pub enum Color {
    Css(String),
    Rgba { r: u8, g: u8, b: u8, a: f32 },
}

impl Color {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self;
    pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self;
    pub fn css(s: impl Into<String>) -> Self;
    pub fn to_css(&self) -> String;
}
```

### DrawCommand

Canvas2D drawing commands.

```rust
pub enum DrawCommand {
    // Context
    Save, Restore,
    Clear, ClearRect { x, y, width, height },

    // Transform
    Translate { x, y }, Rotate { angle }, Scale { x, y },

    // Style
    FillStyle { color: Color },
    StrokeStyle { color: Color },
    LineWidth { width },
    GlobalAlpha { alpha },

    // Path
    BeginPath, ClosePath,
    MoveTo { x, y }, LineTo { x, y },
    Arc { x, y, radius, start_angle, end_angle },

    // Drawing
    Fill, Stroke,
    FillRect { x, y, width, height },
    FillText { text, x, y },

    // Gradients
    SetFillLinearGradient { x0, y0, x1, y1, stops },
    SetFillRadialGradient { x0, y0, r0, x1, y1, r1, stops },
}
```

### PluginContext

Context provided to plugins during initialization.

```rust
pub struct PluginContext {
    pub event_bus: Arc<EventBus>,
    pub data_dir: std::path::PathBuf,
    pub config: HashMap<String, JsonValue>,
    pub width: u32,
    pub height: u32,
}
```

### RenderContext

Context for rendering (passed to OrbStylePlugin).

```rust
pub struct RenderContext {
    pub width: f32,
    pub height: f32,
    pub cx: f32,
    pub cy: f32,
    pub time: f32,
    pub dt: f32,
    pub audio_level: f32,
    pub fft_bins: Vec<f32>,
    pub is_recording: bool,
    pub dpr: f32,
}
```

---

## Usage Examples

### Basic Recording and Transcription

```rust
use kvoice::{AudioCapture, WhisperEngine, WhisperModel};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let event_bus = Arc::new(kvoice::EventBus::new());

    // Create audio capture
    let capture = AudioCapture::new(event_bus.clone());

    // List devices
    let devices = AudioCapture::list_devices()?;
    println!("Available devices: {:?}", devices);

    // Start recording
    capture.start(None).await?;
    println!("Recording... Press Ctrl+C to stop");

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Stop recording
    let audio = capture.stop().await?;
    println!("Captured {} samples", audio.samples.len());

    // Transcribe
    let engine = WhisperEngine::new(event_bus)?;
    let result = engine.transcribe(&audio.samples, WhisperModel::Small).await?;
    println!("Transcription: {}", result.text);

    Ok(())
}
```

### Subscribing to Events

```rust
use kvoice::{EventBus, HookEvent, HookEventType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let event_bus = EventBus::new();

    // Subscribe to audio level changes
    let mut rx = event_bus.subscribe(HookEventType::AudioLevelChange).await;

    // Spawn task to handle events
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let HookEvent::AudioLevelChange { level, peak } = event {
                println!("Level: {:.2}, Peak: {:.2}", level, peak);
            }
        }
    });

    // Application continues...

    Ok(())
}
```

### Creating a Custom Handler

```rust
use kvoice::{HookHandler, HookEvent, HookResult, HookEventType};
use async_trait::async_trait;
use std::sync::Arc;

struct LoggingHandler;

#[async_trait]
impl HookHandler for LoggingHandler {
    fn id(&self) -> &str {
        "logger"
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        vec![HookEventType::All]
    }

    fn priority(&self) -> i32 {
        0 // High priority (runs early)
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        match event {
            HookEvent::AudioCaptureStart { device_id, .. } => {
                println!("Recording started on {}", device_id);
            }
            HookEvent::TranscriptionComplete { text, .. } => {
                println!("Got transcription: {}", text);
            }
            _ => {}
        }
        HookResult::Continue
    }
}

// Register handler
let handler = Arc::new(LoggingHandler);
event_bus.register(handler).await?;
```

---

## Error Handling

Most kVoice functions return `anyhow::Result<T>`:

```rust
pub async fn transcribe(&self, samples: &[f32], model: WhisperModel)
    -> anyhow::Result<TranscriptionResult>;
```

Common errors:
- `No audio input devices found`
- `Recording already in progress`
- `Model not downloaded`
- `Transcription failed`

---

## Thread Safety

Most kVoice types use `Arc` with interior mutability:

```rust
pub struct AudioCapture { /* ... */ }

unsafe impl Send for AudioCapture {}
unsafe impl Sync for AudioCapture {}
```

This allows sharing across async tasks safely.

---

## Platform Differences

### macOS
- Requires microphone permissions
- Uses CoreAudio backend

### Linux
- Requires PulseAudio or PipeWire
- Uses ALSA/PulseAudio backend

### Windows
- Requires WASAPI
- May need administrator privileges

---

## See Also

- [Architecture Documentation](./ARCHITECTURE.md)
- [Plugin Development Guide](./PLUGIN_DEVELOPMENT.md)
- [User Guide](./USER_GUIDE.md)
