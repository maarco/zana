/**
 * @license
 *
 * Originally developed for Unicorn Studio
 * https://unicorn.studio
 *
 * Licensed under the Polyform Non-Resale License 1.0.0
 * https://polyformproject.org/licenses/non-resale/1.0.0/
 *
 * © 2026 UNCRN LLC
 */
/*
 * Aura: the agent audio visualizer from LiveKit's Agents UI (agent-audio-visualizer-aura,
 * https://livekit.com/ui/r/agent-audio-visualizer-aura.json). The fragment shader below is
 * copied unmodified, with the license header above. The WebGL runner and the state
 * animation around it are a plain-JavaScript port of the React component, its
 * useAgentAudioVisualizerAura hook and ReactShaderToy (Apache-2.0, LiveKit, Inc.).
 */
(function () {
  const SHADER = "\nconst float TAU = 6.283185;\n\n// Noise for dithering\nvec2 randFibo(vec2 p) {\n  p = fract(p * vec2(443.897, 441.423));\n  p += dot(p, p.yx + 19.19);\n  return fract((p.xx + p.yx) * p.xy);\n}\n\n// Tonemap\nvec3 Tonemap(vec3 x) {\n  x *= 4.0;\n  return x / (1.0 + x);\n}\n\n// Luma for alpha\nfloat luma(vec3 color) {\n  return dot(color, vec3(0.299, 0.587, 0.114));\n}\n\n// RGB to HSV\nvec3 rgb2hsv(vec3 c) {\n  vec4 K = vec4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);\n  vec4 p = mix(vec4(c.bg, K.wz), vec4(c.gb, K.xy), step(c.b, c.g));\n  vec4 q = mix(vec4(p.xyw, c.r), vec4(c.r, p.yzx), step(p.x, c.r));\n  float d = q.x - min(q.w, q.y);\n  float e = 1.0e-10;\n  return vec3(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);\n}\n\n// HSV to RGB\nvec3 hsv2rgb(vec3 c) {\n  vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);\n  vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);\n  return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);\n}\n\n// SDF shapes\nfloat sdCircle(vec2 st, float r) {\n  return length(st) - r;\n}\n\nfloat sdLine(vec2 p, float r) {\n  float halfLen = r * 2.0;\n  vec2 a = vec2(-halfLen, 0.0);\n  vec2 b = vec2(halfLen, 0.0);\n  vec2 pa = p - a;\n  vec2 ba = b - a;\n  float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);\n  return length(pa - ba * h);\n}\n\nfloat getSdf(vec2 st) {\n  if(uShape == 1.0) return sdCircle(st, uScale);\n  else if(uShape == 2.0) return sdLine(st, uScale);\n  return sdCircle(st, uScale); // Default\n}\n\nvec2 turb(vec2 pos, float t, float it) {\n  // Initial rotation matrix for swirl direction\n  mat2 rotation = mat2(0.6, -0.25, 0.25, 0.9);\n  // Secondary rotation applied each iteration (approx 53 degree rotation)\n  mat2 layerRotation = mat2(0.6, -0.8, 0.8, 0.6);\n\n  float frequency = mix(2.0, 15.0, uFrequency);\n  float amplitude = uAmplitude;\n  float frequencyGrowth = 1.4;\n  float animTime = t * 0.1 * uSpeed;\n\n  const int LAYERS = 4;\n  for(int i = 0; i < LAYERS; i++) {\n    // Calculate wave displacement for this layer\n    vec2 rotatedPos = pos * rotation;\n    vec2 wave = sin(frequency * rotatedPos + float(i) * animTime + it);\n\n    // Apply displacement along rotation direction\n    pos += (amplitude / frequency) * rotation[0] * wave;\n\n    // Evolve parameters for next layer\n    rotation *= layerRotation;\n    amplitude *= mix(1.0, max(wave.x, wave.y), uVariance);\n    frequency *= frequencyGrowth;\n  }\n\n  return pos;\n}\n\nconst float ITERATIONS = 36.0;\n\nvoid mainImage(out vec4 fragColor, in vec2 fragCoord) {\n  vec2 uv = fragCoord / iResolution.xy;\n\n  vec3 pp = vec3(0.0);\n  vec3 bloom = vec3(0.0);\n  float t = iTime * 0.5;\n  vec2 pos = uv - 0.5;\n\n  vec2 prevPos = turb(pos, t, 0.0 - 1.0 / ITERATIONS);\n  float spacing = mix(1.0, TAU, uSpacing);\n\n  for(float i = 1.0; i < ITERATIONS + 1.0; i++) {\n    float iter = i / ITERATIONS;\n    vec2 st = turb(pos, t, iter * spacing);\n    float d = abs(getSdf(st));\n    float pd = distance(st, prevPos);\n    prevPos = st;\n    float dynamicBlur = exp2(pd * 2.0 * 1.4426950408889634) - 1.0;\n    float ds = smoothstep(0.0, uBlur * 0.05 + max(dynamicBlur * uSmoothing, 0.001), d);\n\n    // Shift color based on iteration using uColorScale\n    vec3 color = uColor;\n    if(uColorShift > 0.01) {\n      vec3 hsv = rgb2hsv(color);\n      // Shift hue by iteration\n      hsv.x = fract(hsv.x + (1.0 - iter) * uColorShift * 0.3);\n      color = hsv2rgb(hsv);\n    }\n\n    float invd = 1.0 / max(d + dynamicBlur, 0.001);\n    pp += (ds - 1.0) * color;\n    bloom += clamp(invd, 0.0, 250.0) * color;\n  }\n\n  pp *= 1.0 / ITERATIONS;\n\n  vec3 color;\n\n  // Dark mode (default)\n  if(uMode < 0.5) {\n    // use bloom effect\n    bloom = bloom / (bloom + 2e4);\n    color = (-pp + bloom * 3.0 * uBloom) * 1.2;\n    color += (randFibo(fragCoord).x - 0.5) / 255.0;\n    color = Tonemap(color);\n    float alpha = luma(color) * uMix;\n    fragColor = vec4(color * uMix, alpha);\n  }\n\n  // Light mode\n  else {\n    // no bloom effect\n    color = -pp;\n    color += (randFibo(fragCoord).x - 0.5) / 255.0;\n\n    // Preserve hue by tone mapping brightness only\n    float brightness = length(color);\n    vec3 direction = brightness > 0.0 ? color / brightness : color;\n\n    // Reinhard on brightness\n    float factor = 2.0;\n    float mappedBrightness = (brightness * factor) / (1.0 + brightness * factor);\n    color = direction * mappedBrightness;\n\n    // Boost saturation to compensate for white background bleed-through\n    // When alpha < 1.0, white bleeds through making colors look desaturated\n    // So we increase saturation to maintain vibrant appearance\n    float gray = dot(color, vec3(0.2, 0.5, 0.1));\n    float saturationBoost = 3.0;\n    color = mix(vec3(gray), color, saturationBoost);\n\n    // Clamp between 0-1\n    color = clamp(color, 0.0, 1.0);\n\n    float alpha = mappedBrightness * clamp(uMix, 1.0, 2.0);\n    fragColor = vec4(color, alpha);\n  }\n}";

  const FLOATS = ['uSpeed', 'uBlur', 'uScale', 'uShape', 'uFrequency', 'uAmplitude', 'uBloom', 'uMix',
    'uSpacing', 'uColorShift', 'uVariance', 'uSmoothing', 'uMode'];

  // The hook's targets per agent state (speed, scale, amplitude, frequency, brightness or a
  // [low, high] brightness pulse).
  const STATES = {
    idle: { speed: 10, scale: 0.2, amplitude: 1.2, frequency: 0.4, brightness: 1.0 },
    listening: { speed: 20, scale: 0.3, amplitude: 1.0, frequency: 0.7, brightness: [1.5, 2.0] },
    thinking: { speed: 30, scale: 0.3, amplitude: 0.5, frequency: 1.0, brightness: [0.5, 2.5] },
    speaking: { speed: 70, scale: 0.3, amplitude: 0.75, frequency: 1.25, brightness: 1.5 },
  };
  const ALIASES = { failed: 'idle', disconnected: 'idle', 'pre-connect-buffering': 'listening',
    connecting: 'thinking', initializing: 'thinking' };

  function hexToRgb(hex) {
    const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex || '');
    return m ? [1, 2, 3].map((i) => parseInt(m[i], 16) / 255) : [0.122, 0.835, 0.976];
  }

  function compile(gl, type, source) {
    const shader = gl.createShader(type);
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      throw new Error(gl.getShaderInfoLog(shader) || 'shader compile failed');
    }
    return shader;
  }

  /**
   * Draw the Aura on `canvas`. Call setState(state) with the agent state and
   * setVolume(0..1) with the agent's voice level while it speaks.
   */
  window.createAura = function createAura(canvas, { color = '#1FD5F9', colorShift = 0.05, themeMode = 'dark' } = {}) {
    const gl = canvas.getContext('webgl', { premultipliedAlpha: true, alpha: true, antialias: false });
    if (!gl) throw new Error('WebGL is not available');
    const fragment = 'precision highp float;\nuniform vec3 iResolution;\nuniform float iTime;\n'
      + FLOATS.map((u) => `uniform float ${u};`).join('\n') + '\nuniform vec3 uColor;\n'
      + SHADER + '\nvoid main() { mainImage(gl_FragColor, gl_FragCoord.xy); }';
    const program = gl.createProgram();
    gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, 'attribute vec2 p; void main() { gl_Position = vec4(p, 0.0, 1.0); }'));
    gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, fragment));
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program) || 'link failed');
    gl.useProgram(program);
    gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]), gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(program, 'p');
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    // Like ReactShaderToy: no blending; the shader's color and alpha go straight to the canvas.
    const u = (name) => gl.getUniformLocation(program, name);

    // Fixed uniforms, as the React component sets them.
    const fixed = { uBlur: 1.0, uShape: 1.0, uBloom: 0.0, uSpacing: 0.5, uColorShift: colorShift,
      uVariance: 0.1, uSmoothing: 1.0, uMode: themeMode === 'light' ? 1.0 : 0.0, uSpeed: 1.0 };
    for (const [name, value] of Object.entries(fixed)) gl.uniform1f(u(name), value);
    gl.uniform3fv(u('uColor'), hexToRgb(color));

    // Animated values: each eases toward its target (0.5 s ease-out); brightness can pulse.
    const value = { speed: 10, scale: 0.2, amplitude: 2, frequency: 0.5, brightness: 1.5 };
    let target = { ...STATES.idle };
    let state = 'idle';
    let volume = 0;
    let phase = 0;
    let last = performance.now();
    let pulseStart = last;
    let running = true;

    function resize() {
      const dpr = window.devicePixelRatio || 1;
      const w = Math.round(canvas.clientWidth * dpr);
      const h = Math.round(canvas.clientHeight * dpr);
      if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.uniform3f(u('iResolution'), canvas.width, canvas.height, 1);
    }

    function frame(now) {
      if (!running) return;
      const dt = Math.min(0.1, (now - last) / 1000);
      last = now;
      const ease = 1 - Math.exp(-dt / 0.16); // about 0.5 s to settle, ease-out
      for (const key of ['scale', 'amplitude', 'frequency']) value[key] += (target[key] - value[key]) * ease;
      value.speed = target.speed; // the hook switches speed at once
      if (Array.isArray(target.brightness)) {
        const [lo, hi] = target.brightness;
        const k = ((now - pulseStart) / 350) % 2; // 0.35 s each way, mirrored
        const s = k < 1 ? k : 2 - k;
        value.brightness = lo + (hi - lo) * (1 - (1 - s) * (1 - s));
      } else {
        value.brightness += (target.brightness - value.brightness) * ease;
      }
      if (state === 'speaking' && volume > 0) value.scale = 0.2 + 0.2 * volume;
      // Integrate speed into time, so a speed change never jumps the waves.
      phase += dt * value.speed;
      resize();
      gl.uniform1f(u('iTime'), phase);
      gl.uniform1f(u('uScale'), value.scale);
      gl.uniform1f(u('uAmplitude'), value.amplitude);
      gl.uniform1f(u('uFrequency'), value.frequency);
      gl.uniform1f(u('uMix'), value.brightness);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.drawArrays(gl.TRIANGLES, 0, 6);
      requestAnimationFrame(frame);
    }
    requestAnimationFrame(frame);

    return {
      setState(next) {
        const key = STATES[next] ? next : ALIASES[next] || 'idle';
        if (key === state) return;
        state = key;
        target = { ...STATES[key] };
        pulseStart = performance.now();
      },
      setVolume(v) { volume = Math.max(0, Math.min(1, v || 0)); },
      setColor(hex) { gl.useProgram(program); gl.uniform3fv(u('uColor'), hexToRgb(hex)); },
      destroy() { running = false; },
    };
  };
}());
