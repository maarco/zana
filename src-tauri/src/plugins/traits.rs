//! Plugin Traits
//!
//! Defines the trait interfaces that plugins must implement.

use super::manifest::PluginManifest;
use crate::hooks::{EventBus, HookHandler};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

/// Plugin context provided during initialization
#[derive(Clone)]
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

/// Render context for orb-style plugins
#[derive(Debug, Clone)]
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

/// Base plugin trait that all plugins must implement
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

    /// Get the plugin's hook handler (if it provides one)
    fn hook_handler(&self) -> Option<Arc<dyn HookHandler>> {
        None
    }

    /// Check if plugin has a specific capability
    fn has_capability(&self, cap: &str) -> bool {
        let caps = &self.manifest().plugin.capabilities;
        match cap {
            "audio_level" => caps.audio_level,
            "audio_fft" => caps.audio_fft,
            "audio_buffer" => caps.audio_buffer,
            "transcription_events" => caps.transcription_events,
            "transcription_modify" => caps.transcription_modify,
            "settings_read" => caps.settings_read,
            "settings_write" => caps.settings_write,
            "network" => caps.network,
            "filesystem" => caps.filesystem,
            _ => false,
        }
    }
}

/// Orb style plugin trait for audio visualizations
#[async_trait]
pub trait OrbStylePlugin: Plugin {
    /// Update animation state
    ///
    /// Called every frame before render.
    fn update(&mut self, ctx: &RenderContext);

    /// Get draw commands for the current frame
    ///
    /// Returns a list of Canvas 2D draw commands.
    fn render(&self, ctx: &RenderContext) -> Vec<DrawCommand>;

    /// Handle window resize
    fn on_resize(&mut self, width: u32, height: u32);
}

/// Audio processor plugin trait
#[async_trait]
pub trait AudioProcessorPlugin: Plugin {
    /// Process audio buffer before transcription
    ///
    /// Can modify the audio in place.
    async fn process(&mut self, samples: &mut Vec<f32>, sample_rate: u32) -> anyhow::Result<()>;
}

/// Post-processor plugin trait
#[async_trait]
pub trait PostProcessorPlugin: Plugin {
    /// Process transcription text after STT
    ///
    /// Returns modified text.
    async fn process(&mut self, text: &str) -> anyhow::Result<String>;
}

/// Integration plugin trait
#[async_trait]
pub trait IntegrationPlugin: Plugin {
    /// Called when transcription completes
    async fn on_transcription(&mut self, text: &str) -> anyhow::Result<()>;
}

/// Canvas 2D draw commands
///
/// These commands are serialized and sent to the frontend
/// to be executed on a Canvas 2D context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args")]
pub enum DrawCommand {
    // Context state
    Save,
    Restore,

    // Clear
    Clear,
    ClearRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },

    // Transform
    Translate {
        x: f32,
        y: f32,
    },
    Rotate {
        angle: f32,
    },
    Scale {
        x: f32,
        y: f32,
    },

    // Style
    FillStyle {
        color: Color,
    },
    StrokeStyle {
        color: Color,
    },
    LineWidth {
        width: f32,
    },
    LineCap {
        cap: LineCap,
    },
    LineJoin {
        join: LineJoin,
    },
    GlobalAlpha {
        alpha: f32,
    },
    GlobalCompositeOperation {
        op: CompositeOp,
    },
    ShadowColor {
        color: Color,
    },
    ShadowBlur {
        blur: f32,
    },
    ShadowOffsetX {
        offset: f32,
    },
    ShadowOffsetY {
        offset: f32,
    },

    // Path
    BeginPath,
    ClosePath,
    MoveTo {
        x: f32,
        y: f32,
    },
    LineTo {
        x: f32,
        y: f32,
    },
    Arc {
        x: f32,
        y: f32,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
    },
    ArcTo {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        radius: f32,
    },
    QuadraticCurveTo {
        cpx: f32,
        cpy: f32,
        x: f32,
        y: f32,
    },
    BezierCurveTo {
        cp1x: f32,
        cp1y: f32,
        cp2x: f32,
        cp2y: f32,
        x: f32,
        y: f32,
    },
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },

    // Drawing
    Fill,
    Stroke,
    FillRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    StrokeRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },

    // Gradients
    SetFillRadialGradient {
        x0: f32,
        y0: f32,
        r0: f32,
        x1: f32,
        y1: f32,
        r1: f32,
        stops: Vec<GradientStop>,
    },
    SetFillLinearGradient {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        stops: Vec<GradientStop>,
    },
    SetStrokeRadialGradient {
        x0: f32,
        y0: f32,
        r0: f32,
        x1: f32,
        y1: f32,
        r1: f32,
        stops: Vec<GradientStop>,
    },
    SetStrokeLinearGradient {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        stops: Vec<GradientStop>,
    },

    // Text
    FillText {
        text: String,
        x: f32,
        y: f32,
    },
    StrokeText {
        text: String,
        x: f32,
        y: f32,
    },
    Font {
        font: String,
    },
    TextAlign {
        align: TextAlign,
    },
    TextBaseline {
        baseline: TextBaseline,
    },
}

use serde::{Deserialize, Serialize};

/// Color representation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Color {
    /// CSS color string (e.g., "red", "#ff0000", "rgba(255, 0, 0, 0.5)")
    Css(String),
    /// RGBA values (0-255 for RGB, 0.0-1.0 for A)
    Rgba {
        r: u8,
        g: u8,
        b: u8,
        a: f32,
    },
}

impl Color {
    /// Create from RGBA values
    pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Color::Rgba { r, g, b, a }
    }

    /// Create from RGB values (opaque)
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color::Rgba { r, g, b, a: 1.0 }
    }

    /// Create from CSS string
    pub fn css(s: impl Into<String>) -> Self {
        Color::Css(s.into())
    }

    /// Convert to CSS string
    pub fn to_css(&self) -> String {
        match self {
            Color::Css(s) => s.clone(),
            Color::Rgba { r, g, b, a } => {
                if *a >= 1.0 {
                    format!("rgb({}, {}, {})", r, g, b)
                } else {
                    format!("rgba({}, {}, {}, {})", r, g, b, a)
                }
            }
        }
    }
}

/// Gradient color stop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

/// Line cap style
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// Line join style
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// Composite operation
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompositeOp {
    SourceOver,
    SourceIn,
    SourceOut,
    SourceAtop,
    DestinationOver,
    DestinationIn,
    DestinationOut,
    DestinationAtop,
    Lighter,
    Copy,
    Xor,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

/// Text alignment
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    Left,
    Right,
    Center,
    Start,
    End,
}

/// Text baseline
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextBaseline {
    Top,
    Hanging,
    Middle,
    Alphabetic,
    Ideographic,
    Bottom,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_to_css() {
        assert_eq!(Color::rgb(255, 0, 0).to_css(), "rgb(255, 0, 0)");
        assert_eq!(Color::rgba(255, 0, 0, 0.5).to_css(), "rgba(255, 0, 0, 0.5)");
        assert_eq!(Color::css("red").to_css(), "red");
    }

    #[test]
    fn test_draw_command_serialization() {
        let cmd = DrawCommand::Arc {
            x: 100.0,
            y: 100.0,
            radius: 50.0,
            start_angle: 0.0,
            end_angle: std::f32::consts::PI * 2.0,
        };

        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("Arc"));
        assert!(json.contains("100"));
    }
}
