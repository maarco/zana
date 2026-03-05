# Zana Plugin Development Guide

This guide explains how to create plugins for Zana. All plugins are written in Rust and use wgpu for GPU rendering.

---

## Plugin Types

Zana supports several plugin types:

| Type | Description | Capabilities |
|------|-------------|--------------|
| `orb-style` | Visual audio representation (wgpu or Canvas2D) | Audio level, FFT, settings |
| `audio-processor` | Modify audio before STT | Raw audio buffer access |
| `post-processor` | Modify transcription output | Transcription text access |
| `integration` | External service connections | Network, settings |

---

## Quick Start: Orb Style Plugin

### Overview

Orb style plugins create visual representations of audio. You have two rendering options:

1. **Canvas2D (DrawCommand)** - CPU-based drawing via declarative commands
2. **wgpu (Direct GPU)** - GPU-accelerated rendering with WGSL shaders

### Option 1: Canvas2D Plugin (Simple)

Canvas2D plugins return a list of draw commands that Zana executes on a 2D canvas.

### 1. Create Plugin Crate

```bash
cargo new --lib my-orb-style
cd my-orb-style
```

### 2. Add Dependencies to Cargo.toml

```toml
[package]
name = "my-orb-style"
version = "0.1.0"
edition = "2021"

[dependencies]
Zana = { path = "../.." }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
anyhow = "1.0"
```

### 3. Create Manifest (plugin.toml)

```toml
[plugin]
id = "my-orb-style"
name = "My Orb Style"
version = "1.0.0"
description = "A custom audio visualization"
author = "Your Name"
license = "MIT"

[plugin.type]
kind = "orb-style"

[plugin.capabilities]
audio_level = true
audio_fft = true

[plugin.ui]
default_width = 400
default_height = 400
transparent = true
resizable = true

[[plugin.config.options]]
key = "color_primary"
label = "Primary Color"
type = "color"
default = "#8B5CF6"

[[plugin.config.options]]
key = "particle_count"
label = "Particle Count"
type = "number"
default = 50
min = 10
max = 200

[marketplace]
category = "visualization"
tags = ["particles", "audio-reactive"]
```

### 4. Create Plugin (src/lib.rs)

```rust
use Zana::plugins::{
    DrawCommand, Color, OrbStylePlugin, Plugin, PluginContext, PluginManifest,
    RenderContext,
};
use async_trait::async_trait;
use std::collections::HashMap;
use serde_json::Value;

pub struct MyOrbStyle {
    manifest: PluginManifest,
    config: HashMap<String, Value>,
    particle_count: usize,
    primary_color: Color,
}

impl MyOrbStyle {
    pub fn new(manifest: PluginManifest) -> Self {
        Self {
            manifest,
            config: HashMap::new(),
            particle_count: 50,
            primary_color: Color::css("#8B5CF6"),
        }
    }
}

#[async_trait]
impl Plugin for MyOrbStyle {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn init(&mut self, ctx: PluginContext) -> anyhow::Result<()> {
        // Load configuration
        self.config = ctx.config;
        Ok(())
    }

    async fn shutdown(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_config_change(&mut self, config: &HashMap<String, Value>) {
        // Update particle count from config
        if let Some(count) = config.get("particle_count") {
            if let Some(n) = count.as_u64() {
                self.particle_count = n as usize;
            }
        }

        // Update color from config
        if let Some(color) = config.get("color_primary") {
            if let Some(s) = color.as_str() {
                self.primary_color = Color::css(s);
            }
        }
    }
}

impl OrbStylePlugin for MyOrbStyle {
    fn update(&mut self, ctx: &RenderContext) {
        // Animation state updates go here
        // Use ctx.time and ctx.dt for frame-independent animation
    }

    fn render(&self, ctx: &RenderContext) -> Vec<DrawCommand> {
        let mut cmds = Vec::new();

        // Clear canvas
        cmds.push(DrawCommand::Clear);

        // Get center point
        let cx = ctx.cx;
        let cy = ctx.cy;

        // Draw pulsing circle based on audio level
        let radius = 50.0 + ctx.audio_level * 100.0;

        cmds.push(DrawCommand::FillStyle {
            color: self.primary_color.clone(),
        });
        cmds.push(DrawCommand::BeginPath);
        cmds.push(DrawCommand::Arc {
            x: cx,
            y: cy,
            radius,
            start_angle: 0.0,
            end_angle: std::f32::consts::PI * 2.0,
        });
        cmds.push(DrawCommand::Fill);

        // Draw particles
        for i in 0..self.particle_count {
            let angle = (i as f32 / self.particle_count as f32) * std::f32::consts::PI * 2.0;
            let dist = radius + 20.0 + (ctx.time * 50.0 + i as f32 * 10.0).sin() * 10.0;
            let px = cx + angle.cos() * dist;
            let py = cy + angle.sin() * dist;

            cmds.push(DrawCommand::FillStyle {
                color: Color::rgba(200, 200, 255, 0.7),
            });
            cmds.push(DrawCommand::BeginPath);
            cmds.push(DrawCommand::Arc {
                x: px,
                y: py,
                radius: 3.0 + ctx.audio_level * 3.0,
                start_angle: 0.0,
                end_angle: std::f32::consts::PI * 2.0,
            });
            cmds.push(DrawCommand::Fill);
        }

        cmds
    }

    fn on_resize(&mut self, width: u32, height: u32) {
        // Handle resize if needed
    }
}
```

---

## Option 2: wgpu Plugin (Advanced)

For high-performance GPU rendering, use wgpu directly with WGSL shaders.

### 1. Create Plugin Structure

Same as Canvas2D, but add wgpu dependency:

```toml
[dependencies]
Zana = { path = "../.." }
wgpu = "0.18"
bytemuck = { version = "1.14", features = ["derive"] }
```

### 2. Create WGSL Shader (shaders/orb.wgsl)

```wgsl
struct Uniforms {
    resolution: vec2<f32>,
    time: f32,
    audio_level: f32,
    audio_peak: f32,
    color_scheme: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    var out: VertexOutput;
    out.position = vec4<f32>(pos[vi], 0.0, 1.0);
    out.uv = pos[vi] * 0.5 + 0.5;
    out.uv.y = 1.0 - out.uv.y;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let d = length(in.uv - center);

    // Pulsing circle
    let radius = 0.2 + u.audio_level * 0.3;
    let circle = smoothstep(radius + 0.05, radius, d);

    // Purple color
    let color = vec3<f32>(0.5, 0.2, 0.6);

    return vec4<f32>(color * circle, circle);
}
```

### 3. Create Plugin (src/lib.rs)

```rust
use Zana::plugins::{Plugin, PluginContext, PluginManifest, RenderContext};
use async_trait::async_trait;
use std::collections::HashMap;
use std::rc::Rc;
use wgpu::util::DeviceExt;
use serde_json::Value;

pub struct MyGpuOrb {
    manifest: PluginManifest,
    device: Rc<wgpu::Device>,
    queue: Rc<wgpu::Queue>,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    start_time: std::time::Instant,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    audio_level: f32,
    audio_peak: f32,
    color_scheme: f32,
    _padding: [f32; 3],
}

impl MyGpuOrb {
    pub async fn new(manifest: PluginManifest) -> anyhow::Result<Self> {
        // Initialize wgpu
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }).await.ok_or_else(|| anyhow::anyhow!("No adapter found"))?;

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor::default(),
            None,
        ).await?;

        let device = Rc::new(device);
        let queue = Rc::new(queue);

        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("My Orb Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/orb.wgsl").into()),
        });

        // Create uniform buffer
        let uniforms = Uniforms {
            resolution: [400.0, 400.0],
            time: 0.0,
            audio_level: 0.0,
            audio_peak: 0.0,
            color_scheme: 0.0,
            _padding: [0.0; 3],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group layout and bind group
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Ok(Self {
            manifest,
            device,
            queue,
            pipeline,
            uniform_buffer,
            bind_group,
            start_time: std::time::Instant::now(),
        })
    }
}

#[async_trait]
impl Plugin for MyGpuOrb {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn init(&mut self, _ctx: PluginContext) -> anyhow::Result<()> {
        Ok(())
    }

    async fn shutdown(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_config_change(&mut self, _config: &HashMap<String, Value>) {
        // Handle config changes
    }
}

impl OrbStylePlugin for MyGpuOrb {
    fn update(&mut self, ctx: &RenderContext) {
        // Update uniforms based on audio data
        let uniforms = Uniforms {
            resolution: [ctx.width, ctx.height],
            time: self.start_time.elapsed().as_secs_f32(),
            audio_level: ctx.audio_level,
            audio_peak: ctx.audio_level, // Simplified
            color_scheme: 0.0,
            _padding: [0.0; 3],
        };

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[uniforms]),
        );
    }

    fn render(&self, _ctx: &RenderContext) -> Vec<DrawCommand> {
        // For GPU plugins, return empty - rendering handled via wgpu pipeline
        Vec::new()
    }

    fn on_resize(&mut self, width: u32, height: u32) {
        // Update resolution in uniforms
    }
}
```

---

## Plugin API Reference

### Base Plugin Trait

All plugins must implement the `Plugin` trait:

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get the plugin manifest
    fn manifest(&self) -> &PluginManifest;

    /// Initialize the plugin with context
    async fn init(&mut self, context: PluginContext) -> anyhow::Result<()>;

    /// Shutdown the plugin
    async fn shutdown(&mut self) -> anyhow::Result<()>;

    /// Handle configuration changes
    fn on_config_change(&mut self, config: &HashMap<String, JsonValue>);

    /// Get the plugin's hook handler (optional)
    fn hook_handler(&self) -> Option<Arc<dyn HookHandler>> {
        None
    }

    /// Check if plugin has a specific capability
    fn has_capability(&self, cap: &str) -> bool;
}
```

### Plugin Context

Passed to `init()` method:

```rust
pub struct PluginContext {
    /// The event bus for subscribing to events
    pub event_bus: Arc<EventBus>,

    /// Plugin's data directory
    pub data_dir: std::path::PathBuf,

    /// Current configuration
    pub config: HashMap<String, JsonValue>,

    /// Window dimensions (for orb-style plugins)
    pub width: u32,
    pub height: u32,
}
```

### Render Context

Passed to `update()` and `render()` methods:

```rust
pub struct RenderContext {
    /// Canvas width
    pub width: f32,
    /// Canvas height
    pub height: f32,
    /// Center X
    pub cx: f32,
    /// Center Y
    pub cy: f32,
    /// Current time in seconds
    pub time: f32,
    /// Delta time since last frame
    pub dt: f32,
    /// Current audio level (0.0 - 1.0)
    pub audio_level: f32,
    /// FFT bins (frequency data)
    pub fft_bins: Vec<f32>,
    /// Whether currently recording
    pub is_recording: bool,
    /// Device pixel ratio
    pub dpr: f32,
}
```

### DrawCommand Enum

Canvas2D drawing commands:

| Command | Description |
|---------|-------------|
| `Clear` | Clear the canvas |
| `FillStyle { color }` | Set fill color |
| `StrokeStyle { color }` | Set stroke color |
| `LineWidth { width }` | Set line width |
| `BeginPath` | Start a new path |
| `MoveTo { x, y }` | Move to point |
| `LineTo { x, y }` | Line to point |
| `Arc { x, y, radius, start_angle, end_angle }` | Draw arc |
| `Fill` | Fill current path |
| `Stroke` | Stroke current path |
| `FillRect { x, y, width, height }` | Fill rectangle |
| `FillText { text, x, y }` | Draw text |
| `SetFillLinearGradient { ... }` | Linear gradient |
| `SetFillRadialGradient { ... }` | Radial gradient |
| `Translate { x, y }` | Translate origin |
| `Rotate { angle }` | Rotate |
| `Scale { x, y }` | Scale |

---

## Audio Data

### Audio Level

- **Range**: 0.0 to 1.0
- **Description**: Smoothed average of audio amplitude
- **Update Rate**: ~60fps

### FFT Data

- **Format**: `Vec<f32>` with 32 frequency bins
- **Range**: 0.0 to 1.0 per bin
- **Layout**: Low frequencies at index 0, high at index 31
- **Use Case**: Frequency-based visualizations

---

## Configuration Types

In `plugin.toml`, you can define user-configurable options:

```toml
# Number with range
[[plugin.config.options]]
key = "speed"
label = "Animation Speed"
type = "number"
default = 1.0
min = 0.1
max = 5.0

# Color picker
[[plugin.config.options]]
key = "color"
label = "Primary Color"
type = "color"
default = "#8B5CF6"

# Boolean toggle
[[plugin.config.options]]
key = "show_particles"
label = "Show Particles"
type = "boolean"
default = true

# Selection dropdown
[[plugin.config.options]]
key = "mode"
label = "Render Mode"
type = "select"
default = "standard"
options = ["standard", "minimal", "intense"]
```

Access in code:

```rust
fn on_config_change(&mut self, config: &HashMap<String, Value>) {
    if let Some(speed) = config.get("speed") {
        if let Some(n) = speed.as_f64() {
            self.speed = n as f32;
        }
    }
}
```

---

## Hook System Integration

Plugins can subscribe to Zana events via the EventBus.

### Subscribing to Events

```rust
async fn init(&mut self, ctx: PluginContext) -> anyhow::Result<()> {
    // Subscribe to transcription events
    let mut rx = ctx.event_bus.subscribe(HookEventType::TranscriptionComplete).await?;

    // Spawn task to handle events
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                HookEvent::TranscriptionComplete { text, .. } => {
                    log::info!("Transcription: {}", text);
                }
                _ => {}
            }
        }
    });

    Ok(())
}
```

### Available Events

| Event | Payload | Description |
|-------|---------|-------------|
| `AudioCaptureStart` | `{ device_id, sample_rate }` | Recording started |
| `AudioCaptureStop` | `{ duration_ms }` | Recording stopped |
| `AudioLevelChange` | `{ level, peak }` | Audio level update |
| `AudioFftReady` | `{ bins }` | FFT data ready |
| `TranscriptionStart` | `{ model }` | STT processing began |
| `TranscriptionProgress` | `{ percent }` | Progress update |
| `TranscriptionComplete` | `{ text, segments }` | Full transcription |
| `TranscriptionError` | `{ error }` | Transcription failed |
| `SettingChanged` | `{ key, value }` | User changed setting |
| `Error` | `{ code, message }` | Error occurred |

---

## Testing Your Plugin

### Local Development

1. Build plugin:
   ```bash
   cd my-orb-style
   cargo build
   ```

2. Copy to Zana plugins directory:
   ```bash
   cp -r . ~/.Zana/plugins/my-orb-style
   ```

3. Restart Zana

4. Select your plugin from Settings > Orb Style

### Debug Mode

Enable logging:

```bash
RUST_LOG=debug Zana
```

Check plugin output:

```bash
Zana --dev
```

---

## Publishing to Marketplace

### 1. Prepare Assets

- `preview.png` - 800x600 preview image (required)
- `icon.svg` - 64x64 plugin icon (optional)
- `README.md` - Documentation (optional)

### 2. Validate Plugin

```bash
Zana plugin validate ./my-orb-style
```

### 3. Package

```bash
Zana plugin pack ./my-orb-style
# Creates: my-orb-style-1.0.0.Zana
```

### 4. Submit

```bash
Zana plugin publish ./my-orb-style-1.0.0.Zana
```

---

## Best Practices

### Performance

1. **Minimize allocations in render loop**
   ```rust
   // Bad: Creates new Vec every frame
   fn render(&self, ctx: &RenderContext) -> Vec<DrawCommand> {
       let mut cmds = Vec::new();
       cmds.push(DrawCommand::Clear);
       // ...
   }

   // Good: Pre-allocate if possible
   struct MyOrb {
       commands_cache: Vec<DrawCommand>,
   }
   ```

2. **Use request_repaint sparingly**
   - Only call when needed
   - Audio updates already trigger repaints

3. **Batch canvas operations**
   ```rust
   // Good: Single path for multiple shapes
   cmds.push(DrawCommand::BeginPath);
   for p in &particles {
       cmds.push(DrawCommand::Arc { ... });
   }
   cmds.push(DrawCommand::Fill);
   ```

### Visual Design

1. **Support transparency** - Window background is transparent
2. **Respect DPI** - Use `ctx.dpr` for retina displays
3. **Smooth animations** - Use `ctx.dt` for frame-rate independent animation
4. **Audio responsiveness** - React to `ctx.audio_level` meaningfully

### Accessibility

1. **Reduce motion option** - Check `config.reduce_motion` if available
2. **Color contrast** - Ensure visibility against backgrounds
3. **No seizure-inducing patterns** - Avoid rapid flashing

---

## Example Plugins

### Minimal Pulsing Orb

```rust
impl OrbStylePlugin for MinimalOrb {
    fn render(&self, ctx: &RenderContext) -> Vec<DrawCommand> {
        vec![
            DrawCommand::Clear,
            DrawCommand::FillStyle {
                color: Color::css("#8B5CF6"),
            },
            DrawCommand::BeginPath,
            DrawCommand::Arc {
                x: ctx.cx,
                y: ctx.cy,
                radius: 30.0 + ctx.audio_level * 50.0,
                start_angle: 0.0,
                end_angle: std::f32::consts::PI * 2.0,
            },
            DrawCommand::Fill,
        ]
    }
}
```

### Frequency Bars

```rust
impl OrbStylePlugin for FrequencyBars {
    fn render(&self, ctx: &RenderContext) -> Vec<DrawCommand> {
        let mut cmds = vec![DrawCommand::Clear];

        let bar_width = ctx.width / ctx.fft_bins.len() as f32;

        for (i, &bin) in ctx.fft_bins.iter().enumerate() {
            let bar_height = bin * ctx.height * 0.8;
            let x = i as f32 * bar_width;
            let y = ctx.height - bar_height;

            let hue = (i as f32 / ctx.fft_bins.len() as f32) * 360.0;
            cmds.push(DrawCommand::FillStyle {
                color: Color::css(&format!("hsl({}, 80%, 60%)", hue)),
            });
            cmds.push(DrawCommand::FillRect {
                x,
                y,
                width: bar_width - 2.0,
                height: bar_height,
            });
        }

        cmds
    }
}
```

---

## Support

- Documentation: https://Zana.app/docs/plugins
- Discord: https://discord.gg/Zana
- Issues: https://github.com/Zana/Zana/issues
