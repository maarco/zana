/**
 * Nebula Aura GPU Renderer
 *
 * WebGPU-accelerated nebula visualization.
 * Falls back to WebGL2 if WebGPU is not available.
 */

// Shader source (loaded at build time or fetched)
import shaderSource from './shaders/nebula.wgsl?raw';

// Color scheme indices
const COLOR_SCHEMES = {
  purple: 0,
  cyan: 1,
  fire: 2,
  aurora: 3,
  cosmic: 4,
};

// Quality level indices
const QUALITY_LEVELS = {
  low: 0,
  medium: 1,
  high: 2,
  ultra: 3,
};

/**
 * GPU Renderer class
 */
export class NebulaGPURenderer {
  constructor() {
    this.device = null;
    this.context = null;
    this.pipeline = null;
    this.uniformBuffer = null;
    this.bindGroup = null;
    this.fftTexture = null;

    this.canvas = null;
    this.startTime = performance.now();
    this.animationId = null;

    this.config = {
      cloud_count: 7,
      particle_count: 200,
      glow_intensity: 1.0,
      color_scheme: 'purple',
      rotation_speed: 1.0,
      bloom_enabled: true,
      quality: 'high',
    };

    this.audioLevel = 0;
    this.audioPeak = 0;
    this.fftData = new Float32Array(32);

    this.isInitialized = false;
    this.fallbackMode = false;
  }

  /**
   * Initialize the renderer
   */
  async init(canvas, config = {}) {
    this.canvas = canvas;
    this.config = { ...this.config, ...config };

    // Check for WebGPU support
    if (!navigator.gpu) {
      console.warn('[NebulaGPU] WebGPU not supported, falling back to WebGL2');
      return this.initWebGL2Fallback();
    }

    try {
      // Request adapter and device
      const adapter = await navigator.gpu.requestAdapter({
        powerPreference: 'high-performance',
      });

      if (!adapter) {
        throw new Error('No WebGPU adapter found');
      }

      this.device = await adapter.requestDevice();

      // Configure canvas context
      this.context = canvas.getContext('webgpu');
      const format = navigator.gpu.getPreferredCanvasFormat();

      this.context.configure({
        device: this.device,
        format: format,
        alphaMode: 'premultiplied',
      });

      // Create shader module
      const shaderModule = this.device.createShaderModule({
        code: shaderSource,
      });

      // Create pipeline
      this.pipeline = this.device.createRenderPipeline({
        layout: 'auto',
        vertex: {
          module: shaderModule,
          entryPoint: 'vs_main',
        },
        fragment: {
          module: shaderModule,
          entryPoint: 'fs_main',
          targets: [
            {
              format: format,
              blend: {
                color: {
                  srcFactor: 'src-alpha',
                  dstFactor: 'one-minus-src-alpha',
                  operation: 'add',
                },
                alpha: {
                  srcFactor: 'one',
                  dstFactor: 'one-minus-src-alpha',
                  operation: 'add',
                },
              },
            },
          ],
        },
        primitive: {
          topology: 'triangle-list',
        },
      });

      // Create uniform buffer (aligned to 16 bytes)
      // struct Uniforms {
      //   resolution: vec2<f32>,   // 8 bytes
      //   time: f32,               // 4 bytes
      //   audio_level: f32,        // 4 bytes
      //   audio_peak: f32,         // 4 bytes
      //   cloud_count: f32,        // 4 bytes
      //   particle_count: f32,     // 4 bytes
      //   glow_intensity: f32,     // 4 bytes
      //   rotation_speed: f32,     // 4 bytes
      //   color_scheme: f32,       // 4 bytes
      //   quality: f32,            // 4 bytes
      //   _padding: vec2<f32>,     // 8 bytes
      // } = 48 bytes total
      this.uniformBuffer = this.device.createBuffer({
        size: 48,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      });

      // Create FFT texture (1D, 32 samples)
      this.fftTexture = this.device.createTexture({
        size: [32, 1, 1],
        format: 'r32float',
        usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
      });

      // Create sampler
      const sampler = this.device.createSampler({
        magFilter: 'linear',
        minFilter: 'linear',
      });

      // Create bind group
      this.bindGroup = this.device.createBindGroup({
        layout: this.pipeline.getBindGroupLayout(0),
        entries: [
          {
            binding: 0,
            resource: { buffer: this.uniformBuffer },
          },
          {
            binding: 1,
            resource: this.fftTexture.createView(),
          },
          {
            binding: 2,
            resource: sampler,
          },
        ],
      });

      this.isInitialized = true;
      console.log('[NebulaGPU] WebGPU initialized successfully');

      return true;
    } catch (error) {
      console.error('[NebulaGPU] WebGPU initialization failed:', error);
      return this.initWebGL2Fallback();
    }
  }

  /**
   * WebGL2 fallback initialization
   */
  initWebGL2Fallback() {
    this.fallbackMode = true;
    const gl = this.canvas.getContext('webgl2', {
      alpha: true,
      premultipliedAlpha: true,
      antialias: true,
    });

    if (!gl) {
      console.error('[NebulaGPU] WebGL2 not supported');
      return false;
    }

    this.gl = gl;
    this.initWebGL2Pipeline();
    this.isInitialized = true;
    console.log('[NebulaGPU] WebGL2 fallback initialized');
    return true;
  }

  /**
   * Initialize WebGL2 render pipeline
   */
  initWebGL2Pipeline() {
    const gl = this.gl;

    // Vertex shader (full-screen quad)
    const vsSource = `#version 300 es
      out vec2 vUv;
      void main() {
        vec2 positions[3] = vec2[3](
          vec2(-1.0, -1.0),
          vec2(3.0, -1.0),
          vec2(-1.0, 3.0)
        );
        gl_Position = vec4(positions[gl_VertexID], 0.0, 1.0);
        vUv = positions[gl_VertexID] * 0.5 + 0.5;
        vUv.y = 1.0 - vUv.y;
      }
    `;

    // Fragment shader (simplified nebula)
    const fsSource = `#version 300 es
      precision highp float;
      in vec2 vUv;
      out vec4 fragColor;

      uniform vec2 uResolution;
      uniform float uTime;
      uniform float uAudioLevel;
      uniform float uCloudCount;
      uniform float uGlowIntensity;
      uniform float uColorScheme;

      float hash(vec2 p) {
        vec3 p3 = fract(vec3(p.xyx) * 0.13);
        p3 += dot(p3, p3.yzx + 3.333);
        return fract(p3.x * p3.y * p3.z);
      }

      float noise(vec2 p) {
        vec2 i = floor(p);
        vec2 f = fract(p);
        f = f * f * (3.0 - 2.0 * f);
        return mix(
          mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x),
          mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x),
          f.y
        );
      }

      vec3 palette(float t, float scheme) {
        vec3 a, b, c, d;
        if (scheme < 0.5) { // Purple
          a = vec3(0.5, 0.2, 0.6); b = vec3(0.5, 0.3, 0.4);
          c = vec3(1.0); d = vec3(0.8, 0.3, 0.5);
        } else if (scheme < 1.5) { // Cyan
          a = vec3(0.2, 0.5, 0.6); b = vec3(0.3, 0.4, 0.4);
          c = vec3(1.0); d = vec3(0.0, 0.5, 0.6);
        } else { // Fire
          a = vec3(0.5, 0.2, 0.1); b = vec3(0.5, 0.3, 0.2);
          c = vec3(1.0, 1.0, 0.5); d = vec3(0.0, 0.1, 0.2);
        }
        return a + b * cos(6.28318 * (c * t + d));
      }

      void main() {
        float aspect = uResolution.x / uResolution.y;
        vec2 uv = vUv;
        uv.x = (uv.x - 0.5) * aspect + 0.5;

        vec2 center = vec2(0.5);
        float audio = min(uAudioLevel * 3.0, 1.0);

        vec3 color = vec3(0.0);
        float alpha = 0.0;

        // Ambient glow
        float d = length(uv - center);
        float ambient = 1.0 - smoothstep(0.0, 0.4 + audio * 0.2, d);
        color += palette(0.5, uColorScheme) * 0.3 * ambient * (0.15 + audio * 0.5);

        // Clouds
        for (float i = 0.0; i < 7.0; i++) {
          if (i >= uCloudCount) break;

          float angle = i * 6.28318 / uCloudCount + uTime * (0.3 + mod(i, 2.0) * 0.2);
          float dist = 0.05 + audio * 0.3 + sin(uTime * 0.8 + i) * 0.08;
          vec2 pos = center + vec2(cos(angle), sin(angle)) * dist;

          float cd = length(uv - pos);
          float size = 0.1 * audio + 0.05;
          float n = noise(uv * 3.0 + uTime * 0.1) * 0.3;
          float cloud = 1.0 - smoothstep(0.0, size * (1.0 + n), cd);
          cloud = cloud * cloud * (0.3 + audio * 0.5);

          vec3 cloudColor = palette(i / uCloudCount + uTime * 0.05, uColorScheme);
          color += cloudColor * cloud * uGlowIntensity;
          alpha = max(alpha, cloud);
        }

        // Core
        float coreDist = length(uv - center);
        float coreSize = 0.02 + audio * 0.15;
        float core = 1.0 - smoothstep(0.0, coreSize, coreDist);
        core = core * core * (0.4 + audio * 0.6);
        color += mix(vec3(1.0), palette(0.3, uColorScheme), smoothstep(0.0, coreSize, coreDist)) * core;
        alpha = max(alpha, core);

        // Vignette
        float vignette = 1.0 - smoothstep(0.3, 0.8, d * 1.5);
        color *= vignette;

        fragColor = vec4(color, clamp(alpha + length(color) * 0.5, 0.0, 1.0));
      }
    `;

    // Compile shaders
    const vs = gl.createShader(gl.VERTEX_SHADER);
    gl.shaderSource(vs, vsSource);
    gl.compileShader(vs);

    const fs = gl.createShader(gl.FRAGMENT_SHADER);
    gl.shaderSource(fs, fsSource);
    gl.compileShader(fs);

    // Create program
    this.glProgram = gl.createProgram();
    gl.attachShader(this.glProgram, vs);
    gl.attachShader(this.glProgram, fs);
    gl.linkProgram(this.glProgram);

    // Get uniform locations
    this.glUniforms = {
      resolution: gl.getUniformLocation(this.glProgram, 'uResolution'),
      time: gl.getUniformLocation(this.glProgram, 'uTime'),
      audioLevel: gl.getUniformLocation(this.glProgram, 'uAudioLevel'),
      cloudCount: gl.getUniformLocation(this.glProgram, 'uCloudCount'),
      glowIntensity: gl.getUniformLocation(this.glProgram, 'uGlowIntensity'),
      colorScheme: gl.getUniformLocation(this.glProgram, 'uColorScheme'),
    };

    // Create VAO
    this.glVAO = gl.createVertexArray();
  }

  /**
   * Update audio data
   */
  setAudioData(level, peak = level, fftData = null) {
    this.audioLevel = this.audioLevel * 0.4 + level * 0.6; // Smooth
    this.audioPeak = peak;
    if (fftData) {
      this.fftData.set(fftData.slice(0, 32));
    }
  }

  /**
   * Update configuration
   */
  setConfig(config) {
    this.config = { ...this.config, ...config };
  }

  /**
   * Render a frame
   */
  render() {
    if (!this.isInitialized) return;

    const time = (performance.now() - this.startTime) / 1000;
    const width = this.canvas.width;
    const height = this.canvas.height;

    if (this.fallbackMode) {
      this.renderWebGL2(time, width, height);
    } else {
      this.renderWebGPU(time, width, height);
    }
  }

  /**
   * WebGPU render
   */
  renderWebGPU(time, width, height) {
    // Update uniforms
    const uniformData = new Float32Array([
      width,
      height, // resolution
      time, // time
      this.audioLevel, // audio_level
      this.audioPeak, // audio_peak
      this.config.cloud_count, // cloud_count
      this.config.particle_count, // particle_count
      this.config.glow_intensity, // glow_intensity
      this.config.rotation_speed, // rotation_speed
      COLOR_SCHEMES[this.config.color_scheme] || 0, // color_scheme
      QUALITY_LEVELS[this.config.quality] || 2, // quality
      0,
      0, // padding
    ]);

    this.device.queue.writeBuffer(this.uniformBuffer, 0, uniformData);

    // Update FFT texture
    this.device.queue.writeTexture(
      { texture: this.fftTexture },
      this.fftData,
      { bytesPerRow: 32 * 4 },
      { width: 32, height: 1 }
    );

    // Create command encoder
    const commandEncoder = this.device.createCommandEncoder();

    // Get current texture
    const textureView = this.context.getCurrentTexture().createView();

    // Begin render pass
    const renderPass = commandEncoder.beginRenderPass({
      colorAttachments: [
        {
          view: textureView,
          loadOp: 'clear',
          storeOp: 'store',
          clearValue: { r: 0, g: 0, b: 0, a: 0 },
        },
      ],
    });

    renderPass.setPipeline(this.pipeline);
    renderPass.setBindGroup(0, this.bindGroup);
    renderPass.draw(3, 1, 0, 0); // Full-screen triangle
    renderPass.end();

    // Submit
    this.device.queue.submit([commandEncoder.finish()]);
  }

  /**
   * WebGL2 render
   */
  renderWebGL2(time, width, height) {
    const gl = this.gl;

    gl.viewport(0, 0, width, height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);

    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

    gl.useProgram(this.glProgram);
    gl.bindVertexArray(this.glVAO);

    gl.uniform2f(this.glUniforms.resolution, width, height);
    gl.uniform1f(this.glUniforms.time, time);
    gl.uniform1f(this.glUniforms.audioLevel, this.audioLevel);
    gl.uniform1f(this.glUniforms.cloudCount, this.config.cloud_count);
    gl.uniform1f(this.glUniforms.glowIntensity, this.config.glow_intensity);
    gl.uniform1f(this.glUniforms.colorScheme, COLOR_SCHEMES[this.config.color_scheme] || 0);

    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  /**
   * Handle resize
   */
  resize(width, height) {
    if (!this.canvas) return;

    const dpr = window.devicePixelRatio || 1;
    this.canvas.width = width * dpr;
    this.canvas.height = height * dpr;
    this.canvas.style.width = width + 'px';
    this.canvas.style.height = height + 'px';

    if (!this.fallbackMode && this.context) {
      this.context.configure({
        device: this.device,
        format: navigator.gpu.getPreferredCanvasFormat(),
        alphaMode: 'premultiplied',
      });
    }
  }

  /**
   * Start animation loop
   */
  start() {
    const loop = () => {
      this.render();
      this.animationId = requestAnimationFrame(loop);
    };
    loop();
  }

  /**
   * Stop animation loop
   */
  stop() {
    if (this.animationId) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }
  }

  /**
   * Cleanup
   */
  destroy() {
    this.stop();

    if (this.uniformBuffer) {
      this.uniformBuffer.destroy();
    }
    if (this.fftTexture) {
      this.fftTexture.destroy();
    }

    this.device = null;
    this.context = null;
    this.pipeline = null;
    this.bindGroup = null;
    this.isInitialized = false;
  }
}

// ============================================================================
// Plugin Interface (for kVoice plugin system)
// ============================================================================

let renderer = null;

/**
 * Initialize the plugin
 */
export async function init(ctx) {
  renderer = new NebulaGPURenderer();

  // Find or create canvas
  let canvas = ctx.canvas;
  if (!canvas) {
    canvas = document.createElement('canvas');
    ctx.container.appendChild(canvas);
  }

  // Initialize renderer
  await renderer.init(canvas, ctx.config);
  renderer.resize(ctx.width, ctx.height);
  renderer.start();
}

/**
 * Update animation state
 */
export function update(dt, audioLevel, fft) {
  if (renderer) {
    renderer.setAudioData(audioLevel, audioLevel, fft);
  }
}

/**
 * Render is handled by the GPU renderer's animation loop
 */
export function render(ctx, width, height, audioLevel) {
  // GPU rendering is handled internally
}

/**
 * Handle resize
 */
export function onResize(width, height) {
  if (renderer) {
    renderer.resize(width, height);
  }
}

/**
 * Handle config change
 */
export function onConfigChange(newConfig) {
  if (renderer) {
    renderer.setConfig(newConfig);
  }
}

/**
 * Cleanup
 */
export function destroy() {
  if (renderer) {
    renderer.destroy();
    renderer = null;
  }
}

// Export the renderer class for direct use
export default NebulaGPURenderer;
