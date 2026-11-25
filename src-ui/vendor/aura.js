/**
 * Zana original WebGL orb renderer.
 *
 * The fragment shader and the small WebGL runner are original Zana code. The
 * public createAura API is retained because the Voicebot page already owns
 * the state and volume wiring.
 */
(function () {
  const SHADER = [
    'float hash2(vec2 p) {',
    '  p = fract(p * vec2(127.1, 311.7));',
    '  return fract(sin(dot(p, vec2(91.7, 47.3))) * 43758.5453);',
    '}',
    '',
    'void mainImage(out vec4 fragColor, in vec2 fragCoord) {',
    '  vec2 aspect = vec2(iResolution.x / iResolution.y, 1.0);',
    '  vec2 p = (fragCoord / iResolution.xy - 0.5) * aspect;',
    '  float radius = length(p);',
    '  float angle = atan(p.y, p.x);',
    '  float time = iTime * 0.08;',
    '  float scale = clamp(uScale, 0.0, 1.0);',
    '  float frequency = 3.0 + uFrequency * 7.0;',
    '  float wobble = sin(angle * frequency + time * (1.0 + uSpeed * 0.04))',
    '    * (0.006 + uAmplitude * 0.018);',
    '  float ringRadius = mix(0.16, 0.33, scale);',
    '  float ring = 1.0 - smoothstep(0.0, 0.035 + uBlur * 0.012,',
    '    abs(radius - ringRadius - wobble));',
    '  float halo = exp(-radius * radius / (0.095 + uBlur * 0.07));',
    '  float filament = 0.5 + 0.5 * sin(angle * (frequency + 2.0)',
    '    - time * (2.0 + uSpeed * 0.06) + radius * 21.0);',
    '  float drift = sin((p.x - p.y) * 16.0 + time * 3.0)',
    '    * sin((p.x + p.y) * 11.0 - time * 2.0);',
    '  float grain = hash2(fragCoord + vec2(iTime, -iTime)) - 0.5;',
    '  float edge = 1.0 - smoothstep(0.28, 0.62, radius);',
    '  float shape = uShape > 1.5',
    '    ? 1.0 - smoothstep(0.02, 0.18, abs(p.y) + abs(p.x) * 0.35)',
    '    : edge;',
    '  float energy = halo * (0.42 + 0.22 * filament)',
    '    + ring * (0.72 + 0.28 * filament)',
    '    + max(drift, 0.0) * 0.09 * (0.5 + uVariance)',
    '    + grain * 0.018 * uSmoothing;',
    '  energy *= shape * (0.88 + 0.12 * sin(time * 2.0 + uSpacing * 6.0));',
    '  vec3 tint = clamp(uColor + vec3(',
    '    sin(angle + time) * uColorShift * 0.08,',
    '    sin(angle + time + 2.1) * uColorShift * 0.08,',
    '    sin(angle + time + 4.2) * uColorShift * 0.08), 0.0, 1.0);',
    '  float brightness = max(energy * uMix, 0.0);',
    '  if (uMode < 0.5) {',
    '    brightness *= 1.0 + uBloom * 0.2;',
    '  } else {',
    '    brightness *= 0.8;',
    '  }',
    '  vec3 color = clamp(tint * brightness * 1.35, 0.0, 1.0);',
    '  float alpha = clamp(brightness * (0.48 + uBloom * 0.08), 0.0, 1.0);',
    '  fragColor = vec4(color, alpha);',
    '}',
  ].join('\n');

  const FLOAT_UNIFORMS = [
    'uSpeed', 'uBlur', 'uScale', 'uShape', 'uFrequency', 'uAmplitude',
    'uBloom', 'uMix', 'uSpacing', 'uColorShift', 'uVariance', 'uSmoothing',
    'uMode',
  ];

  const STATES = {
    idle: { speed: 8, scale: 0.2, amplitude: 1.2, frequency: 0.4, brightness: 0.6 },
    listening: { speed: 20, scale: 0.3, amplitude: 1.0, frequency: 0.7, brightness: [1.5, 2.0] },
    thinking: { speed: 30, scale: 0.3, amplitude: 0.5, frequency: 1.0, brightness: [1.4, 2.5] },
    speaking: { speed: 70, scale: 0.3, amplitude: 0.75, frequency: 1.25, brightness: 1.5 },
    working: { speed: 45, scale: 0.3, amplitude: 0.9, frequency: 0.6, brightness: [1.2, 1.8] },
  };
  const ALIASES = {
    failed: 'idle',
    disconnected: 'idle',
    'pre-connect-buffering': 'listening',
    connecting: 'thinking',
    initializing: 'thinking',
  };

  function hexToRgb(hex) {
    const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex || '');
    return match
      ? [1, 2, 3].map((index) => parseInt(match[index], 16) / 255)
      : [0.122, 0.835, 0.976];
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

  window.createAura = function createAura(
    canvas,
    { color = '#1FD5F9', colorShift = 0.05, themeMode = 'dark' } = {},
  ) {
    const gl = canvas.getContext('webgl', {
      premultipliedAlpha: true,
      alpha: true,
      antialias: false,
    });
    if (!gl) throw new Error('WebGL is not available');

    const fragment = [
      'precision highp float;',
      'uniform vec3 iResolution;',
      'uniform float iTime;',
      ...FLOAT_UNIFORMS.map((name) => 'uniform float ' + name + ';'),
      'uniform vec3 uColor;',
      SHADER,
      'void main() { mainImage(gl_FragColor, gl_FragCoord.xy); }',
    ].join('\n');
    const program = gl.createProgram();
    gl.attachShader(program, compile(
      gl,
      gl.VERTEX_SHADER,
      'attribute vec2 position; void main() { gl_Position = vec4(position, 0.0, 1.0); }',
    ));
    gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, fragment));
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      throw new Error(gl.getProgramInfoLog(program) || 'program link failed');
    }
    gl.useProgram(program);
    gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
      gl.STATIC_DRAW,
    );
    const position = gl.getAttribLocation(program, 'position');
    gl.enableVertexAttribArray(position);
    gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);

    const uniform = (name) => gl.getUniformLocation(program, name);
    const fixed = {
      uBlur: 1.0,
      uShape: 1.0,
      uBloom: 0.0,
      uSpacing: 0.5,
      uColorShift: colorShift,
      uVariance: 0.1,
      uSmoothing: 1.0,
      uMode: themeMode === 'light' ? 1.0 : 0.0,
      uSpeed: 1.0,
    };
    for (const [name, value] of Object.entries(fixed)) {
      gl.uniform1f(uniform(name), value);
    }

    const colorNow = hexToRgb(color);
    let colorTarget = colorNow.slice();
    gl.uniform3fv(uniform('uColor'), colorNow);

    const value = {
      speed: 10,
      scale: 0.2,
      amplitude: 2,
      frequency: 0.5,
      brightness: 1.5,
    };
    let target = { ...STATES.idle };
    let state = 'idle';
    let volume = 0;
    let phase = 0;
    let previous = performance.now();
    let pulseStart = previous;
    let running = true;

    function resize() {
      const dpr = window.devicePixelRatio || 1;
      const width = Math.round(canvas.clientWidth * dpr);
      const height = Math.round(canvas.clientHeight * dpr);
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
      }
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.uniform3f(uniform('iResolution'), canvas.width, canvas.height, 1);
    }

    function frame(now) {
      if (!running) return;
      const delta = Math.min(0.1, Math.max(0, (now - previous) / 1000));
      previous = now;
      const ease = 1 - Math.exp(-delta / 0.16);
      for (const key of ['scale', 'amplitude', 'frequency']) {
        value[key] += (target[key] - value[key]) * ease;
      }
      value.speed = target.speed;
      if (Array.isArray(target.brightness)) {
        const [low, high] = target.brightness;
        const cycle = ((now - pulseStart) / 350) % 2;
        const slope = cycle < 1 ? cycle : 2 - cycle;
        value.brightness = low + (high - low) * (1 - (1 - slope) * (1 - slope));
      } else {
        value.brightness += (target.brightness - value.brightness) * ease;
      }
      if (state === 'speaking' && volume > 0) {
        value.scale = 0.2 + 0.2 * volume;
      }
      for (let index = 0; index < 3; index += 1) {
        colorNow[index] += (colorTarget[index] - colorNow[index]) * ease;
      }
      gl.uniform3fv(uniform('uColor'), colorNow);
      phase += delta * value.speed;
      resize();
      gl.uniform1f(uniform('iTime'), phase);
      gl.uniform1f(uniform('uScale'), value.scale);
      gl.uniform1f(uniform('uAmplitude'), value.amplitude);
      gl.uniform1f(uniform('uFrequency'), value.frequency);
      gl.uniform1f(uniform('uMix'), value.brightness);
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
      setVolume(next) {
        volume = Math.max(0, Math.min(1, next || 0));
      },
      setColor(hex) {
        colorTarget = hexToRgb(hex);
      },
      destroy() {
        running = false;
      },
    };
  };
}());
