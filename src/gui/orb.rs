//! Orb Visualization Renderer
//!
//! GPU-accelerated audio visualization using wgpu.

use crate::errors::{GpuError, Result};
use egui::Context;
use std::rc::Rc;
use wgpu::util::DeviceExt;

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

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            resolution: [800.0, 600.0],
            time: 0.0,
            audio_level: 0.0,
            audio_peak: 0.0,
            cloud_count: 5.0,
            particle_count: 50.0,
            glow_intensity: 1.0,
            rotation_speed: 1.0,
            color_scheme: 0.0,
            quality: 2.0,
            _padding: [0.0; 2],
        }
    }
}

/// Orb renderer for audio visualization
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

impl OrbRenderer {
    /// Create a new orb renderer
    ///
    /// # Errors
    ///
    /// Returns an error if wgpu initialization fails or shader compilation fails.
    pub fn new(_ctx: &Context) -> Result<Self> {
        log::info!("Initializing OrbRenderer (GPU visualization)");

        // Initialize wgpu instance and adapter
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        log::debug!("Created wgpu instance with all backends");

        // Request adapter (integrated or discrete GPU)
        log::trace!("Requesting GPU adapter (HighPerformance)");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| GpuError::NoAdapterFound)?;

        // Log adapter info
        let adapter_info = adapter.get_info();
        log::info!(
            "GPU adapter found: {} ({:?})",
            adapter_info.name,
            adapter_info.backend
        );
        log::debug!(
            "GPU vendor: {:?}, device: {:?}, driver: {}",
            adapter_info.vendor,
            adapter_info.device,
            adapter_info.driver
        );
        log::debug!("GPU adapter features: {:?}", adapter.features());

        // Create device and queue
        log::trace!("Creating wgpu device and queue");
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("OrbRenderer Device"),
                required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
                required_limits: wgpu::Limits {
                    max_texture_dimension_1d: 32,
                    ..Default::default()
                },
                memory_hints: Default::default(),
            },
            None,
        ))
        .map_err(|e| GpuError::DeviceCreationFailed {
            reason: format!("Failed to create GPU device: {}", e),
            source: Some(Box::new(e) as _),
        })?;

        log::debug!("wgpu device and queue created successfully");

        let device = Rc::new(device);
        let queue = Rc::new(queue);

        // Load shader from WGSL file
        log::trace!("Loading WGSL shader");
        let shader_code = Self::load_shader()?;
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Nebula Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_code.clone().into()),
        });
        log::debug!("Shader module created ({} bytes)", shader_code.len());

        // Create uniform buffer
        let uniforms = Uniforms::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        log::trace!("Uniform buffer created ({} bytes)", std::mem::size_of::<Uniforms>());

        // Create FFT texture (1D texture with 32 bins)
        let fft_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("FFT Texture"),
            size: wgpu::Extent3d {
                width: 32,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D1,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        log::trace!("FFT texture created (32x1, R32Float)");

        let fft_texture_view = fft_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("FFT Texture View"),
            format: None,
            dimension: Some(wgpu::TextureViewDimension::D1),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
            usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
        });

        // Create FFT sampler (using Nearest filtering for non-filterable R32Float format)
        let fft_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("FFT Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &[
                // Uniform buffer binding
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // FFT texture binding (R32Float is not filterable)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D1,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                // FFT sampler binding (non-filtering for R32Float)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });
        log::trace!("Bind group layout created");

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&fft_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&fft_sampler),
                },
            ],
        });
        log::trace!("Bind group created");

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        log::trace!("Creating render pipeline");
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Nebula Pipeline"),
            layout: Some(&pipeline_layout),
            cache: None,
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
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
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });
        log::info!("Render pipeline created successfully");

        log::info!("OrbRenderer initialization complete");

        Ok(Self {
            device,
            queue,
            pipeline,
            uniform_buffer,
            bind_group,
            fft_texture,
            fft_texture_view,
            fft_sampler,
            uniforms,
            start_time: std::time::Instant::now(),
            audio_level: 0.0,
            audio_peak: 0.0,
            fft_data: [0.0; 32],
        })
    }

    /// Load the WGSL shader from the plugin directory
    fn load_shader() -> Result<String> {
        // Path to the nebula shader
        let shader_path = "plugins/nebula-aura-gpu/src/shaders/nebula.wgsl";

        log::trace!("Loading shader from {}", shader_path);

        // Try to read the shader file
        match std::fs::read_to_string(shader_path) {
            Ok(shader) => {
                log::debug!("Loaded shader from {} ({} bytes)", shader_path, shader.len());
                Ok(shader)
            }
            Err(e) => {
                log::warn!("Failed to load shader from {}: {}, using fallback", shader_path, e);
                log::warn!("This may indicate a corrupted installation. Reinstalling the application is recommended.");
                // Fallback shader (minimal vertex + fragment)
                Ok(r#"
// Full-screen quad vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    return vec4<f32>(positions[vertex_index], 0.0, 1.0);
}

// Simple fragment shader
@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.5, 0.2, 0.6, 1.0); // Purple color
}
"#
                .to_string())
            }
        }
    }

    /// Update audio data for visualization
    pub fn update_audio(&mut self, level: f32, fft: &[f32; 32]) {
        self.audio_level = level;
        self.audio_peak = self.audio_peak.max(level * 0.1 + self.audio_peak * 0.9);
        self.fft_data.copy_from_slice(fft);

        // Update FFT texture
        let texture_data: Vec<u8> = fft
            .iter()
            .flat_map(|&v| v.to_le_bytes())
            .collect();

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.fft_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texture_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(32 * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 32,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Render the orb visualization
    pub fn render(&mut self, _ctx: &Context, level: f32, fft: &[f32; 32]) {
        // Update audio data
        self.update_audio(level, fft);

        // Update uniforms
        let elapsed = self.start_time.elapsed().as_secs_f32();
        self.uniforms.time = elapsed;
        self.uniforms.audio_level = self.audio_level;
        self.uniforms.audio_peak = self.audio_peak;

        // Update uniform buffer
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );

        // Note: Actual rendering will be done through the egui-wgpu integration
        // The pipeline is ready, but we need a surface to render to
        // This will be integrated with the egui render pass
    }

    /// Get the render pipeline for external use (egui-wgpu integration)
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    /// Get the bind group for external use
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Get the wgpu device for external use
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Get the wgpu queue for external use
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Set color scheme (0=purple, 1=cyan, 2=fire, 3=aurora, 4=cosmic)
    pub fn set_color_scheme(&mut self, scheme: f32) {
        self.uniforms.color_scheme = scheme;
    }

    /// Set quality level (0-3, higher = more details)
    pub fn set_quality(&mut self, quality: f32) {
        self.uniforms.quality = quality;
    }

    /// Set glow intensity
    pub fn set_glow_intensity(&mut self, intensity: f32) {
        self.uniforms.glow_intensity = intensity;
    }

    /// Set cloud count
    pub fn set_cloud_count(&mut self, count: f32) {
        self.uniforms.cloud_count = count;
    }

    /// Set particle count
    pub fn set_particle_count(&mut self, count: f32) {
        self.uniforms.particle_count = count;
    }

    /// Set rotation speed
    pub fn set_rotation_speed(&mut self, speed: f32) {
        self.uniforms.rotation_speed = speed;
    }

    /// Set resolution
    pub fn set_resolution(&mut self, width: f32, height: f32) {
        self.uniforms.resolution = [width, height];
    }
}

impl Default for OrbRenderer {
    fn default() -> Self {
        // Note: This creates a dummy context which may fail in real usage
        // Consider using Option<OrbRenderer> or removing Default impl
        Self::new(&egui::Context::default()).expect("Failed to create default OrbRenderer")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniforms_size() {
        // Ensure Uniforms struct is correctly sized
        assert_eq!(std::mem::size_of::<Uniforms>(), 48);
    }

    #[test]
    fn test_uniforms_alignment() {
        // Ensure proper alignment for wgpu
        assert_eq!(std::mem::align_of::<Uniforms>(), 4);
    }

    #[test]
    fn test_uniforms_default() {
        let uniforms = Uniforms::default();
        assert_eq!(uniforms.resolution, [800.0, 600.0]);
        assert_eq!(uniforms.time, 0.0);
        assert_eq!(uniforms.audio_level, 0.0);
        assert_eq!(uniforms.cloud_count, 5.0);
        assert_eq!(uniforms.particle_count, 50.0);
    }

    #[test]
    fn test_orb_renderer_creation() {
        // This test ensures OrbRenderer can be created without panicking
        // Note: May fail on systems without GPU support
        let ctx = egui::Context::default();
        let _renderer = OrbRenderer::new(&ctx);
    }

    #[test]
    fn test_update_audio() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        let fft_data = [0.5; 32];
        renderer.update_audio(0.7, &fft_data);

        assert_eq!(renderer.audio_level, 0.7);
        assert_eq!(renderer.fft_data, [0.5; 32]);
        assert!(renderer.audio_peak > 0.0);
    }

    #[test]
    fn test_set_color_scheme() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_color_scheme(2.0);
        assert_eq!(renderer.uniforms.color_scheme, 2.0);
    }

    #[test]
    fn test_set_quality() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_quality(3.0);
        assert_eq!(renderer.uniforms.quality, 3.0);
    }

    #[test]
    fn test_set_glow_intensity() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_glow_intensity(1.5);
        assert_eq!(renderer.uniforms.glow_intensity, 1.5);
    }

    #[test]
    fn test_set_cloud_count() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_cloud_count(8.0);
        assert_eq!(renderer.uniforms.cloud_count, 8.0);
    }

    #[test]
    fn test_set_particle_count() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_particle_count(100.0);
        assert_eq!(renderer.uniforms.particle_count, 100.0);
    }

    #[test]
    fn test_set_rotation_speed() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_rotation_speed(2.0);
        assert_eq!(renderer.uniforms.rotation_speed, 2.0);
    }

    #[test]
    fn test_set_resolution() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_resolution(1920.0, 1080.0);
        assert_eq!(renderer.uniforms.resolution, [1920.0, 1080.0]);
    }

    #[test]
    fn test_render_updates_uniforms() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        let fft_data = [0.3; 32];
        renderer.render(&ctx, 0.5, &fft_data);

        // After render, audio_level should be updated
        assert_eq!(renderer.audio_level, 0.5);
        assert_eq!(renderer.uniforms.audio_level, 0.5);

        // Time should have advanced
        assert!(renderer.uniforms.time > 0.0);
    }
}
