// Nebula Aura GPU Shader
// Real-time procedural nebula visualization in WGSL

// Uniforms passed from JavaScript
struct Uniforms {
    resolution: vec2<f32>,      // Canvas resolution
    time: f32,                  // Time in seconds
    audio_level: f32,           // Audio level (0-1)
    audio_peak: f32,            // Audio peak (0-1)
    cloud_count: f32,           // Number of clouds
    particle_count: f32,        // Number of particles
    glow_intensity: f32,        // Glow multiplier
    rotation_speed: f32,        // Cloud rotation speed
    color_scheme: f32,          // Color scheme index (0-4)
    quality: f32,               // Quality level (0-3)
    _padding: vec2<f32>,        // Padding for alignment
}

@group(0) @binding(0) var<uniform> u: Uniforms;

// FFT data texture (32 frequency bins)
@group(0) @binding(1) var fft_texture: texture_1d<f32>;
@group(0) @binding(2) var fft_sampler: sampler;

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Full-screen quad vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Generate full-screen triangle
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = positions[vertex_index] * 0.5 + 0.5;
    out.uv.y = 1.0 - out.uv.y; // Flip Y for canvas coordinates
    return out;
}

// ============================================================================
// NOISE FUNCTIONS
// ============================================================================

// Hash function for pseudo-random values
fn hash(p: vec2<f32>) -> f32 {
    let p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.13);
    let dot_result = dot(p3, p3 + vec3<f32>(3.333, 3.333, 3.333));
    return fract(dot_result * dot_result);
}

fn hash2(p: vec2<f32>) -> vec2<f32> {
    let k = vec2<f32>(0.3183099, 0.3678794);
    let p2 = p * k + k.yx;
    return fract(16.0 * k * fract(p2.x * p2.y * (p2.x + p2.y)));
}

// Simplex-style 2D noise
fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(hash(i + vec2<f32>(0.0, 0.0)), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

// Fractal Brownian Motion
fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var p_mut = p;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise(p_mut * frequency);
        amplitude *= 0.5;
        frequency *= 2.0;
        // Rotate for less axis-aligned artifacts
        p_mut = vec2<f32>(
            p_mut.x * 0.866 - p_mut.y * 0.5,
            p_mut.x * 0.5 + p_mut.y * 0.866
        );
    }
    return value;
}

// ============================================================================
// COLOR PALETTES
// ============================================================================

fn get_palette_color(t: f32, scheme: f32) -> vec3<f32> {
    // Purple/Magenta (default)
    if (scheme < 0.5) {
        let a = vec3<f32>(0.5, 0.2, 0.6);
        let b = vec3<f32>(0.5, 0.3, 0.4);
        let c = vec3<f32>(1.0, 1.0, 1.0);
        let d = vec3<f32>(0.8, 0.3, 0.5);
        return a + b * cos(6.28318 * (c * t + d));
    }
    // Cyan
    else if (scheme < 1.5) {
        let a = vec3<f32>(0.2, 0.5, 0.6);
        let b = vec3<f32>(0.3, 0.4, 0.4);
        let c = vec3<f32>(1.0, 1.0, 1.0);
        let d = vec3<f32>(0.0, 0.5, 0.6);
        return a + b * cos(6.28318 * (c * t + d));
    }
    // Fire
    else if (scheme < 2.5) {
        let a = vec3<f32>(0.5, 0.2, 0.1);
        let b = vec3<f32>(0.5, 0.3, 0.2);
        let c = vec3<f32>(1.0, 1.0, 0.5);
        let d = vec3<f32>(0.0, 0.1, 0.2);
        return a + b * cos(6.28318 * (c * t + d));
    }
    // Aurora
    else if (scheme < 3.5) {
        let a = vec3<f32>(0.2, 0.5, 0.3);
        let b = vec3<f32>(0.3, 0.4, 0.5);
        let c = vec3<f32>(1.0, 1.0, 1.5);
        let d = vec3<f32>(0.3, 0.5, 0.2);
        return a + b * cos(6.28318 * (c * t + d));
    }
    // Cosmic (deep space)
    else {
        let a = vec3<f32>(0.1, 0.1, 0.3);
        let b = vec3<f32>(0.4, 0.2, 0.5);
        let c = vec3<f32>(2.0, 1.0, 1.0);
        let d = vec3<f32>(0.5, 0.2, 0.5);
        return a + b * cos(6.28318 * (c * t + d));
    }
}

// ============================================================================
// NEBULA RENDERING
// ============================================================================

// Soft circle (used for clouds and core)
fn soft_circle(uv: vec2<f32>, center: vec2<f32>, radius: f32, softness: f32) -> f32 {
    let d = length(uv - center);
    return 1.0 - smoothstep(radius - softness, radius + softness, d);
}

// Render a single nebula cloud
fn render_cloud(
    uv: vec2<f32>,
    center: vec2<f32>,
    radius: f32,
    color: vec3<f32>,
    time: f32,
    audio: f32
) -> vec4<f32> {
    // Distort the cloud shape with noise
    let noise_offset = fbm(uv * 3.0 + time * 0.1, 3) * 0.3;
    let distorted_radius = radius * (1.0 + noise_offset + audio * 0.5);

    // Calculate distance with soft falloff
    let d = length(uv - center);
    let falloff = 1.0 - smoothstep(0.0, distorted_radius, d);
    let alpha = falloff * falloff * (0.3 + audio * 0.5);

    // Add internal detail
    let detail = fbm(uv * 8.0 + center * 10.0, 2) * 0.3;
    let final_color = color * (1.0 + detail);

    return vec4<f32>(final_color * alpha, alpha);
}

// Render sparkle particles
fn render_particles(uv: vec2<f32>, time: f32, audio: f32, count: f32) -> vec3<f32> {
    var particles = vec3<f32>(0.0);
    let particle_count = i32(count);

    for (var i = 0; i < particle_count; i++) {
        let fi = f32(i);

        // Pseudo-random position using golden ratio
        let golden = 1.618033988749;
        let angle = fi * golden * 6.28318 + time * (0.02 + hash(vec2<f32>(fi, 0.0)) * 0.05);
        let base_dist = hash(vec2<f32>(fi, 1.0)) * 0.4 + 0.05;
        let dist = base_dist + audio * 0.25;

        let pos = vec2<f32>(cos(angle), sin(angle)) * dist + 0.5;

        // Twinkle effect
        let twinkle = sin(time * (2.0 + hash(vec2<f32>(fi, 2.0)) * 3.0) + fi * 0.5);
        let alpha = (0.2 + twinkle * 0.3) * (0.5 + audio * 0.5);

        // Particle size
        let size = 0.003 + audio * 0.008 + twinkle * 0.002;

        // Soft point
        let d = length(uv - pos);
        let brightness = smoothstep(size, 0.0, d) * max(alpha, 0.0);

        particles += vec3<f32>(1.0) * brightness;
    }

    return particles;
}

// Render pulsing ring
fn render_ring(uv: vec2<f32>, center: vec2<f32>, audio: f32, time: f32) -> vec3<f32> {
    if (audio < 0.05) {
        return vec3<f32>(0.0);
    }

    let radius = 0.02 + audio * 0.35;
    let d = length(uv - center);
    let thickness = 0.005 + audio * 0.015;

    let ring = smoothstep(thickness, 0.0, abs(d - radius));
    let alpha = (audio - 0.05) * 0.3;

    return vec3<f32>(0.78, 0.59, 1.0) * ring * alpha;
}

// ============================================================================
// MAIN FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Normalized coordinates centered at (0.5, 0.5)
    let aspect = u.resolution.x / u.resolution.y;
    var uv = in.uv;
    uv.x = (uv.x - 0.5) * aspect + 0.5;

    let center = vec2<f32>(0.5, 0.5);
    let t = u.time;
    let audio = min(u.audio_level * 3.0, 1.0); // Boosted audio

    // Quality-based settings
    let octaves = select(2, select(3, select(4, 5, u.quality > 2.5), u.quality > 1.5), u.quality > 0.5);
    let particle_mult = select(0.25, select(0.5, select(1.0, 1.5, u.quality > 2.5), u.quality > 1.5), u.quality > 0.5);

    // Accumulate color
    var color = vec3<f32>(0.0);
    var alpha = 0.0;

    // 1. Ambient background glow
    let idle_glow = 0.15 + sin(t * 0.5) * 0.1;
    let ambient_radius = 0.3 + audio * 0.2;
    let ambient = soft_circle(uv, center, ambient_radius, 0.2);
    let ambient_color = get_palette_color(0.5, u.color_scheme) * 0.3;
    color += ambient_color * ambient * (idle_glow + audio * 0.5) * u.glow_intensity;

    // 2. Swirling nebula clouds
    let cloud_count = i32(u.cloud_count);
    for (var i = 0; i < cloud_count; i++) {
        let fi = f32(i);
        let fc = f32(cloud_count);

        // Cloud position (orbiting)
        let base_angle = fi * 6.28318 / fc;
        let orbit_speed = (0.3 + f32(i % 2) * 0.2) * u.rotation_speed;
        let angle = base_angle + t * orbit_speed;

        // Distance from center (expands with audio)
        let base_dist = 0.05;
        let audio_dist = audio * 0.3;
        let breath = sin(t * 0.8 + fi) * 0.08;
        let dist = base_dist + audio_dist + breath;

        let cloud_pos = center + vec2<f32>(cos(angle), sin(angle)) * dist;

        // Cloud size
        let cloud_size = 0.1 * audio + 0.05;

        // Cloud color from palette
        let palette_t = fi / fc + t * 0.05;
        let cloud_color = get_palette_color(palette_t, u.color_scheme);

        // Render cloud
        let cloud = render_cloud(uv, cloud_pos, cloud_size, cloud_color, t, audio);
        color += cloud.rgb * u.glow_intensity;
        alpha = max(alpha, cloud.a);
    }

    // 3. Central core
    let core_radius = 0.02 + audio * 0.15;
    let core_dist = length(uv - center);
    let core_falloff = 1.0 - smoothstep(0.0, core_radius, core_dist);
    let core_alpha = core_falloff * core_falloff * (0.4 + audio * 0.6);

    // Core gradient (white center -> colored edge)
    let core_color = mix(
        vec3<f32>(1.0, 1.0, 1.0),
        get_palette_color(0.3, u.color_scheme),
        smoothstep(0.0, core_radius, core_dist)
    );
    color += core_color * core_alpha * u.glow_intensity;
    alpha = max(alpha, core_alpha);

    // 4. Sparkle particles
    let particles = render_particles(uv, t, audio, u.particle_count * particle_mult);
    color += particles;

    // 5. Pulsing ring
    let ring = render_ring(uv, center, audio, t);
    color += ring * u.glow_intensity;

    // 6. Vignette (soft edge fade)
    let vignette_dist = length(uv - center) * 1.5;
    let vignette = 1.0 - smoothstep(0.3, 0.8, vignette_dist);
    color *= vignette;

    // Final alpha calculation
    let final_alpha = clamp(alpha + length(color) * 0.5, 0.0, 1.0);

    // Output with premultiplied alpha for transparency
    return vec4<f32>(color, final_alpha);
}

// ============================================================================
// BLOOM POST-PROCESS SHADER (separate pass)
// ============================================================================

// Bloom extraction - extracts bright areas
@fragment
fn fs_bloom_extract(in: VertexOutput) -> @location(0) vec4<f32> {
    // This would sample from the main render target
    // For now, return placeholder
    return vec4<f32>(0.0);
}

// Bloom blur (Gaussian)
@fragment
fn fs_bloom_blur(in: VertexOutput) -> @location(0) vec4<f32> {
    // Gaussian blur implementation
    return vec4<f32>(0.0);
}

// Bloom composite
@fragment
fn fs_bloom_composite(in: VertexOutput) -> @location(0) vec4<f32> {
    // Combine original + bloom
    return vec4<f32>(0.0);
}
