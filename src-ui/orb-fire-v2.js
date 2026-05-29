/**
 * Fire Orb v2 - WebGL2 Shader Implementation
 * Based on onboarding genesis star style
 */

const canvas = document.getElementById('canvas');
const gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: true });
const statusEl = document.getElementById('status');

let program, vao, uniforms;
let startTime = performance.now();
let audioLevel = 0;
let targetAudioLevel = 0;
let genesisLevel = 0;
let targetGenesisLevel = 0;
let globalOpacity = 0;
let targetOpacity = 1;
let isRecording = false;
let isProcessing = false;
let isFadingOut = false;

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
window.fadeIn = () => { targetOpacity = 1; isFadingOut = false; };
window.fadeOut = () => { targetOpacity = 0; isFadingOut = true; targetGenesisLevel = 0; };
window.isFadeComplete = () => isFadingOut && globalOpacity < 0.01;
window.resetOrb = () => {
  globalOpacity = 0; targetOpacity = 1; isFadingOut = false;
  isRecording = false; isProcessing = false;
  genesisLevel = 0; targetGenesisLevel = 0;
  audioLevel = 0; targetAudioLevel = 0;
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

vec3 palette(float t) {
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

  vec3 col = vec3(0.0);
  float d = length(uv - center);

  float breathe = sin(t * 0.5) * 0.5 + 0.5;

  // DORMANT SPARK
  float sparkSize = 0.012 + breathe * 0.006;
  float spark = 1.0 - smoothstep(0.0, sparkSize, d);
  float sparkGlow = 1.0 - smoothstep(0.0, 0.12, d);

  vec3 sparkColor = vec3(0.97, 0.45, 0.09) * spark * 2.5;
  sparkColor += vec3(0.95, 0.35, 0.1) * sparkGlow * 0.4 * (0.4 + breathe * 0.3);

  // ALIVE ORB
  float orbSize = 0.15 + audio * 0.2 + breathe * 0.025;

  float core = 1.0 - smoothstep(0.0, orbSize * 0.25, d);
  float inner = 1.0 - smoothstep(0.0, orbSize * 0.5, d);
  float outer = 1.0 - smoothstep(0.0, orbSize, d);
  float glow = 1.0 - smoothstep(0.0, orbSize * 2.0, d);

  float clouds = 0.0;
  for (float i = 0.0; i < 7.0; i++) {
    float angle = i * PI * 2.0 / 7.0 + t * (0.4 + i * 0.06);
    float cdist = orbSize * 0.6 + sin(t * 0.6 + i) * 0.04;
    vec2 cloudPos = center + vec2(cos(angle), sin(angle)) * cdist;
    float cd = length(uv - cloudPos);
    float cloud = 1.0 - smoothstep(0.0, 0.07 + audio * 0.05, cd);
    clouds += cloud * 0.35;
  }

  float sparkle = 0.0;
  for (float i = 0.0; i < 40.0; i++) {
    float seed = sin(i * 12.9898) * 43758.5453;
    float pAngle = fract(seed) * PI * 2.0 + t * (0.12 + fract(seed * 2.3) * 0.25);
    float pDist = fract(seed * 3.7) * 0.4 + 0.06;
    vec2 pPos = center + vec2(cos(pAngle), sin(pAngle)) * pDist;
    float twinkle = sin(t * (2.5 + fract(seed * 5.1) * 3.5) + i);
    float pAlpha = max(0.0, twinkle) * 0.6;
    float pd = length(uv - pPos);
    sparkle += smoothstep(0.005, 0.0, pd) * pAlpha;
  }

  float ripples = 0.0;
  if (audio > 0.05) {
    for (float i = 0.0; i < 3.0; i++) {
      float ripple = orbSize * (1.0 + i * 0.35) + audio * 0.25;
      float ring = smoothstep(0.025, 0.0, abs(d - ripple));
      ripples += ring * (1.0 - i * 0.3) * audio;
    }
  }

  vec3 aliveColor = vec3(0.0);
  aliveColor += vec3(1.0, 0.95, 0.85) * core * 2.5;
  aliveColor += vec3(1.0, 0.55, 0.15) * inner * 1.0;
  aliveColor += palette(0.5 + sin(t * 0.12) * 0.25) * outer * 0.6;
  aliveColor += palette(0.35) * glow * 0.3;
  aliveColor += palette(0.6 + t * 0.06) * clouds;
  aliveColor += vec3(1.0, 0.9, 0.75) * sparkle;
  aliveColor += vec3(0.97, 0.5, 0.12) * ripples;

  float ring = smoothstep(0.012, 0.0, abs(d - orbSize * 1.25));
  aliveColor += vec3(0.97, 0.45, 0.09) * ring * 0.35;

  // BLEND
  col = mix(sparkColor, aliveColor, genesis);

  float vignetteStrength = mix(1.8, 1.0, genesis);
  col *= 1.0 - smoothstep(0.25, 0.85, d * vignetteStrength);

  float alpha = 0.0;
  alpha = max(alpha, spark * (1.0 - genesis));
  alpha = max(alpha, sparkGlow * 0.6 * (1.0 - genesis));
  alpha = max(alpha, (core + inner * 0.6 + outer * 0.35 + glow * 0.25) * genesis);
  alpha = max(alpha, clouds * 0.5 * genesis);
  alpha = max(alpha, ripples * genesis);
  alpha = clamp(alpha + length(col) * 0.35, 0.0, 1.0);

  float edgeMask = 1.0 - smoothstep(0.52, 0.78, d);
  alpha *= edgeMask * uOpacity;
  fragColor = vec4(col * alpha, alpha);
}`;

function initGL() {
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
  audioLevel += (targetAudioLevel - audioLevel) * 0.15;
  globalOpacity += (targetOpacity - globalOpacity) * (isFadingOut ? 0.08 : 0.12);

  const birthSpeed = 0.035;
  const deathSpeed = 0.015;
  genesisLevel += (targetGenesisLevel - genesisLevel) * (targetGenesisLevel > genesisLevel ? birthSpeed : deathSpeed);

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
initGL();
resize();
render();

console.log('[Orb-Fire-v2] WebGL2 genesis orb ready');
