/**
 * Fire Orb v3 - WebGL2 Shader-based (onboarding style)
 *
 * Tiny breathing ember -> BIRTH -> Full fire orb -> DEATH -> back to ember
 */

const canvas = document.getElementById('canvas');
const gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: true });

let program, vao, uniforms;
let startTime = performance.now();
let genesisLevel = 0;
let targetGenesisLevel = 0;
let audioLevel = 0;
let targetAudioLevel = 0;
let globalOpacity = 0;
let targetOpacity = 1;
let isRecording = false;
let isProcessing = false;

// Rust API
window.setAudioLevel = (level) => { targetAudioLevel = level; };
window.setRecordingState = (recording, processing) => {
  isRecording = recording;
  isProcessing = processing;
  if (recording) targetGenesisLevel = 1;
};
window.setTranscriptionComplete = () => {
  isProcessing = false;
  targetGenesisLevel = 0;
};
window.fadeIn = () => { targetOpacity = 1; };
window.fadeOut = () => { targetOpacity = 0; targetGenesisLevel = 0; };
window.resetOrb = () => {
  genesisLevel = 0;
  targetGenesisLevel = 0;
  audioLevel = 0;
  globalOpacity = 0;
  targetOpacity = 1;
};

const vsSource = `#version 300 es
out vec2 vUv;
void main() {
  vec2 pos[3] = vec2[3](vec2(-1,-1), vec2(3,-1), vec2(-1,3));
  gl_Position = vec4(pos[gl_VertexID], 0, 1);
  vUv = pos[gl_VertexID] * 0.5 + 0.5;
}`;

const fsSource = `#version 300 es
precision highp float;
in vec2 vUv;
out vec4 fragColor;

uniform vec2 uRes;
uniform float uTime;
uniform float uGenesis;
uniform float uAudio;
uniform float uOpacity;

#define PI 3.14159265359

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

vec3 fireColor(float t) {
  vec3 a = vec3(0.5, 0.2, 0.1);
  vec3 b = vec3(0.5, 0.3, 0.2);
  vec3 c = vec3(1.0, 1.0, 0.5);
  vec3 d = vec3(0.0, 0.15, 0.2);
  return a + b * cos(6.28318 * (c * t + d));
}

void main() {
  vec2 uv = vUv;
  float aspect = uRes.x / uRes.y;
  uv.x = (uv.x - 0.5) * aspect + 0.5;

  vec2 center = vec2(0.5);
  float t = uTime;
  float genesis = uGenesis;
  float audio = uAudio;

  float d = length(uv - center);
  float breathe = sin(t * 0.8) * 0.5 + 0.5;

  vec3 col = vec3(0.0);
  float alpha = 0.0;

  // === SPARK (always visible, fades as genesis increases) ===
  float sparkIntensity = 1.0 - genesis;
  float sparkSize = 0.012 + breathe * 0.006;
  float spark = smoothstep(sparkSize, 0.0, d);
  float sparkGlow = smoothstep(0.12, 0.0, d);

  vec3 sparkCol = vec3(1.0, 0.6, 0.2) * spark * 2.5;
  sparkCol += vec3(1.0, 0.4, 0.1) * sparkGlow * 0.4 * (0.5 + breathe * 0.3);

  col += sparkCol * sparkIntensity;
  alpha += (spark + sparkGlow * 0.3) * sparkIntensity;

  // === ALIVE ORB (fades in as genesis increases) ===
  float aliveIntensity = genesis;
  float orbSize = 0.08 + audio * 0.12 + breathe * 0.02;

  // Multi-layer core
  float core = smoothstep(orbSize * 0.4, 0.0, d);
  float inner = smoothstep(orbSize * 0.7, 0.0, d);
  float outer = smoothstep(orbSize, 0.0, d);
  float glow = smoothstep(orbSize * 2.0, 0.0, d);

  // Swirling fire clouds
  float clouds = 0.0;
  for (float i = 0.0; i < 7.0; i++) {
    float angle = i * PI * 2.0 / 7.0 + t * (0.4 + i * 0.08);
    float dist = orbSize * 0.6 + sin(t * 0.6 + i) * 0.025;
    dist += audio * 0.05;
    vec2 cloudPos = center + vec2(cos(angle), sin(angle)) * dist;
    float cd = length(uv - cloudPos);
    float cloud = smoothstep(0.05 + audio * 0.03, 0.0, cd);
    clouds += cloud * 0.35;
  }

  // Particles
  float particles = 0.0;
  for (float i = 0.0; i < 20.0; i++) {
    float seed = sin(i * 12.9898) * 43758.5453;
    float pAngle = fract(seed) * PI * 2.0 + t * (0.15 + fract(seed * 2.3) * 0.25);
    float pDist = fract(seed * 3.7) * 0.25 + 0.05 + audio * 0.1;
    vec2 pPos = center + vec2(cos(pAngle), sin(pAngle)) * pDist;
    float twinkle = sin(t * (2.5 + fract(seed * 5.1) * 3.5) + i);
    float pAlpha = max(0.0, twinkle) * 0.6;
    float pd = length(uv - pPos);
    particles += smoothstep(0.006, 0.0, pd) * pAlpha;
  }

  // Audio ripples
  float ripples = 0.0;
  if (audio > 0.05) {
    for (float i = 0.0; i < 3.0; i++) {
      float rippleR = orbSize * (1.2 + i * 0.35) + audio * 0.15;
      float ring = smoothstep(0.015, 0.0, abs(d - rippleR));
      ripples += ring * (1.0 - i * 0.3) * audio * 1.5;
    }
  }

  // Compose alive orb
  vec3 aliveCol = vec3(0.0);
  aliveCol += vec3(1.0, 0.95, 0.85) * core * 2.5;
  aliveCol += vec3(1.0, 0.55, 0.15) * inner * 1.2;
  aliveCol += fireColor(0.5 + sin(t * 0.15) * 0.2) * outer * 0.7;
  aliveCol += fireColor(0.3) * glow * 0.35;
  aliveCol += fireColor(0.6 + t * 0.05) * clouds;
  aliveCol += vec3(1.0, 0.85, 0.6) * particles;
  aliveCol += vec3(1.0, 0.5, 0.15) * ripples;

  // Outer ring
  float outerRing = smoothstep(0.012, 0.0, abs(d - orbSize * 1.4));
  aliveCol += vec3(1.0, 0.5, 0.1) * outerRing * 0.4;

  col += aliveCol * aliveIntensity;

  float aliveAlpha = core * 0.9 + inner * 0.6 + outer * 0.4 + glow * 0.25;
  aliveAlpha += clouds * 0.4 + particles * 0.5 + ripples * 0.6;
  alpha += aliveAlpha * aliveIntensity;

  // Vignette
  float vig = 1.0 - smoothstep(0.25, 0.7, d);
  col *= vig;

  float edgeMask = 1.0 - smoothstep(0.52, 0.78, d);
  alpha = clamp(alpha, 0.0, 1.0) * edgeMask * uOpacity;
  fragColor = vec4(col * alpha, alpha);
}`;

function init() {
  const vs = gl.createShader(gl.VERTEX_SHADER);
  gl.shaderSource(vs, vsSource);
  gl.compileShader(vs);

  const fs = gl.createShader(gl.FRAGMENT_SHADER);
  gl.shaderSource(fs, fsSource);
  gl.compileShader(fs);

  if (!gl.getShaderParameter(fs, gl.COMPILE_STATUS)) {
    console.error('Shader error:', gl.getShaderInfoLog(fs));
  }

  program = gl.createProgram();
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);

  uniforms = {
    res: gl.getUniformLocation(program, 'uRes'),
    time: gl.getUniformLocation(program, 'uTime'),
    genesis: gl.getUniformLocation(program, 'uGenesis'),
    audio: gl.getUniformLocation(program, 'uAudio'),
    opacity: gl.getUniformLocation(program, 'uOpacity'),
  };

  vao = gl.createVertexArray();
}

function resize() {
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
}

function render() {
  const w = canvas.width;
  const h = canvas.height;
  const time = (performance.now() - startTime) / 1000;

  // Smooth transitions
  genesisLevel += (targetGenesisLevel - genesisLevel) * 0.03;
  audioLevel += (targetAudioLevel - audioLevel) * 0.15;
  globalOpacity += (targetOpacity - globalOpacity) * 0.08;

  gl.viewport(0, 0, w, h);
  gl.clearColor(0, 0, 0, 0);
  gl.clear(gl.COLOR_BUFFER_BIT);
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

  gl.useProgram(program);
  gl.bindVertexArray(vao);

  gl.uniform2f(uniforms.res, w, h);
  gl.uniform1f(uniforms.time, time);
  gl.uniform1f(uniforms.genesis, genesisLevel);
  gl.uniform1f(uniforms.audio, audioLevel);
  gl.uniform1f(uniforms.opacity, globalOpacity);

  gl.drawArrays(gl.TRIANGLES, 0, 3);

  requestAnimationFrame(render);
}

window.addEventListener('resize', resize);
init();
resize();
render();

console.log('[Orb-Fire-v3] WebGL2 shader orb ready');
