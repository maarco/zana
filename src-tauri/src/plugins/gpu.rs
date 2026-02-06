//! GPU Plugin Support
//!
//! Types and traits for GPU-accelerated visualization plugins.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// GPU renderer type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuRendererType {
    /// Canvas 2D (CPU-based, fallback)
    Canvas2D,
    /// WebGL 2.0
    WebGL2,
    /// WebGPU (preferred)
    WebGPU,
}

impl Default for GpuRendererType {
    fn default() -> Self {
        GpuRendererType::WebGPU
    }
}

/// GPU capabilities detected at runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuCapabilities {
    /// WebGPU available
    pub webgpu: bool,
    /// WebGL2 available
    pub webgl2: bool,
    /// WebGL1 available (legacy)
    pub webgl1: bool,
    /// GPU vendor
    pub vendor: Option<String>,
    /// GPU renderer string
    pub renderer: Option<String>,
    /// Maximum texture size
    pub max_texture_size: u32,
    /// Supports compute shaders
    pub compute_shaders: bool,
    /// Supports storage buffers
    pub storage_buffers: bool,
}

impl Default for GpuCapabilities {
    fn default() -> Self {
        Self {
            webgpu: false,
            webgl2: true, // Usually available
            webgl1: true,
            vendor: None,
            renderer: None,
            max_texture_size: 4096,
            compute_shaders: false,
            storage_buffers: false,
        }
    }
}

/// Shader source for GPU plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderSource {
    /// Shader name/identifier
    pub name: String,
    /// WGSL source code (for WebGPU)
    pub wgsl: Option<String>,
    /// GLSL source code (for WebGL2 fallback)
    pub glsl_vertex: Option<String>,
    pub glsl_fragment: Option<String>,
    /// Entry point names
    pub vertex_entry: String,
    pub fragment_entry: String,
}

/// Uniform buffer layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniformLayout {
    /// Buffer name
    pub name: String,
    /// Binding index
    pub binding: u32,
    /// Fields in the buffer
    pub fields: Vec<UniformField>,
    /// Total size in bytes (must be 16-byte aligned)
    pub size: u32,
}

/// A single uniform field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniformField {
    /// Field name
    pub name: String,
    /// Field type
    pub field_type: UniformType,
    /// Offset in bytes
    pub offset: u32,
}

/// Uniform types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UniformType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Mat3,
    Mat4,
    Int,
    Ivec2,
    Ivec3,
    Ivec4,
}

impl UniformType {
    /// Size in bytes
    pub fn size(&self) -> u32 {
        match self {
            UniformType::Float | UniformType::Int => 4,
            UniformType::Vec2 | UniformType::Ivec2 => 8,
            UniformType::Vec3 | UniformType::Ivec3 => 12,
            UniformType::Vec4 | UniformType::Ivec4 => 16,
            UniformType::Mat3 => 36,
            UniformType::Mat4 => 64,
        }
    }

    /// Alignment in bytes
    pub fn alignment(&self) -> u32 {
        match self {
            UniformType::Float | UniformType::Int => 4,
            UniformType::Vec2 | UniformType::Ivec2 => 8,
            UniformType::Vec3 | UniformType::Ivec3 | UniformType::Vec4 | UniformType::Ivec4 => 16,
            UniformType::Mat3 | UniformType::Mat4 => 16,
        }
    }
}

/// Texture binding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureBinding {
    /// Binding name
    pub name: String,
    /// Binding index
    pub binding: u32,
    /// Texture type
    pub texture_type: TextureType,
    /// Sample type
    pub sample_type: TextureSampleType,
}

/// Texture types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextureType {
    Texture1D,
    Texture2D,
    Texture3D,
    TextureCube,
}

/// Texture sample types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextureSampleType {
    Float,
    UnfilterableFloat,
    Depth,
    Sint,
    Uint,
}

/// GPU render pass configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderPassConfig {
    /// Pass name
    pub name: String,
    /// Shader to use
    pub shader: String,
    /// Target (screen or texture name)
    pub target: RenderTarget,
    /// Clear color (if clearing)
    pub clear_color: Option<[f32; 4]>,
    /// Blend mode
    pub blend: Option<BlendMode>,
}

/// Render target
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderTarget {
    /// Render to screen
    Screen,
    /// Render to named texture
    Texture(String),
}

/// Blend modes
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlendMode {
    /// No blending
    None,
    /// Standard alpha blending
    Alpha,
    /// Additive blending
    Additive,
    /// Multiplicative blending
    Multiply,
    /// Premultiplied alpha
    PremultipliedAlpha,
}

/// GPU plugin manifest extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuPluginConfig {
    /// Preferred renderer type
    #[serde(default)]
    pub renderer: GpuRendererType,

    /// Shaders used by this plugin
    #[serde(default)]
    pub shaders: Vec<ShaderSource>,

    /// Uniform buffer layouts
    #[serde(default)]
    pub uniforms: Vec<UniformLayout>,

    /// Texture bindings
    #[serde(default)]
    pub textures: Vec<TextureBinding>,

    /// Render passes (in order)
    #[serde(default)]
    pub render_passes: Vec<RenderPassConfig>,

    /// Post-processing effects
    #[serde(default)]
    pub post_effects: Vec<String>,

    /// Required GPU features
    #[serde(default)]
    pub required_features: Vec<String>,
}

impl Default for GpuPluginConfig {
    fn default() -> Self {
        Self {
            renderer: GpuRendererType::WebGPU,
            shaders: Vec::new(),
            uniforms: Vec::new(),
            textures: Vec::new(),
            render_passes: Vec::new(),
            post_effects: Vec::new(),
            required_features: Vec::new(),
        }
    }
}

/// Uniform values that can be passed to shaders
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UniformValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    Ivec2([i32; 2]),
    Ivec3([i32; 3]),
    Ivec4([i32; 4]),
    Mat3([[f32; 3]; 3]),
    Mat4([[f32; 4]; 4]),
}

impl UniformValue {
    /// Convert to bytes for GPU upload
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            UniformValue::Float(v) => v.to_le_bytes().to_vec(),
            UniformValue::Vec2(v) => v.iter().flat_map(|f| f.to_le_bytes()).collect(),
            UniformValue::Vec3(v) => v.iter().flat_map(|f| f.to_le_bytes()).collect(),
            UniformValue::Vec4(v) => v.iter().flat_map(|f| f.to_le_bytes()).collect(),
            UniformValue::Int(v) => v.to_le_bytes().to_vec(),
            UniformValue::Ivec2(v) => v.iter().flat_map(|i| i.to_le_bytes()).collect(),
            UniformValue::Ivec3(v) => v.iter().flat_map(|i| i.to_le_bytes()).collect(),
            UniformValue::Ivec4(v) => v.iter().flat_map(|i| i.to_le_bytes()).collect(),
            UniformValue::Mat3(v) => v.iter().flatten().flat_map(|f| f.to_le_bytes()).collect(),
            UniformValue::Mat4(v) => v.iter().flatten().flat_map(|f| f.to_le_bytes()).collect(),
        }
    }
}

/// Build a uniform buffer from values
#[allow(dead_code)]
pub fn build_uniform_buffer(layout: &UniformLayout, values: &HashMap<String, UniformValue>) -> Vec<u8> {
    let mut buffer = vec![0u8; layout.size as usize];

    for field in &layout.fields {
        if let Some(value) = values.get(&field.name) {
            let bytes = value.to_bytes();
            let offset = field.offset as usize;
            let end = (offset + bytes.len()).min(buffer.len());
            buffer[offset..end].copy_from_slice(&bytes[..end - offset]);
        }
    }

    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform_value_to_bytes() {
        let v = UniformValue::Vec4([1.0, 2.0, 3.0, 4.0]);
        let bytes = v.to_bytes();
        assert_eq!(bytes.len(), 16);
    }

    #[test]
    fn test_uniform_type_size() {
        assert_eq!(UniformType::Float.size(), 4);
        assert_eq!(UniformType::Vec4.size(), 16);
        assert_eq!(UniformType::Mat4.size(), 64);
    }
}
