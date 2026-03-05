# Zana Architecture Specification

## Build Status (Jan 2026)

status: ARCHITECTURE REDESIGN IN PROGRESS
change: Removing Tauri/HTML/JS, going pure Rust with egui+wgpu

---

## Overview

Zana is a cross-platform speech-to-text application with a beautiful, extensible visualization system. Built entirely in Rust with no web technologies, it provides native performance and a true desktop experience.

## Core Philosophy

1. **Pure Rust Everything** - UI, rendering, audio, STT, plugins - all Rust
2. **Hook-Based Architecture** - Every operation emits events that plugins can intercept
3. **Plugin Marketplace** - Visual styles distributed as installable plugins
4. **Cross-Platform** - Windows, macOS, and Linux from a single codebase
5. **Native Performance** - No webview overhead, direct GPU access

---

## Technology Stack

### Core (Pure Rust)
- **Windowing**: winit (cross-platform window creation)
- **GUI Framework**: egui (immediate-mode GUI)
- **GPU Rendering**: wgpu (WebGPU API, used for both UI and orb visualization)
- **Audio Capture**: cpal (cross-platform audio I/O)
- **STT Engine**: whisper-rs (whisper.cpp bindings)
- **Audio Codecs**:
  - hound (WAV)
  - audiopus (Opus)
  - symphonia (MP4/AAC)
  - webm-iterable (WebM)
- **Async Runtime**: tokio
- **HTTP Client**: reqwest (model downloads, marketplace)
- **Plugin System**: Custom Rust trait-based + WASM plugins

---

## Architecture Diagram

```
+------------------------------------------------------------------+
|                           Zana App                              |
+------------------------------------------------------------------+
|                                                                   |
|  +--------------------+    +----------------------------------+   |
|  |   Plugin Manager   |    |         Hook System              |   |
|  |                    |    |                                  |   |
|  | - Load plugins     |    | Events:                         |   |
|  | - Marketplace API  |    |   - audio:capture:start         |   |
|  | - Version control  |    |   - audio:capture:stop          |   |
|  | - Sandboxing       |    |   - audio:level:change          |   |
|  +--------------------+    |   - transcription:start         |   |
|           |                |   - transcription:complete      |   |
|           v                |   - transcription:segment       |   |
|  +--------------------+    |   - plugin:loaded               |   |
|  |   Plugin Registry  |    |   - plugin:error                |   |
|  |                    |    |   - ui:theme:change             |   |
|  | - Orb Styles       |    |   - settings:changed            |   |
|  | - Audio Processors |    +----------------------------------+   |
|  | - Post-processors  |                    |                      |
|  +--------------------+                    v                      |
|           |                +----------------------------------+   |
|           v                |         Event Bus (Rust)         |   |
|  +--------------------+    |                                  |   |
|  | Built-in Plugins   |    | - Pub/Sub pattern               |   |
|  |                    |    | - Async handlers                |   |
|  | - NebulaAuraOrb    |    | - Priority ordering             |   |
|  | - SphereOrb        |    | - Cancellable events            |   |
|  +--------------------+    +----------------------------------+   |
|                                            |                      |
+------------------------------------------------------------------+
                                             |
                                             v
+------------------------------------------------------------------+
|                        Core Services                              |
+------------------------------------------------------------------+
|                                                                   |
|  +------------------+  +------------------+  +------------------+ |
|  |  Audio Engine    |  |   STT Engine     |  |  Model Manager   | |
|  |                  |  |                  |  |                  | |
|  | - cpal capture   |  | - whisper-rs     |  | - Download       | |
|  | - Format detect  |  | - Model loading  |  | - Cache          | |
|  | - Resampling     |  | - Transcribe     |  | - Verification   | |
|  | - Level monitor  |  | - Segments       |  | - Updates        | |
|  +------------------+  +------------------+  +------------------+ |
|                                                                   |
+------------------------------------------------------------------+
```

---

## Hook System Design

The hook system is the foundation of Zana's extensibility. Every significant operation emits events that plugins can subscribe to.

### Event Categories

```rust
// Core event types
pub enum HookEvent {
    // Audio Events
    AudioCaptureStart { device_id: String, sample_rate: u32 },
    AudioCaptureStop { duration_ms: u64 },
    AudioLevelChange { level: f32, peak: f32 },
    AudioBufferReady { samples: Vec<f32>, channels: u16 },

    // Transcription Events
    TranscriptionStart { model: WhisperModel },
    TranscriptionProgress { percent: f32 },
    TranscriptionSegment { start_ms: i64, end_ms: i64, text: String },
    TranscriptionComplete { text: String, segments: Vec<Segment> },
    TranscriptionError { error: String },

    // Plugin Events
    PluginLoaded { id: String, version: String },
    PluginUnloaded { id: String },
    PluginError { id: String, error: String },

    // UI Events
    OrbStyleChanged { style_id: String },
    ThemeChanged { theme: Theme },
    WindowResized { width: u32, height: u32 },

    // Settings Events
    SettingChanged { key: String, value: serde_json::Value },
    ProfileChanged { profile_id: String },
}
```

### Hook Handler Interface

```rust
#[async_trait]
pub trait HookHandler: Send + Sync {
    /// Unique identifier for this handler
    fn id(&self) -> &str;

    /// Priority (lower = earlier execution)
    fn priority(&self) -> i32 { 100 }

    /// Which events this handler subscribes to
    fn subscribed_events(&self) -> Vec<HookEventType>;

    /// Handle an event, optionally modifying it
    async fn handle(&self, event: &mut HookEvent) -> HookResult;
}

pub enum HookResult {
    /// Continue to next handler
    Continue,
    /// Stop event propagation
    Stop,
    /// Event was modified, continue with modified version
    Modified,
}
```

### Event Bus Implementation

```rust
pub struct EventBus {
    handlers: RwLock<Vec<Arc<dyn HookHandler>>>,
    subscribers: RwLock<HashMap<HookEventType, Vec<Sender<HookEvent>>>>,
}

impl EventBus {
    /// Register a hook handler
    pub fn register(&self, handler: Arc<dyn HookHandler>);

    /// Unregister a hook handler
    pub fn unregister(&self, handler_id: &str);

    /// Emit an event through the hook pipeline
    pub async fn emit(&self, event: HookEvent) -> HookEvent;

    /// Subscribe to events (for UI updates)
    pub fn subscribe(&self, event_type: HookEventType) -> Receiver<HookEvent>;
}
```

---

## Plugin System Design

### Plugin Types

1. **Orb Style Plugins** - Visual representations of audio
2. **Audio Processor Plugins** - Modify audio before transcription
3. **Post-Processor Plugins** - Modify transcription output
4. **Integration Plugins** - Connect to external services

### Plugin Manifest (plugin.toml)

```toml
[plugin]
id = "nebula-aura"
name = "Nebula Aura"
version = "1.0.0"
description = "Cosmic nebula visualization with swirling particles"
author = "Zana Team"
license = "MIT"
homepage = "https://Zana.app/plugins/nebula-aura"

[plugin.type]
kind = "orb-style"

[plugin.capabilities]
# What this plugin can access
audio_level = true
audio_fft = true
transcription_events = false
settings_read = true
settings_write = false
network = false

[plugin.ui]
# For orb styles
default_width = 500
default_height = 500
transparent = true
resizable = true

[plugin.config]
# User-configurable options
[[plugin.config.options]]
key = "particle_density"
label = "Particle Density"
type = "number"
default = 1.0
min = 0.5
max = 2.0

[[plugin.config.options]]
key = "glow_intensity"
label = "Glow Intensity"
type = "number"
default = 1.0
min = 0.5
max = 2.0

[marketplace]
# Marketplace metadata
category = "visualization"
tags = ["cosmic", "nebula", "particles", "audio-reactive"]
preview_images = ["preview1.png", "preview2.png"]
```

### Plugin Trait (Rust)

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin metadata
    fn manifest(&self) -> &PluginManifest;

    /// Initialize the plugin
    async fn init(&mut self, context: PluginContext) -> Result<()>;

    /// Shutdown the plugin
    async fn shutdown(&mut self) -> Result<()>;

    /// Handle configuration changes
    fn on_config_change(&mut self, config: &PluginConfig);
}

/// Orb style plugin trait
#[async_trait]
pub trait OrbStylePlugin: Plugin {
    /// Get the rendering component (returns Canvas2D draw commands)
    fn render(&self, ctx: &RenderContext) -> Vec<DrawCommand>;

    /// Update animation state
    fn update(&mut self, dt: f32, audio_level: f32, fft: &[f32]);

    /// Handle window resize
    fn on_resize(&mut self, width: u32, height: u32);
}

/// Draw commands for Canvas2D rendering
pub enum DrawCommand {
    Clear,
    FillStyle(Color),
    StrokeStyle(Color),
    LineWidth(f32),
    BeginPath,
    Arc { x: f32, y: f32, radius: f32, start: f32, end: f32 },
    MoveTo(f32, f32),
    LineTo(f32, f32),
    Fill,
    Stroke,
    RadialGradient { cx: f32, cy: f32, r0: f32, r1: f32, stops: Vec<(f32, Color)> },
    // ... more as needed
}
```

### Plugin Distribution

Plugins are distributed as packages containing:

```
nebula-aura-1.0.0.Zana/
  plugin.toml          # Manifest
  plugin.wasm          # WASM binary (optional, for custom logic)
  assets/
    preview1.png       # Marketplace previews
    preview2.png
  src/
    render.js          # Canvas rendering code (JS for flexibility)
    style.css          # Optional styles
```

### Marketplace API

```rust
pub struct MarketplaceClient {
    base_url: String,
    api_key: Option<String>,
}

impl MarketplaceClient {
    /// Search plugins
    pub async fn search(&self, query: &str, category: Option<&str>) -> Vec<PluginListing>;

    /// Get plugin details
    pub async fn get_plugin(&self, id: &str) -> PluginDetails;

    /// Download plugin
    pub async fn download(&self, id: &str, version: &str) -> PathBuf;

    /// Get installed plugins
    pub fn installed(&self) -> Vec<InstalledPlugin>;

    /// Check for updates
    pub async fn check_updates(&self) -> Vec<PluginUpdate>;
}
```

---

## Directory Structure

```
Zana/
  Cargo.toml                 # Workspace manifest

  src/                        # Main Rust application
    main.rs                   # Application entry point (winit + egui)
    lib.rs                    # Library exports

    # GUI modules
    gui/
      mod.rs
      app.rs                  # Main egui app
      orb.rs                  # Orb visualization renderer
      settings.rs             # Settings panel
      marketplace.rs          # Marketplace UI (future)

    # Core modules
    audio/
      mod.rs
      capture.rs              # Audio capture (cpal)
      format.rs               # Format detection
      resample.rs             # Resampling utilities

    stt/
      mod.rs
      whisper.rs              # Whisper integration
      models.rs               # Model management

    hooks/
      mod.rs
      event.rs                # Event types
      bus.rs                  # Event bus
      handler.rs              # Handler trait

    plugins/
      mod.rs
      manager.rs              # Plugin lifecycle
      registry.rs             # Plugin registry
      sandbox.rs              # WASM sandboxing
      marketplace.rs          # Marketplace client

    state.rs                  # App state
    error.rs                  # Error types
    config.rs                 # Configuration

  plugins/                    # Built-in plugins
    nebula-aura-gpu/
      plugin.toml
      shaders/
        nebula.wgsl           # WGSL shader
      src/
        lib.rs                # Rust plugin implementation

  docs/
    ARCHITECTURE.md           # This file
    PLUGIN_DEVELOPMENT.md     # Plugin dev guide
    MIGRATION.md              # Migration from Tauri to egui
```

---

## Core Components Implementation

### 1. Audio Engine (Extracted from kollabor)

The audio engine handles cross-platform audio capture using cpal.

```rust
// src-tauri/src/audio/capture.rs

pub struct AudioCapture {
    event_bus: Arc<EventBus>,
    device: Option<cpal::Device>,
    stream: Option<cpal::Stream>,
    config: AudioConfig,
}

impl AudioCapture {
    pub fn new(event_bus: Arc<EventBus>) -> Self;

    /// List available input devices
    pub fn list_devices() -> Result<Vec<AudioDevice>>;

    /// Start capturing audio
    pub async fn start(&mut self, device_id: Option<&str>) -> Result<()> {
        // Emit hook event
        self.event_bus.emit(HookEvent::AudioCaptureStart {
            device_id: device_id.unwrap_or("default").to_string(),
            sample_rate: self.config.sample_rate,
        }).await;

        // Start capture...
    }

    /// Stop capturing
    pub async fn stop(&mut self) -> Result<AudioBuffer> {
        // Stop and emit event
        self.event_bus.emit(HookEvent::AudioCaptureStop {
            duration_ms: self.duration_ms(),
        }).await;

        // Return buffer...
    }
}
```

### 2. STT Engine (Extracted from kollabor)

The STT engine wraps whisper-rs with hook integration.

```rust
// src-tauri/src/stt/whisper.rs

pub struct WhisperEngine {
    event_bus: Arc<EventBus>,
    context_cache: Arc<Mutex<Option<CachedContext>>>,
    models_dir: PathBuf,
}

impl WhisperEngine {
    /// Transcribe audio buffer
    pub async fn transcribe(&self, audio: &AudioBuffer, model: WhisperModel) -> Result<Transcription> {
        // Emit start event
        self.event_bus.emit(HookEvent::TranscriptionStart {
            model: model.clone()
        }).await;

        // Load/get model context
        let ctx = self.get_or_load_context(model).await?;

        // Process audio
        let samples = self.prepare_samples(audio)?;

        // Run whisper
        let result = self.run_whisper(&ctx, &samples).await?;

        // Emit segment events
        for segment in &result.segments {
            self.event_bus.emit(HookEvent::TranscriptionSegment {
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                text: segment.text.clone(),
            }).await;
        }

        // Emit complete event
        self.event_bus.emit(HookEvent::TranscriptionComplete {
            text: result.text.clone(),
            segments: result.segments.clone(),
        }).await;

        Ok(result)
    }
}
```

### 3. Plugin Manager

```rust
// src-tauri/src/plugins/manager.rs

pub struct PluginManager {
    event_bus: Arc<EventBus>,
    registry: PluginRegistry,
    plugins_dir: PathBuf,
    marketplace: MarketplaceClient,
}

impl PluginManager {
    /// Load all installed plugins
    pub async fn load_all(&mut self) -> Result<()> {
        let plugin_dirs = self.discover_plugins()?;

        for dir in plugin_dirs {
            match self.load_plugin(&dir).await {
                Ok(plugin) => {
                    self.event_bus.emit(HookEvent::PluginLoaded {
                        id: plugin.manifest().id.clone(),
                        version: plugin.manifest().version.clone(),
                    }).await;

                    self.registry.register(plugin);
                }
                Err(e) => {
                    log::error!("Failed to load plugin from {:?}: {}", dir, e);
                }
            }
        }

        Ok(())
    }

    /// Install plugin from marketplace
    pub async fn install(&mut self, plugin_id: &str) -> Result<()> {
        let path = self.marketplace.download(plugin_id, "latest").await?;
        self.load_plugin(&path).await?;
        Ok(())
    }

    /// Uninstall plugin
    pub async fn uninstall(&mut self, plugin_id: &str) -> Result<()>;

    /// Get all orb style plugins
    pub fn orb_styles(&self) -> Vec<&dyn OrbStylePlugin>;
}
```

---

## GUI Architecture (egui + wgpu)

### Overview

Zana uses egui for immediate-mode GUI rendering combined with wgpu for high-performance GPU-accelerated orb visualization. This pure Rust approach eliminates the webview overhead and provides direct GPU access.

### Application Structure

The main application (`src/gui/app.rs`) coordinates all GUI components:

```rust
pub struct ZanaApp {
    /// Core application state
    state: Arc<AppState>,

    /// Orb renderer for GPU visualization
    orb_renderer: OrbRenderer,

    /// Settings panel
    settings_panel: SettingsPanel,

    /// Event handler for EventBus integration
    event_handler: GuiEventHandler,

    /// UI state
    recording_state: RecordingState,
    transcription_state: TranscriptionState,
    ui_state: UIState,

    /// Audio metrics for visualization
    audio_metrics: AudioMetrics,

    /// Channel bridge for async communication
    channels: GuiChannels,

    /// Task spawner for background operations
    task_spawner: AsyncTaskSpawner,

    /// Tokio runtime for async operations
    _runtime: tokio::runtime::Runtime,
}
```

### Async-to-Sync Bridge

Since egui is synchronous but audio/transcription are async, Zana uses a channel bridge:

```rust
/// Commands sent from GUI to async recording task
pub enum RecordingCommand {
    Start { device_id: Option<String> },
    Stop,
    QueryStatus,
}

/// Events sent from async recording task to GUI
pub enum RecordingEvent {
    Started { device_name: String, sample_rate: u32 },
    Stopped { sample_count: usize, duration_ms: u64 },
    MetricsUpdate { level: f32, peak: f32, fft_bins: Vec<f32> },
    Error(String),
}

/// Channel bundle for GUI-async communication
pub struct GuiChannels {
    pub recording_cmd_tx: UnboundedSender<RecordingCommand>,
    pub recording_event_rx: UnboundedReceiver<RecordingEvent>,
    pub transcription_cmd_tx: UnboundedSender<TranscriptionCommand>,
    pub transcription_event_rx: UnboundedReceiver<TranscriptionEvent>,
}
```

### Orb Rendering with wgpu

The orb visualization (`src/gui/orb.rs`) uses wgpu for GPU-accelerated rendering:

```rust
pub struct OrbRenderer {
    /// wgpu device
    device: Rc<wgpu::Device>,

    /// wgpu queue
    queue: Rc<wgpu::Queue>,

    /// Render pipeline
    pipeline: wgpu::RenderPipeline,

    /// Uniform buffer
    uniform_buffer: wgpu::Buffer,

    /// Bind group
    bind_group: wgpu::BindGroup,

    /// FFT texture (1D texture with 32 bins)
    fft_texture: wgpu::Texture,

    /// FFT texture view
    fft_texture_view: wgpu::TextureView,

    /// FFT sampler
    fft_sampler: wgpu::Sampler,

    /// Current uniform data
    uniforms: Uniforms,

    /// Start time for animation
    start_time: std::time::Instant,

    /// Current audio level
    audio_level: f32,

    /// Current audio peak
    audio_peak: f32,

    /// Current FFT data
    fft_data: [f32; 32],
}

/// Uniform buffer data matching the shader's Uniforms struct
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    audio_level: f32,
    audio_peak: f32,
    cloud_count: f32,
    particle_count: f32,
    glow_intensity: f32,
    rotation_speed: f32,
    color_scheme: f32,
    quality: f32,
    _padding: [f32; 2],
}
```

### wgpu Initialization Flow

1. **Create Instance**: `wgpu::Instance::new()` with all backends
2. **Request Adapter**: High-performance GPU (integrated or discrete)
3. **Create Device/Queue**: Primary device for GPU operations
4. **Load Shader**: WGSL shader from `plugins/nebula-aura-gpu/src/shaders/nebula.wgsl`
5. **Create Buffers**: Uniform buffer + FFT texture
6. **Create Pipeline**: Vertex + fragment shader stages
7. **Create Bind Groups**: Connect shaders to GPU resources

### Audio Data Flow to GPU

1. **Audio Capture** (cpal) -> samples
2. **FFT Processing** -> 32 frequency bins
3. **Metrics Update** -> level, peak, FFT data
4. **Uniform Update** -> write to GPU buffer
5. **FFT Texture Update** -> write 1D texture
6. **Render Pass** -> execute WGSL shader

### egui Event Loop

```rust
impl eframe::App for ZanaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Poll channel events
        while let Ok(event) = self.channels.recording_event_rx.try_recv() {
            // Update recording state
        }

        // 2. Process EventBus events
        self.process_events_from_bus(ctx);

        // 3. Update audio metrics
        self.update_metrics().await;

        // 4. Render orb visualization
        self.render_orb(ctx);

        // 5. Render controls
        self.render_controls(ctx);

        // 6. Render panels (settings, transcription)
        if self.ui_state.show_settings {
            self.render_settings_panel(ctx);
        }

        // 7. Request repaint for smooth animation
        ctx.request_repaint();
    }
}
```

### EventBus Integration

The GUI subscribes to EventBus events for real-time updates:

```rust
fn process_events_from_bus(&mut self, ctx: &egui::Context) {
    self.event_handler.process_pending(|event| {
        match event {
            HookEvent::AudioLevelChange { level, peak } => {
                self.audio_metrics.level = *level;
                self.audio_metrics.peak = *peak;
                ctx.request_repaint();
            }

            HookEvent::AudioFftReady { bins, .. } => {
                self.audio_metrics.fft_bins = bins.clone();
                ctx.request_repaint();
            }

            HookEvent::TranscriptionComplete { text, .. } => {
                self.transcription_state.last_result = Some(...);
                self.ui_state.show_transcription = true;
                ctx.request_repaint();
            }

            // ... more events
        }
    });
}
```

### UI Components

1. **Control Panel** (`src/gui/app.rs`):
   - Record/Stop button
   - Settings button
   - Transcribe button
   - Audio level indicator

2. **Orb Visualization** (`src/gui/orb.rs`):
   - GPU-accelerated rendering
   - Real-time audio reactivity
   - Configurable styles (via color scheme uniform)

3. **Settings Panel** (`src/gui/settings.rs`):
   - Audio device selection
   - Whisper model selection
   - Orb style selection
   - Plugin configuration

4. **Transcription Panel** (`src/gui/app.rs`):
   - Display transcription results
   - Progress indicator
   - Error messages

---

## wgpu Rendering Pipeline

### Pipeline Architecture

Zana uses a direct wgpu rendering pipeline for the orb visualization, bypassing egui for the GPU-intensive rendering:

```
Audio Data (samples)
       |
       v
FFT Analysis (32 bins)
       |
       v
Uniform Update (CPU -> GPU)
       |
       v
FFT Texture Update (1D texture)
       |
       v
Render Pass
       |
       +-> Vertex Shader (fullscreen triangle)
       |
       v
Fragment Shader (nebula.wgsl)
       |
       v
Swap Chain -> Screen
```

### WGSL Shader Structure

The shader (`plugins/nebula-aura-gpu/src/shaders/nebula.wgsl`) receives:

**Uniforms (std140 layout)**:
- `resolution: vec2<f32>` - Canvas dimensions
- `time: f32` - Animation time
- `audio_level: f32>` - Current audio level (0-1)
- `audio_peak: f32>` - Peak audio level (smoothed)
- `cloud_count: f32>` - Number of cloud layers
- `particle_count: f32>` - Particle density
- `glow_intensity: f32>` - Glow brightness
- `rotation_speed: f32>` - Animation speed
- `color_scheme: f32>` - 0=purple, 1=cyan, 2=fire, 3=aurora, 4=cosmic
- `quality: f32>` - Render quality level

**FFT Texture (1D R32Float)**:
- 32 frequency bins
- Sampled in shader for frequency-based effects

### Render Pass Configuration

```rust
// Create render pipeline
let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    label: Some("Nebula Pipeline"),
    layout: Some(&pipeline_layout),
    vertex: wgpu::VertexState {
        module: &shader_module,
        entry_point: Some("vs_main"),
        buffers: &[],
    },
    fragment: Some(wgpu::FragmentState {
        module: &shader_module,
        entry_point: Some("fs_main"),
        targets: &[Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })],
    }),
    primitive: wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        strip_index_format: None,
        front_face: wgpu::FrontFace::Ccw,
        cull_mode: None,
        polygon_mode: wgpu::PolygonMode::Fill,
        ..Default::default()
    },
    depth_stencil: None,
    multisample: wgpu::MultisampleState::default(),
    multiview: None,
});
```

### Color Schemes

The shader supports 5 built-in color schemes controlled by the `color_scheme` uniform:

| Scheme | Value | Colors |
|--------|-------|--------|
| Purple | 0.0 | Purple, violet, magenta |
| Cyan | 1.0 | Cyan, blue, teal |
| Fire | 2.0 | Red, orange, yellow |
| Aurora | 3.0 | Green, blue, purple gradient |
| Cosmic | 4.0 | Deep space, stars, nebula |

### Performance Considerations

1. **Fullscreen Triangle**: Uses 3 vertices instead of quad (2 triangles)
2. **FFT Texture**: 1D texture with nearest filtering (R32Float non-filterable)
3. **Uniform Buffer**: Single buffer updated per-frame (48 bytes)
4. **Alpha Blending**: Premultiplied alpha for proper transparency
5. **No Depth Buffer**: 2D rendering doesn't need depth testing

---

## Platform-Specific Considerations

### macOS
- NSPanel for floating transparent window
- Native menu bar integration
- Accessibility permissions for audio capture

### Windows
- WS_EX_LAYERED for window transparency
- Per-monitor DPI awareness
- WASAPI audio backend via cpal

### Linux
- X11/Wayland compositor support for transparency
- PulseAudio/PipeWire audio backend
- XDG paths for configuration

---

## Migration from Tauri to egui

See [MIGRATION.md](./MIGRATION.md) for complete migration guide.

### Key Changes

**Removed:**
- Tauri 2 framework
- src-ui/ directory (HTML/CSS/JS)
- webview and Chromium
- Tauri commands (invoke/emit pattern)
- tauri.conf.json and capabilities

**Added:**
- winit for window management
- egui for immediate-mode GUI
- wgpu for GPU rendering
- Direct function calls (no IPC)

**Preserved:**
- All Rust backend code (audio, STT, hooks, plugins)
- WGSL shaders (wgpu uses same format)
- Hook system architecture
- Plugin system design

---

## Development Phases

### Phase 1: Core Foundation ✓ DONE
- [x] Implement hook system (EventBus, HookHandler)
- [x] Port audio capture from kollabor
- [x] Port whisper.rs from kollabor
- [x] Verify compilation and basic functionality

### Phase 2: egui Migration (CURRENT)
- [ ] Remove Tauri dependencies
- [ ] Add winit + egui + wgpu
- [ ] Create main.rs with egui app
- [ ] Port orb visualization to wgpu (Rust)
- [ ] Implement transparent window
- [ ] Build UI controls (record button, settings)

### Phase 3: Plugin System
- [ ] Update plugin traits for egui
- [ ] Build NebulaAuraOrb as Rust plugin
- [ ] Implement plugin hot reload
- [ ] Add plugin configuration UI

### Phase 4: Marketplace
- [ ] Design marketplace API
- [ ] Implement client
- [ ] Build egui marketplace UI
- [ ] Add update mechanism

### Phase 5: Polish
- [ ] Cross-platform testing
- [ ] Performance optimization
- [ ] Documentation
- [ ] Beta release

---

## Contributing

See [PLUGIN_DEVELOPMENT.md](./PLUGIN_DEVELOPMENT.md) for plugin development guidelines.
