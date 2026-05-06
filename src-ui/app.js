/**
 * qVoice - Main Application
 *
 * Two orb modes:
 * - Nebula (purple) - classic always-alive orb
 * - Genesis (fire)  - born with every activation, dies when done
 *
 * Press Ctrl+S to toggle between modes.
 * Pure vanilla JS with WebGPU/WebGL2 rendering.
 */

// Tauri API imports
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { appWindow } = window.__TAURI__.window;

// ============================================================================
// STATE
// ============================================================================

const state = {
  isRecording: false,
  isProcessing: false,
  audioLevel: 0,
  fftBins: new Float32Array(32),
  lastTranscription: '',
  // Genesis state - 0 = dormant spark, 1 = fully alive
  genesisLevel: 0,
  targetGenesisLevel: 0,
  // Orb mode: 'nebula' (purple) or 'genesis' (fire)
  orbMode: 'nebula',
  config: {
    colorScheme: 'purple',
    quality: 'high',
    cloudCount: 7,
    particleCount: 200,
    glowIntensity: 1.0,
    rotationSpeed: 1.0,
  },
  models: [],
  devices: [],
  selectedModel: 'small',
  selectedDevice: '',
};

// ============================================================================
// DOM ELEMENTS
// ============================================================================

const elements = {
  canvas: document.getElementById('orb-canvas'),
  dragHandle: document.getElementById('drag-handle'),
  btnRecord: document.getElementById('btn-record'),
  btnSettings: document.getElementById('btn-settings'),
  btnClose: document.getElementById('btn-close'),
  btnCloseSettings: document.getElementById('btn-close-settings'),
  btnCopy: document.getElementById('btn-copy'),
  btnDownloadModel: document.getElementById('btn-download-model'),
  iconMic: document.getElementById('icon-mic'),
  iconStop: document.getElementById('icon-stop'),
  iconSpinner: document.getElementById('icon-spinner'),
  transcription: document.getElementById('transcription'),
  transcriptionText: document.getElementById('transcription-text'),
  settingsPanel: document.getElementById('settings-panel'),
  selectModel: document.getElementById('select-model'),
  selectDevice: document.getElementById('select-device'),
  selectColor: document.getElementById('select-color'),
  selectQuality: document.getElementById('select-quality'),
  modelStatus: document.getElementById('model-status'),
  status: document.getElementById('status'),
};

// ============================================================================
// NEBULA RENDERER - Classic Purple Orb (Always Alive)
// ============================================================================

class NebulaRenderer {
  constructor(canvas) {
    this.canvas = canvas;
    this.device = null;
    this.context = null;
    this.pipeline = null;
    this.uniformBuffer = null;
    this.bindGroup = null;
    this.startTime = performance.now();
    this.animationId = null;
    this.isWebGPU = false;

    // Fallback WebGL2
    this.gl = null;
    this.glProgram = null;
    this.glUniforms = {};
  }

  async init(sharedGPU) {
    // Use shared WebGPU if available
    if (sharedGPU) {
      this.device = sharedGPU.device;
      this.context = sharedGPU.context;
      await this.initWebGPU(sharedGPU.format);
      this.isWebGPU = true;
      console.log('[NebulaRenderer] Using shared WebGPU');
      return true;
    }

    // Fallback to WebGL2
    this.gl = this.canvas.getContext('webgl2', {
      alpha: true,
      premultipliedAlpha: true,
      antialias: true,
    });

    if (this.gl) {
      this.initWebGL2();
      console.log('[NebulaRenderer] WebGL2 fallback initialized');
      return true;
    }

    console.error('[NebulaRenderer] No GPU rendering available');
    return false;
  }

  async initWebGPU(format) {
    const shaderCode = `
      struct Uniforms {
        resolution: vec2<f32>,
        time: f32,
        audioLevel: f32,
        cloudCount: f32,
        glowIntensity: f32,
        colorScheme: f32,
        quality: f32,
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

      fn hash(p: vec2<f32>) -> f32 {
        let p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.13);
        return fract(dot(p3, p3 + 3.333) * (p3.x + p3.y + p3.z));
      }

      fn noise(p: vec2<f32>) -> f32 {
        let i = floor(p);
        let f = fract(p);
        let u = f * f * (3.0 - 2.0 * f);
        return mix(
          mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
          mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
          u.y
        );
      }

      fn palette(t: f32, scheme: f32) -> vec3<f32> {
        var a: vec3<f32>; var b: vec3<f32>; var c: vec3<f32>; var d: vec3<f32>;
        if (scheme < 0.5) { // Purple
          a = vec3<f32>(0.5, 0.2, 0.6); b = vec3<f32>(0.5, 0.3, 0.4);
          c = vec3<f32>(1.0, 1.0, 1.0); d = vec3<f32>(0.8, 0.3, 0.5);
        } else if (scheme < 1.5) { // Cyan
          a = vec3<f32>(0.2, 0.5, 0.6); b = vec3<f32>(0.3, 0.4, 0.4);
          c = vec3<f32>(1.0, 1.0, 1.0); d = vec3<f32>(0.0, 0.5, 0.6);
        } else if (scheme < 2.5) { // Fire
          a = vec3<f32>(0.5, 0.2, 0.1); b = vec3<f32>(0.5, 0.3, 0.2);
          c = vec3<f32>(1.0, 1.0, 0.5); d = vec3<f32>(0.0, 0.1, 0.2);
        } else if (scheme < 3.5) { // Aurora
          a = vec3<f32>(0.2, 0.5, 0.3); b = vec3<f32>(0.3, 0.4, 0.5);
          c = vec3<f32>(1.0, 1.0, 1.5); d = vec3<f32>(0.3, 0.5, 0.2);
        } else { // Cosmic
          a = vec3<f32>(0.1, 0.1, 0.3); b = vec3<f32>(0.4, 0.2, 0.5);
          c = vec3<f32>(2.0, 1.0, 1.0); d = vec3<f32>(0.5, 0.2, 0.5);
        }
        return a + b * cos(6.28318 * (c * t + d));
      }

      @fragment
      fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
        let aspect = u.resolution.x / u.resolution.y;
        var uv = in.uv;
        uv.x = (uv.x - 0.5) * aspect + 0.5;

        let center = vec2<f32>(0.5, 0.5);
        let t = u.time;
        let audio = min(u.audioLevel * 3.0, 1.0);

        var color = vec3<f32>(0.0);
        var alpha = 0.0;

        // Ambient glow
        let d = length(uv - center);
        let ambient = 1.0 - smoothstep(0.0, 0.4 + audio * 0.2, d);
        let idle = 0.15 + sin(t * 0.5) * 0.1;
        color += palette(0.5, u.colorScheme) * 0.3 * ambient * (idle + audio * 0.5) * u.glowIntensity;

        // Clouds
        for (var i = 0.0; i < 12.0; i += 1.0) {
          if (i >= u.cloudCount) { break; }

          let angle = i * 6.28318 / u.cloudCount + t * (0.3 + fract(i * 0.5) * 0.2);
          let dist = 0.05 + audio * 0.3 + sin(t * 0.8 + i) * 0.08;
          let pos = center + vec2<f32>(cos(angle), sin(angle)) * dist;

          let cd = length(uv - pos);
          let size = 0.1 * audio + 0.05;
          let n = noise(uv * 3.0 + t * 0.1) * 0.3;
          let cloud = 1.0 - smoothstep(0.0, size * (1.0 + n), cd);
          let cloudAlpha = cloud * cloud * (0.3 + audio * 0.5);

          let cloudColor = palette(i / u.cloudCount + t * 0.05, u.colorScheme);
          color += cloudColor * cloudAlpha * u.glowIntensity;
          alpha = max(alpha, cloudAlpha);
        }

        // Core
        let coreSize = 0.02 + audio * 0.15;
        let core = 1.0 - smoothstep(0.0, coreSize, d);
        let coreAlpha = core * core * (0.4 + audio * 0.6);
        color += mix(vec3<f32>(1.0), palette(0.3, u.colorScheme), smoothstep(0.0, coreSize, d)) * coreAlpha * u.glowIntensity;
        alpha = max(alpha, coreAlpha);

        // Particles
        for (var i = 0.0; i < 100.0; i += 1.0) {
          if (i >= u.quality * 30.0) { break; }
          let seed = sin(i * 12.9898) * 43758.5453;
          let r1 = fract(seed);
          let r2 = fract(seed * 2.3);
          let pAngle = r1 * 6.28318 + t * (0.02 + r2 * 0.05);
          let pDist = r2 * 0.4 + 0.05 + audio * 0.25;
          let pPos = center + vec2<f32>(cos(pAngle), sin(pAngle)) * pDist;
          let twinkle = sin(t * (2.0 + r1 * 3.0) + i * 0.5);
          let pAlpha = (0.2 + twinkle * 0.3) * (0.5 + audio * 0.5);
          let pSize = 0.003 + audio * 0.008;
          let pd = length(uv - pPos);
          let brightness = smoothstep(pSize, 0.0, pd) * max(pAlpha, 0.0);
          color += vec3<f32>(1.0) * brightness;
        }

        // Ring
        if (audio > 0.05) {
          let ringSize = 0.02 + audio * 0.35;
          let ring = smoothstep(0.01, 0.0, abs(d - ringSize));
          let ringAlpha = (audio - 0.05) * 0.3;
          color += vec3<f32>(0.78, 0.59, 1.0) * ring * ringAlpha * u.glowIntensity;
        }

        // Vignette
        color *= 1.0 - smoothstep(0.3, 0.8, d * 1.5);

        return vec4<f32>(color, clamp(alpha + length(color) * 0.5, 0.0, 1.0));
      }
    `;

    const shaderModule = this.device.createShaderModule({ code: shaderCode });

    this.pipeline = this.device.createRenderPipeline({
      layout: 'auto',
      vertex: { module: shaderModule, entryPoint: 'vs_main' },
      fragment: {
        module: shaderModule,
        entryPoint: 'fs_main',
        targets: [{
          format,
          blend: {
            color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha', operation: 'add' },
            alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
          },
        }],
      },
      primitive: { topology: 'triangle-list' },
    });

    this.uniformBuffer = this.device.createBuffer({
      size: 32, // 8 floats * 4 bytes
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });

    this.bindGroup = this.device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: this.uniformBuffer } }],
    });
  }

  initWebGL2() {
    const gl = this.gl;

    const vsSource = `#version 300 es
      out vec2 vUv;
      void main() {
        vec2 pos[3] = vec2[3](vec2(-1,-1), vec2(3,-1), vec2(-1,3));
        gl_Position = vec4(pos[gl_VertexID], 0, 1);
        vUv = pos[gl_VertexID] * 0.5 + 0.5;
        vUv.y = 1.0 - vUv.y;
      }
    `;

    const fsSource = `#version 300 es
      precision highp float;
      in vec2 vUv;
      out vec4 fragColor;
      uniform vec2 uRes;
      uniform float uTime;
      uniform float uAudio;
      uniform float uClouds;
      uniform float uGlow;
      uniform float uScheme;

      float hash(vec2 p) {
        vec3 p3 = fract(vec3(p.xyx) * 0.13);
        p3 += dot(p3, p3.yzx + 3.333);
        return fract(p3.x * p3.y * p3.z);
      }

      float noise(vec2 p) {
        vec2 i = floor(p), f = fract(p);
        f = f * f * (3.0 - 2.0 * f);
        return mix(mix(hash(i), hash(i+vec2(1,0)), f.x), mix(hash(i+vec2(0,1)), hash(i+vec2(1,1)), f.x), f.y);
      }

      vec3 pal(float t, float s) {
        vec3 a,b,c,d;
        if(s<0.5){a=vec3(.5,.2,.6);b=vec3(.5,.3,.4);c=vec3(1);d=vec3(.8,.3,.5);}
        else if(s<1.5){a=vec3(.2,.5,.6);b=vec3(.3,.4,.4);c=vec3(1);d=vec3(0,.5,.6);}
        else{a=vec3(.5,.2,.1);b=vec3(.5,.3,.2);c=vec3(1,1,.5);d=vec3(0,.1,.2);}
        return a+b*cos(6.28318*(c*t+d));
      }

      void main() {
        float asp = uRes.x/uRes.y;
        vec2 uv = vUv; uv.x = (uv.x-.5)*asp+.5;
        vec2 c = vec2(.5);
        float t = uTime, au = min(uAudio*3.,1.);
        vec3 col = vec3(0); float alph = 0.;

        float d = length(uv-c);
        float amb = 1.-smoothstep(0.,.4+au*.2,d);
        col += pal(.5,uScheme)*.3*amb*(.15+sin(t*.5)*.1+au*.5)*uGlow;

        for(float i=0.;i<7.;i++){
          if(i>=uClouds)break;
          float ang = i*6.28318/uClouds+t*(.3+mod(i,2.)*.2);
          float dst = .05+au*.3+sin(t*.8+i)*.08;
          vec2 p = c+vec2(cos(ang),sin(ang))*dst;
          float cd = length(uv-p);
          float sz = .1*au+.05;
          float n = noise(uv*3.+t*.1)*.3;
          float cld = 1.-smoothstep(0.,sz*(1.+n),cd);
          cld = cld*cld*(.3+au*.5);
          col += pal(i/uClouds+t*.05,uScheme)*cld*uGlow;
          alph = max(alph,cld);
        }

        float csz = .02+au*.15;
        float core = 1.-smoothstep(0.,csz,d);
        core = core*core*(.4+au*.6);
        col += mix(vec3(1),pal(.3,uScheme),smoothstep(0.,csz,d))*core*uGlow;
        alph = max(alph,core);

        col *= 1.-smoothstep(.3,.8,d*1.5);
        fragColor = vec4(col, clamp(alph+length(col)*.5,0.,1.));
      }
    `;

    const vs = gl.createShader(gl.VERTEX_SHADER);
    gl.shaderSource(vs, vsSource);
    gl.compileShader(vs);

    const fs = gl.createShader(gl.FRAGMENT_SHADER);
    gl.shaderSource(fs, fsSource);
    gl.compileShader(fs);

    this.glProgram = gl.createProgram();
    gl.attachShader(this.glProgram, vs);
    gl.attachShader(this.glProgram, fs);
    gl.linkProgram(this.glProgram);

    this.glUniforms = {
      res: gl.getUniformLocation(this.glProgram, 'uRes'),
      time: gl.getUniformLocation(this.glProgram, 'uTime'),
      audio: gl.getUniformLocation(this.glProgram, 'uAudio'),
      clouds: gl.getUniformLocation(this.glProgram, 'uClouds'),
      glow: gl.getUniformLocation(this.glProgram, 'uGlow'),
      scheme: gl.getUniformLocation(this.glProgram, 'uScheme'),
    };

    this.glVAO = gl.createVertexArray();
  }

  resize() {
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();
    this.canvas.width = rect.width * dpr;
    this.canvas.height = rect.height * dpr;
  }

  render(audioLevel, config) {
    const time = (performance.now() - this.startTime) / 1000;
    const w = this.canvas.width;
    const h = this.canvas.height;

    const schemes = { purple: 0, cyan: 1, fire: 2, aurora: 3, cosmic: 4 };
    const qualities = { low: 1, medium: 2, high: 3, ultra: 4 };

    if (this.isWebGPU) {
      const data = new Float32Array([
        w, h, time, audioLevel,
        config.cloudCount, config.glowIntensity,
        schemes[config.colorScheme] || 0,
        qualities[config.quality] || 3,
      ]);
      this.device.queue.writeBuffer(this.uniformBuffer, 0, data);

      const cmd = this.device.createCommandEncoder();
      const pass = cmd.beginRenderPass({
        colorAttachments: [{
          view: this.context.getCurrentTexture().createView(),
          loadOp: 'clear',
          storeOp: 'store',
          clearValue: { r: 0, g: 0, b: 0, a: 0 },
        }],
      });
      pass.setPipeline(this.pipeline);
      pass.setBindGroup(0, this.bindGroup);
      pass.draw(3);
      pass.end();
      this.device.queue.submit([cmd.finish()]);
    } else if (this.gl) {
      const gl = this.gl;
      gl.viewport(0, 0, w, h);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

      gl.useProgram(this.glProgram);
      gl.bindVertexArray(this.glVAO);
      gl.uniform2f(this.glUniforms.res, w, h);
      gl.uniform1f(this.glUniforms.time, time);
      gl.uniform1f(this.glUniforms.audio, audioLevel);
      gl.uniform1f(this.glUniforms.clouds, config.cloudCount);
      gl.uniform1f(this.glUniforms.glow, config.glowIntensity);
      gl.uniform1f(this.glUniforms.scheme, schemes[config.colorScheme] || 0);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }
  }

  start() {
    // Nebula uses the shared render loop
  }

  stop() {
    // Nebula uses the shared render loop
  }
}

// ============================================================================
// GENESIS RENDERER - Fire Orb Born Every Activation
// Same architecture as NebulaRenderer: WebGPU first, WebGL2 fallback
// ============================================================================

class GenesisOrbRenderer {
  constructor(canvas) {
    this.canvas = canvas;
    this.device = null;
    this.context = null;
    this.pipeline = null;
    this.uniformBuffer = null;
    this.bindGroup = null;
    this.startTime = performance.now();
    this.isWebGPU = false;

    // Fallback WebGL2
    this.gl = null;
    this.glProgram = null;
    this.glUniforms = {};
  }

  async init(sharedGPU) {
    // Use shared WebGPU if available
    if (sharedGPU) {
      this.device = sharedGPU.device;
      this.context = sharedGPU.context;
      await this.initWebGPU(sharedGPU.format);
      this.isWebGPU = true;
      console.log('[GenesisOrb] Using shared WebGPU');
      return true;
    }

    // Fallback to WebGL2
    this.gl = this.canvas.getContext('webgl2', {
      alpha: true,
      premultipliedAlpha: true,
      antialias: true,
    });

    if (this.gl) {
      this.initWebGL2();
      console.log('[GenesisOrb] WebGL2 fallback initialized');
      return true;
    }

    console.error('[GenesisOrb] No GPU rendering available');
    return false;
  }

  async initWebGPU(format) {
    const shaderCode = `
      struct Uniforms {
        resolution: vec2<f32>,
        time: f32,
        audioLevel: f32,
        genesis: f32,
        padding1: f32,
        padding2: f32,
        padding3: f32,
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

      fn hash(p: vec2<f32>) -> f32 {
        let p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
        return fract((p3.x + p3.y + p3.z) * (p3.x + p3.y) * 33.33);
      }

      fn noise(p: vec2<f32>) -> f32 {
        let i = floor(p);
        let f = fract(p);
        let uf = f * f * (3.0 - 2.0 * f);
        return mix(
          mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), uf.x),
          mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), uf.x),
          uf.y
        );
      }

      fn palette(t: f32) -> vec3<f32> {
        // Warm fire palette - orange, red, yellow
        let a = vec3<f32>(0.5, 0.2, 0.1);
        let b = vec3<f32>(0.5, 0.3, 0.2);
        let c = vec3<f32>(1.0, 1.0, 0.5);
        let d = vec3<f32>(0.0, 0.15, 0.2);
        return a + b * cos(6.28318 * (c * t + d));
      }

      @fragment
      fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
        let aspect = u.resolution.x / u.resolution.y;
        var uv = in.uv;
        uv.x = (uv.x - 0.5) * aspect + 0.5;

        let center = vec2<f32>(0.5, 0.5);
        let t = u.time;
        let genesis = u.genesis;
        let audio = min(u.audioLevel * 3.0, 1.0);

        var col = vec3<f32>(0.0);
        let dist = length(uv - center);

        // Breathing animation
        let breathe = sin(t * 0.5) * 0.5 + 0.5;

        // === DORMANT SPARK (genesis = 0) ===
        let sparkSize = 0.008 + breathe * 0.004;
        let spark = 1.0 - smoothstep(0.0, sparkSize, dist);
        let sparkGlow = 1.0 - smoothstep(0.0, 0.08, dist);

        var sparkColor = vec3<f32>(0.97, 0.45, 0.09) * spark * 2.0;
        sparkColor += vec3<f32>(0.9, 0.3, 0.1) * sparkGlow * 0.3 * (0.3 + breathe * 0.2);

        // === FULLY ALIVE ORB (genesis = 1) ===
        let orbSize = 0.12 + audio * 0.15 + breathe * 0.02;

        // Multi-layer orb
        let core = 1.0 - smoothstep(0.0, orbSize * 0.3, dist);
        let inner = 1.0 - smoothstep(0.0, orbSize * 0.6, dist);
        let outer = 1.0 - smoothstep(0.0, orbSize, dist);
        let glow = 1.0 - smoothstep(0.0, orbSize * 2.5, dist);

        // Nebula clouds
        var clouds = 0.0;
        for (var i = 0.0; i < 7.0; i += 1.0) {
          let angle = i * 6.28318 / 7.0 + t * (0.3 + i * 0.05);
          let cdist = orbSize * 0.7 + sin(t * 0.5 + i) * 0.03;
          let cloudPos = center + vec2<f32>(cos(angle), sin(angle)) * cdist;
          let cd = length(uv - cloudPos);
          let cloud = 1.0 - smoothstep(0.0, 0.06 + audio * 0.04, cd);
          clouds += cloud * 0.3;
        }

        // Particle sparkles
        var sparkle = 0.0;
        for (var i = 0.0; i < 30.0; i += 1.0) {
          let seed = sin(i * 12.9898) * 43758.5453;
          let pAngle = fract(seed) * 6.28318 + t * (0.1 + fract(seed * 2.3) * 0.2);
          let pDist = fract(seed * 3.7) * 0.35 + 0.05;
          let pPos = center + vec2<f32>(cos(pAngle), sin(pAngle)) * pDist;
          let twinkle = sin(t * (2.0 + fract(seed * 5.1) * 3.0) + i);
          let pAlpha = max(0.0, twinkle) * 0.5;
          let pd = length(uv - pPos);
          sparkle += smoothstep(0.004, 0.0, pd) * pAlpha;
        }

        // Audio ripples
        var ripples = 0.0;
        if (audio > 0.05) {
          for (var i = 0.0; i < 3.0; i += 1.0) {
            let ripple = orbSize * (1.0 + i * 0.3) + audio * 0.2;
            let ring = smoothstep(0.02, 0.0, abs(dist - ripple));
            ripples += ring * (1.0 - i * 0.3) * audio;
          }
        }

        // Compose alive orb color
        var aliveColor = vec3<f32>(0.0);
        aliveColor += vec3<f32>(1.0, 0.95, 0.8) * core * 2.0;
        aliveColor += vec3<f32>(1.0, 0.6, 0.2) * inner * 0.8;
        aliveColor += palette(0.5 + sin(t * 0.1) * 0.2) * outer * 0.5;
        aliveColor += palette(0.3) * glow * 0.25;
        aliveColor += palette(0.6 + t * 0.05) * clouds;
        aliveColor += vec3<f32>(1.0, 0.9, 0.7) * sparkle;
        aliveColor += vec3<f32>(0.97, 0.45, 0.09) * ripples;

        // Outer ring
        let ring = smoothstep(0.01, 0.0, abs(dist - orbSize * 1.3));
        aliveColor += vec3<f32>(0.97, 0.45, 0.09) * ring * 0.3;

        // Blend between spark and alive
        col = mix(sparkColor, aliveColor, genesis);

        // Vignette
        let vignetteStrength = mix(1.5, 1.0, genesis);
        col *= 1.0 - smoothstep(0.3, 0.9, dist * vignetteStrength);

        // Alpha calculation
        var alpha = 0.0;
        alpha = max(alpha, spark * (1.0 - genesis));
        alpha = max(alpha, sparkGlow * 0.5 * (1.0 - genesis));
        alpha = max(alpha, (core + inner * 0.5 + outer * 0.3 + glow * 0.2) * genesis);
        alpha = max(alpha, clouds * 0.5 * genesis);
        alpha = max(alpha, ripples * genesis);
        alpha = clamp(alpha + length(col) * 0.3, 0.0, 1.0);

        return vec4<f32>(col, alpha);
      }
    `;

    const shaderModule = this.device.createShaderModule({ code: shaderCode });

    this.pipeline = this.device.createRenderPipeline({
      layout: 'auto',
      vertex: { module: shaderModule, entryPoint: 'vs_main' },
      fragment: {
        module: shaderModule,
        entryPoint: 'fs_main',
        targets: [{
          format,
          blend: {
            color: { srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha', operation: 'add' },
            alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha', operation: 'add' },
          },
        }],
      },
      primitive: { topology: 'triangle-list' },
    });

    this.uniformBuffer = this.device.createBuffer({
      size: 32, // 8 floats * 4 bytes
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });

    this.bindGroup = this.device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: this.uniformBuffer } }],
    });
  }

  initWebGL2() {
    const gl = this.gl;

    const vsSource = `#version 300 es
      out vec2 vUv;
      void main() {
        vec2 pos[3] = vec2[3](vec2(-1,-1), vec2(3,-1), vec2(-1,3));
        gl_Position = vec4(pos[gl_VertexID], 0, 1);
        vUv = pos[gl_VertexID] * 0.5 + 0.5;
        vUv.y = 1.0 - vUv.y;
      }
    `;

    const fsSource = `#version 300 es
      precision highp float;
      in vec2 vUv;
      out vec4 fragColor;

      uniform vec2 uRes;
      uniform float uTime;
      uniform float uGenesis;
      uniform float uAudio;

      float hash(vec2 p) {
        vec3 p3 = fract(vec3(p.xyx) * 0.1031);
        p3 += dot(p3, p3.yzx + 33.33);
        return fract((p3.x + p3.y) * p3.z);
      }

      float noise(vec2 p) {
        vec2 i = floor(p);
        vec2 f = fract(p);
        f = f * f * (3.0 - 2.0 * f);
        return mix(
          mix(hash(i), hash(i + vec2(1,0)), f.x),
          mix(hash(i + vec2(0,1)), hash(i + vec2(1,1)), f.x),
          f.y
        );
      }

      vec3 palette(float t) {
        vec3 a = vec3(0.5, 0.2, 0.1);
        vec3 b = vec3(0.5, 0.3, 0.2);
        vec3 c = vec3(1.0, 1.0, 0.5);
        vec3 d = vec3(0.0, 0.15, 0.2);
        return a + b * cos(6.28318 * (c * t + d));
      }

      void main() {
        float asp = uRes.x / uRes.y;
        vec2 uv = vUv;
        uv.x = (uv.x - 0.5) * asp + 0.5;

        vec2 center = vec2(0.5);
        float t = uTime;
        float genesis = uGenesis;
        float audio = min(uAudio * 3.0, 1.0);

        vec3 col = vec3(0.0);
        float d = length(uv - center);

        float breathe = sin(t * 0.5) * 0.5 + 0.5;

        // Dormant spark
        float sparkSize = 0.008 + breathe * 0.004;
        float spark = 1.0 - smoothstep(0.0, sparkSize, d);
        float sparkGlow = 1.0 - smoothstep(0.0, 0.08, d);

        vec3 sparkColor = vec3(0.97, 0.45, 0.09) * spark * 2.0;
        sparkColor += vec3(0.9, 0.3, 0.1) * sparkGlow * 0.3 * (0.3 + breathe * 0.2);

        // Alive orb
        float orbSize = 0.12 + audio * 0.15 + breathe * 0.02;

        float core = 1.0 - smoothstep(0.0, orbSize * 0.3, d);
        float inner = 1.0 - smoothstep(0.0, orbSize * 0.6, d);
        float outer = 1.0 - smoothstep(0.0, orbSize, d);
        float glow = 1.0 - smoothstep(0.0, orbSize * 2.5, d);

        float clouds = 0.0;
        for (float i = 0.0; i < 7.0; i++) {
          float angle = i * 6.28318 / 7.0 + t * (0.3 + i * 0.05);
          float cdist = orbSize * 0.7 + sin(t * 0.5 + i) * 0.03;
          vec2 cloudPos = center + vec2(cos(angle), sin(angle)) * cdist;
          float cd = length(uv - cloudPos);
          float cloud = 1.0 - smoothstep(0.0, 0.06 + audio * 0.04, cd);
          clouds += cloud * 0.3;
        }

        float sparkle = 0.0;
        for (float i = 0.0; i < 30.0; i++) {
          float seed = sin(i * 12.9898) * 43758.5453;
          float pAngle = fract(seed) * 6.28318 + t * (0.1 + fract(seed * 2.3) * 0.2);
          float pDist = fract(seed * 3.7) * 0.35 + 0.05;
          vec2 pPos = center + vec2(cos(pAngle), sin(pAngle)) * pDist;
          float twinkle = sin(t * (2.0 + fract(seed * 5.1) * 3.0) + i);
          float pAlpha = max(0.0, twinkle) * 0.5;
          float pd = length(uv - pPos);
          sparkle += smoothstep(0.004, 0.0, pd) * pAlpha;
        }

        float ripples = 0.0;
        if (audio > 0.05) {
          for (float i = 0.0; i < 3.0; i++) {
            float ripple = orbSize * (1.0 + i * 0.3) + audio * 0.2;
            float ring = smoothstep(0.02, 0.0, abs(d - ripple));
            ripples += ring * (1.0 - i * 0.3) * audio;
          }
        }

        vec3 aliveColor = vec3(0.0);
        aliveColor += vec3(1.0, 0.95, 0.8) * core * 2.0;
        aliveColor += vec3(1.0, 0.6, 0.2) * inner * 0.8;
        aliveColor += palette(0.5 + sin(t * 0.1) * 0.2) * outer * 0.5;
        aliveColor += palette(0.3) * glow * 0.25;
        aliveColor += palette(0.6 + t * 0.05) * clouds;
        aliveColor += vec3(1.0, 0.9, 0.7) * sparkle;
        aliveColor += vec3(0.97, 0.45, 0.09) * ripples;

        float ring = smoothstep(0.01, 0.0, abs(d - orbSize * 1.3));
        aliveColor += vec3(0.97, 0.45, 0.09) * ring * 0.3;

        col = mix(sparkColor, aliveColor, genesis);

        float vignetteStrength = mix(1.5, 1.0, genesis);
        col *= 1.0 - smoothstep(0.3, 0.9, d * vignetteStrength);

        float alpha = 0.0;
        alpha = max(alpha, spark * (1.0 - genesis));
        alpha = max(alpha, sparkGlow * 0.5 * (1.0 - genesis));
        alpha = max(alpha, (core + inner * 0.5 + outer * 0.3 + glow * 0.2) * genesis);
        alpha = max(alpha, clouds * 0.5 * genesis);
        alpha = max(alpha, ripples * genesis);
        alpha = clamp(alpha + length(col) * 0.3, 0.0, 1.0);

        fragColor = vec4(col, alpha);
      }
    `;

    const vs = gl.createShader(gl.VERTEX_SHADER);
    gl.shaderSource(vs, vsSource);
    gl.compileShader(vs);

    const fs = gl.createShader(gl.FRAGMENT_SHADER);
    gl.shaderSource(fs, fsSource);
    gl.compileShader(fs);

    if (!gl.getShaderParameter(fs, gl.COMPILE_STATUS)) {
      console.error('[GenesisOrb] Fragment shader error:', gl.getShaderInfoLog(fs));
    }

    this.glProgram = gl.createProgram();
    gl.attachShader(this.glProgram, vs);
    gl.attachShader(this.glProgram, fs);
    gl.linkProgram(this.glProgram);

    this.glUniforms = {
      res: gl.getUniformLocation(this.glProgram, 'uRes'),
      time: gl.getUniformLocation(this.glProgram, 'uTime'),
      genesis: gl.getUniformLocation(this.glProgram, 'uGenesis'),
      audio: gl.getUniformLocation(this.glProgram, 'uAudio'),
    };

    this.glVAO = gl.createVertexArray();
  }

  resize() {
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();
    this.canvas.width = rect.width * dpr;
    this.canvas.height = rect.height * dpr;
  }

  render() {
    const time = (performance.now() - this.startTime) / 1000;
    const w = this.canvas.width;
    const h = this.canvas.height;

    // Smooth genesis level transitions
    const genesisSpeed = state.targetGenesisLevel > state.genesisLevel ? 0.08 : 0.04;
    state.genesisLevel += (state.targetGenesisLevel - state.genesisLevel) * genesisSpeed;

    if (this.isWebGPU) {
      const data = new Float32Array([
        w, h, time, state.audioLevel,
        state.genesisLevel, 0, 0, 0,
      ]);
      this.device.queue.writeBuffer(this.uniformBuffer, 0, data);

      const cmd = this.device.createCommandEncoder();
      const pass = cmd.beginRenderPass({
        colorAttachments: [{
          view: this.context.getCurrentTexture().createView(),
          loadOp: 'clear',
          storeOp: 'store',
          clearValue: { r: 0, g: 0, b: 0, a: 0 },
        }],
      });
      pass.setPipeline(this.pipeline);
      pass.setBindGroup(0, this.bindGroup);
      pass.draw(3);
      pass.end();
      this.device.queue.submit([cmd.finish()]);
    } else if (this.gl) {
      const gl = this.gl;
      gl.viewport(0, 0, w, h);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

      gl.useProgram(this.glProgram);
      gl.bindVertexArray(this.glVAO);
      gl.uniform2f(this.glUniforms.res, w, h);
      gl.uniform1f(this.glUniforms.time, time);
      gl.uniform1f(this.glUniforms.genesis, state.genesisLevel);
      gl.uniform1f(this.glUniforms.audio, state.audioLevel);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }
  }

  start() {
    // Genesis uses the shared render loop
  }

  stop() {
    // Genesis uses the shared render loop
  }
}

// ============================================================================
// RENDERER MANAGEMENT
// ============================================================================

let nebulaRenderer = null;
let genesisRenderer = null;
let animationId = null;
let sharedGPU = null; // Shared WebGPU resources

async function initRenderers() {
  // Initialize shared WebGPU resources first
  if (navigator.gpu) {
    try {
      const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
      if (adapter) {
        const device = await adapter.requestDevice();
        const context = elements.canvas.getContext('webgpu');
        const format = navigator.gpu.getPreferredCanvasFormat();

        context.configure({
          device,
          format,
          alphaMode: 'premultiplied',
        });

        sharedGPU = { device, context, format };
        console.log('[qVoice] Shared WebGPU initialized');
      }
    } catch (e) {
      console.warn('[qVoice] WebGPU failed, using WebGL2:', e);
    }
  }

  nebulaRenderer = new NebulaRenderer(elements.canvas);
  await nebulaRenderer.init(sharedGPU);

  genesisRenderer = new GenesisOrbRenderer(elements.canvas);
  await genesisRenderer.init(sharedGPU);

  nebulaRenderer.resize();
  genesisRenderer.resize();
}

function startRenderLoop() {
  const loop = () => {
    if (state.orbMode === 'nebula') {
      nebulaRenderer.render(state.audioLevel, state.config);
    } else {
      genesisRenderer.render();
    }
    animationId = requestAnimationFrame(loop);
  };
  loop();
}

function toggleOrbMode() {
  if (state.orbMode === 'nebula') {
    state.orbMode = 'genesis';
    // Reset genesis state when switching to genesis mode
    state.genesisLevel = 0;
    state.targetGenesisLevel = state.isRecording ? 1 : 0;
    showStatus('Genesis Mode (fire)', 'success');
  } else {
    state.orbMode = 'nebula';
    showStatus('Nebula Mode (purple)', 'success');
  }
  console.log(`[qVoice] Switched to ${state.orbMode} mode`);
}

// ============================================================================
// AUDIO POLLING
// ============================================================================

let audioPollingInterval = null;

function startAudioPolling() {
  if (audioPollingInterval) return;

  audioPollingInterval = setInterval(async () => {
    if (!state.isRecording) return;

    try {
      const metrics = await invoke('get_audio_metrics');
      state.audioLevel = state.audioLevel * 0.3 + metrics.level * 0.7;
      if (metrics.fft_bins) {
        state.fftBins = new Float32Array(metrics.fft_bins);
      }
    } catch (e) {
      // Ignore polling errors
    }
  }, 16); // ~60fps
}

function stopAudioPolling() {
  if (audioPollingInterval) {
    clearInterval(audioPollingInterval);
    audioPollingInterval = null;
  }
}

// ============================================================================
// RECORDING
// ============================================================================

async function toggleRecording() {
  if (state.isProcessing) return;

  if (state.isRecording) {
    await stopRecording();
  } else {
    await startRecording();
  }
}

async function startRecording() {
  try {
    const result = await invoke('start_recording', {
      deviceId: state.selectedDevice || null,
    });

    if (!result.success) {
      showStatus(`Error: ${result.error}`, 'error');
      return;
    }

    state.isRecording = true;
    // Switch to genesis (fire) mode and birth the orb
    state.orbMode = 'genesis';
    state.genesisLevel = 0;
    state.targetGenesisLevel = 1;
    updateRecordButton();
    startAudioPolling();
    hideTranscription();
    showStatus('Recording...', 'recording');
  } catch (e) {
    showStatus(`Error: ${e}`, 'error');
  }
}

async function stopRecording() {
  try {
    stopAudioPolling();

    const result = await invoke('stop_recording');

    if (!result.success) {
      state.isRecording = false;
      state.orbMode = 'nebula';
      state.targetGenesisLevel = 0;
      updateRecordButton();
      showStatus(`Error: ${result.error}`, 'error');
      return;
    }

    state.isRecording = false;
    state.isProcessing = true;
    // Keep orb alive during processing (genesis mode)
    updateRecordButton();
    showStatus('Transcribing...', 'processing');

    // Transcribe
    const transcription = await invoke('transcribe');

    state.isProcessing = false;
    // Switch back to nebula (purple) mode
    state.orbMode = 'nebula';
    state.targetGenesisLevel = 0;
    updateRecordButton();

    if (transcription.success && transcription.text) {
      state.lastTranscription = transcription.text;
      showTranscription(transcription.text);
      showStatus(`Done in ${transcription.processing_ms}ms`, 'success');

      // Copy to clipboard
      await navigator.clipboard.writeText(transcription.text);
    } else {
      showStatus(transcription.error || 'No speech detected', 'error');
    }

    // Decay audio level
    const decay = () => {
      state.audioLevel *= 0.85;
      if (state.audioLevel > 0.01) {
        requestAnimationFrame(decay);
      } else {
        state.audioLevel = 0;
      }
    };
    decay();
  } catch (e) {
    state.isRecording = false;
    state.isProcessing = false;
    state.orbMode = 'nebula';
    state.targetGenesisLevel = 0;
    updateRecordButton();
    showStatus(`Error: ${e}`, 'error');
  }
}

function updateRecordButton() {
  const btn = elements.btnRecord;
  const iconMic = elements.iconMic;
  const iconStop = elements.iconStop;
  const iconSpinner = elements.iconSpinner;

  btn.classList.remove('recording', 'processing');
  iconMic.style.display = 'none';
  iconStop.style.display = 'none';
  iconSpinner.style.display = 'none';

  if (state.isProcessing) {
    btn.classList.add('processing');
    iconSpinner.style.display = 'block';
  } else if (state.isRecording) {
    btn.classList.add('recording');
    iconStop.style.display = 'block';
  } else {
    iconMic.style.display = 'block';
  }
}

// ============================================================================
// UI HELPERS
// ============================================================================

function showStatus(message, type = 'info') {
  elements.status.textContent = message;
  elements.status.className = `visible ${type}`;

  if (type !== 'recording' && type !== 'processing') {
    setTimeout(() => {
      elements.status.classList.remove('visible');
    }, 3000);
  }
}

function showTranscription(text) {
  elements.transcriptionText.textContent = text;
  elements.transcription.classList.remove('hidden');
}

function hideTranscription() {
  elements.transcription.classList.add('hidden');
}

function showSettings() {
  elements.settingsPanel.classList.remove('hidden');
}

function hideSettings() {
  elements.settingsPanel.classList.add('hidden');
}

// ============================================================================
// SETTINGS
// ============================================================================

async function loadDevices() {
  try {
    const result = await invoke('list_audio_devices');
    if (result.success) {
      state.devices = result.devices;

      elements.selectDevice.innerHTML = '<option value="">System Default</option>';
      for (const device of result.devices) {
        const option = document.createElement('option');
        option.value = device.id;
        option.textContent = device.name + (device.is_default ? ' (Default)' : '');
        elements.selectDevice.appendChild(option);
      }
    }
  } catch (e) {
    console.error('Failed to load devices:', e);
  }
}

async function loadModels() {
  try {
    const result = await invoke('list_models');
    if (result.success) {
      state.models = result.models;
      state.selectedModel = result.current_model;
      elements.selectModel.value = state.selectedModel;
      updateModelStatus();
    }
  } catch (e) {
    console.error('Failed to load models:', e);
  }
}

function updateModelStatus() {
  const model = state.models.find(m => m.id === state.selectedModel);
  if (model) {
    if (model.downloaded) {
      elements.modelStatus.textContent = 'Downloaded';
      elements.modelStatus.className = 'downloaded';
      elements.btnDownloadModel.style.display = 'none';
    } else {
      elements.modelStatus.textContent = `${model.size_mb} MB - Not downloaded`;
      elements.modelStatus.className = '';
      elements.btnDownloadModel.style.display = 'block';
    }
  }
}

async function downloadModel() {
  const modelId = state.selectedModel;
  elements.btnDownloadModel.disabled = true;
  elements.btnDownloadModel.textContent = 'Downloading...';
  elements.modelStatus.textContent = 'Starting download...';
  elements.modelStatus.className = 'downloading';

  try {
    await invoke('download_model', { modelId });
    await loadModels();
    elements.btnDownloadModel.textContent = 'Download';
  } catch (e) {
    showStatus(`Download failed: ${e}`, 'error');
    elements.btnDownloadModel.textContent = 'Retry';
  }

  elements.btnDownloadModel.disabled = false;
}

async function setModel(modelId) {
  state.selectedModel = modelId;
  await invoke('set_model', { modelId });
  updateModelStatus();
}

// ============================================================================
// EVENT LISTENERS
// ============================================================================

function setupEventListeners() {
  // Record button
  elements.btnRecord.addEventListener('click', toggleRecording);

  // Window drag functionality
  let isDragging = false;
  let dragOffset = { x: 0, y: 0 };

  elements.dragHandle.addEventListener('mousedown', (e) => {
    isDragging = true;
    dragOffset.x = e.clientX;
    dragOffset.y = e.clientY;
    elements.dragHandle.style.background = 'rgba(255,255,255,0.15)';
  });

  window.addEventListener('mousemove', (e) => {
    if (isDragging) {
      appWindow.setPosition({
        x: e.screenX - dragOffset.x,
        y: e.screenY - dragOffset.y
      });
    }
  });

  window.addEventListener('mouseup', () => {
    if (isDragging) {
      isDragging = false;
      elements.dragHandle.style.background = '';
    }
  });

  // Settings
  elements.btnSettings.addEventListener('click', showSettings);
  elements.btnCloseSettings.addEventListener('click', hideSettings);

  // Close app
  elements.btnClose.addEventListener('click', () => appWindow.close());

  // Copy button
  elements.btnCopy.addEventListener('click', async () => {
    await navigator.clipboard.writeText(state.lastTranscription);
    showStatus('Copied to clipboard', 'success');
  });

  // Download model
  elements.btnDownloadModel.addEventListener('click', downloadModel);

  // Settings changes
  elements.selectModel.addEventListener('change', (e) => {
    setModel(e.target.value);
  });

  elements.selectDevice.addEventListener('change', (e) => {
    state.selectedDevice = e.target.value;
  });

  elements.selectColor.addEventListener('change', (e) => {
    state.config.colorScheme = e.target.value;
  });

  elements.selectQuality.addEventListener('change', (e) => {
    state.config.quality = e.target.value;
  });

  // Keyboard shortcuts
  document.addEventListener('keydown', (e) => {
    // Space to toggle recording
    if (e.code === 'Space' && !e.repeat) {
      e.preventDefault();
      toggleRecording();
    }
    // Escape to close panels
    if (e.code === 'Escape') {
      hideSettings();
      hideTranscription();
    }
    // Option/Alt + S to toggle orb mode
    if (e.code === 'KeyS' && e.altKey) {
      e.preventDefault();
      toggleOrbMode();
    }
  });

  // Window resize
  window.addEventListener('resize', () => {
    if (nebulaRenderer) nebulaRenderer.resize();
    if (genesisRenderer) genesisRenderer.resize();
    // Reconfigure shared WebGPU context after resize
    if (sharedGPU) {
      sharedGPU.context.configure({
        device: sharedGPU.device,
        format: sharedGPU.format,
        alphaMode: 'premultiplied',
      });
    }
  });

  // Listen for download progress
  listen('download-progress', (event) => {
    const { percent } = event.payload;
    elements.modelStatus.textContent = `Downloading... ${percent.toFixed(0)}%`;
  });
}

// ============================================================================
// INITIALIZATION
// ============================================================================

async function init() {
  console.log('[qVoice] Initializing with dual orb modes...');

  // Initialize both renderers
  await initRenderers();

  // Start render loop
  startRenderLoop();

  // Setup UI
  setupEventListeners();

  // Load settings
  await loadDevices();
  await loadModels();

  console.log('[qVoice] Ready - Press Ctrl+S to toggle between Nebula (purple) and Genesis (fire) modes');
}

// Start when DOM is ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
