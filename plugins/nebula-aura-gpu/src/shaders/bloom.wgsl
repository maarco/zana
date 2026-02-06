// Bloom Post-Processing Shader
// Multi-pass HDR bloom effect for nebula visualization

// Shared uniforms
struct BloomUniforms {
    resolution: vec2<f32>,
    threshold: f32,
    intensity: f32,
    radius: f32,
    _padding: vec3<f32>,
}

@group(0) @binding(0) var<uniform> u: BloomUniforms;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var input_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Full-screen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = positions[vertex_index] * 0.5 + 0.5;
    out.uv.y = 1.0 - out.uv.y;
    return out;
}

// ============================================================================
// PASS 1: BRIGHTNESS EXTRACTION
// Extracts pixels above threshold for bloom
// ============================================================================

@fragment
fn fs_extract(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(input_texture, input_sampler, in.uv);

    // Calculate luminance
    let luminance = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));

    // Soft knee threshold
    let soft_threshold = u.threshold * 0.9;
    let knee = smoothstep(soft_threshold, u.threshold, luminance);

    // Extract bright areas
    let extracted = color.rgb * knee;

    return vec4<f32>(extracted, color.a);
}

// ============================================================================
// PASS 2: GAUSSIAN BLUR (Horizontal)
// ============================================================================

@fragment
fn fs_blur_h(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel_size = 1.0 / u.resolution.x;

    // 9-tap Gaussian blur
    let offsets = array<f32, 5>(0.0, 1.3846153846, 3.2307692308, 5.0769230769, 6.9230769231);
    let weights = array<f32, 5>(0.2270270270, 0.1945945946, 0.1216216216, 0.0540540541, 0.0162162162);

    var color = textureSample(input_texture, input_sampler, in.uv).rgb * weights[0];

    for (var i = 1; i < 5; i++) {
        let offset = offsets[i] * texel_size * u.radius;
        color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(offset, 0.0)).rgb * weights[i];
        color += textureSample(input_texture, input_sampler, in.uv - vec2<f32>(offset, 0.0)).rgb * weights[i];
    }

    return vec4<f32>(color, 1.0);
}

// ============================================================================
// PASS 3: GAUSSIAN BLUR (Vertical)
// ============================================================================

@fragment
fn fs_blur_v(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel_size = 1.0 / u.resolution.y;

    let offsets = array<f32, 5>(0.0, 1.3846153846, 3.2307692308, 5.0769230769, 6.9230769231);
    let weights = array<f32, 5>(0.2270270270, 0.1945945946, 0.1216216216, 0.0540540541, 0.0162162162);

    var color = textureSample(input_texture, input_sampler, in.uv).rgb * weights[0];

    for (var i = 1; i < 5; i++) {
        let offset = offsets[i] * texel_size * u.radius;
        color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, offset)).rgb * weights[i];
        color += textureSample(input_texture, input_sampler, in.uv - vec2<f32>(0.0, -offset)).rgb * weights[i];
    }

    return vec4<f32>(color, 1.0);
}

// ============================================================================
// PASS 4: COMPOSITE (Original + Bloom)
// ============================================================================

@group(0) @binding(3) var bloom_texture: texture_2d<f32>;

@fragment
fn fs_composite(in: VertexOutput) -> @location(0) vec4<f32> {
    let original = textureSample(input_texture, input_sampler, in.uv);
    let bloom = textureSample(bloom_texture, input_sampler, in.uv).rgb;

    // Additive blend with intensity control
    let final_color = original.rgb + bloom * u.intensity;

    // Tone mapping (Reinhard)
    let mapped = final_color / (final_color + vec3<f32>(1.0));

    // Gamma correction
    let gamma_corrected = pow(mapped, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(gamma_corrected, original.a);
}

// ============================================================================
// KAWASE BLUR (Alternative, more efficient blur)
// ============================================================================

struct KawaseUniforms {
    resolution: vec2<f32>,
    offset: f32,
    _padding: f32,
}

@group(0) @binding(0) var<uniform> kawase_u: KawaseUniforms;

@fragment
fn fs_kawase_down(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = 1.0 / kawase_u.resolution;
    let offset = kawase_u.offset;

    var color = textureSample(input_texture, input_sampler, in.uv).rgb * 4.0;
    color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-offset, -offset) * texel).rgb;
    color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(offset, -offset) * texel).rgb;
    color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-offset, offset) * texel).rgb;
    color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(offset, offset) * texel).rgb;

    return vec4<f32>(color / 8.0, 1.0);
}

@fragment
fn fs_kawase_up(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = 1.0 / kawase_u.resolution;
    let offset = kawase_u.offset;

    var color = textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-offset, -offset) * texel).rgb;
    color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, -offset * 2.0) * texel).rgb * 2.0;
    color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(offset, -offset) * texel).rgb;
    color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-offset * 2.0, 0.0) * texel).rgb * 2.0;
    color += textureSample(input_texture, input_sampler, in.uv).rgb * 4.0;
    color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(offset * 2.0, 0.0) * texel).rgb * 2.0;
    color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(-offset, offset) * texel).rgb;
    color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(0.0, offset * 2.0) * texel).rgb * 2.0;
    color += textureSample(input_texture, input_sampler, in.uv + vec2<f32>(offset, offset) * texel).rgb;

    return vec4<f32>(color / 16.0, 1.0);
}

// ============================================================================
// CHROMATIC ABERRATION (Optional effect)
// ============================================================================

struct ChromaticUniforms {
    resolution: vec2<f32>,
    amount: f32,
    _padding: f32,
}

@group(0) @binding(0) var<uniform> chromatic_u: ChromaticUniforms;

@fragment
fn fs_chromatic(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let dir = normalize(in.uv - center);
    let dist = length(in.uv - center);

    // Radial offset based on distance from center
    let offset = dir * dist * chromatic_u.amount * 0.01;

    // Sample each channel with different offsets
    let r = textureSample(input_texture, input_sampler, in.uv + offset).r;
    let g = textureSample(input_texture, input_sampler, in.uv).g;
    let b = textureSample(input_texture, input_sampler, in.uv - offset).b;
    let a = textureSample(input_texture, input_sampler, in.uv).a;

    return vec4<f32>(r, g, b, a);
}

// ============================================================================
// VIGNETTE (Optional effect)
// ============================================================================

struct VignetteUniforms {
    resolution: vec2<f32>,
    intensity: f32,
    smoothness: f32,
}

@group(0) @binding(0) var<uniform> vignette_u: VignetteUniforms;

@fragment
fn fs_vignette(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(input_texture, input_sampler, in.uv);

    let center = vec2<f32>(0.5, 0.5);
    let dist = length(in.uv - center) * 1.414; // Normalize to 0-1 at corners

    let vignette = 1.0 - smoothstep(1.0 - vignette_u.intensity, 1.0 - vignette_u.intensity + vignette_u.smoothness, dist);

    return vec4<f32>(color.rgb * vignette, color.a);
}

// ============================================================================
// FILM GRAIN (Optional effect for cinematic look)
// ============================================================================

struct GrainUniforms {
    resolution: vec2<f32>,
    time: f32,
    amount: f32,
}

@group(0) @binding(0) var<uniform> grain_u: GrainUniforms;

fn rand(co: vec2<f32>) -> f32 {
    return fract(sin(dot(co, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fs_grain(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(input_texture, input_sampler, in.uv);

    // Generate noise
    let noise = rand(in.uv + grain_u.time) * 2.0 - 1.0;

    // Apply grain
    let grain = color.rgb + noise * grain_u.amount * 0.1;

    return vec4<f32>(grain, color.a);
}
