// Orb designs for Voicebot's orb window, next to the original Aura (vendor/aura.js):
// Strands, Halo (canvas 2D) and Fur, Nebula, Liquid (Three.js). Every design takes the same
// states (idle, listening, thinking, talking, working, voiceprint, nomic, asleep) and a voice
// level, and moves on springs. Loaded as a module; voicebot.html uses window.ZanaOrbDesigns.
import * as THREE from './vendor/three.module.js';

// Carter's color personality: Apple system colors (dark appearance), no orange.
const PALETTE = {
  idle:       ['#8E8E93', '#A1A1B5', '#7D7AFF'],
  listening:  ['#30D158', '#63E6E2', '#64D2FF'],
  thinking:   ['#BF5AF2', '#7D7AFF', '#64D2FF'],
  talking:    ['#0A84FF', '#64D2FF', '#BF5AF2', '#FF375F'],
  working:    ['#40C8E0', '#5E5CE6', '#30D158'],
  nomic:      ['#FF453A', '#FF375F'],
  asleep:     ['#48484A', '#636366'],
  voiceprint: ['#FF375F', '#FF453A', '#BF5AF2'],
};
const hex = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
const rgba = (c, a) => `rgba(${c[0] | 0},${c[1] | 0},${c[2] | 0},${a})`;
const clamp = (v, a = 0, b = 1) => Math.max(a, Math.min(b, v));
const VOICED = ['talking', 'listening', 'voiceprint'];
const ENERGY = { idle: 0.2, listening: 0.45, thinking: 0.4, talking: 0.55, working: 0.95, nomic: 0.05, asleep: 0.08, voiceprint: 0.55 };

// Physicality: every move is a spring (a little overshoot, then it settles), never a jump.
class Spring {
  constructor(v = 0, k = 170, d = 20) { this.v = v; this.t = v; this.vel = 0; this.k = k; this.d = d; }
  to(t) { this.t = t; return this; }
  step(dt) { this.vel += (this.k * (this.t - this.v) - this.d * this.vel) * dt; this.v += this.vel * dt; return this.v; }
}
function easeColors(cols, state, dt) {
  const pal = PALETTE[state].map(hex);
  cols.forEach((c, i) => { const g = pal[i % pal.length]; for (let k = 0; k < 3; k++) c[k] += (g[k] - c[k]) * Math.min(1, dt * 3.5); });
}
const snake = (t, i) => Math.sin(t * 4.2 - i * 0.95); // 1 up, 2 up, 3 up, all down
// The shared motion targets every design follows.
function motion() {
  return { split: new Spring(0, 110, 17), flat: new Spring(0, 140, 20), smile: new Spring(0, 80, 13),
    size: new Spring(1, 120, 18), voice: new Spring(0, 140, 16), energy: new Spring(0.2, 60, 14), boop: new Spring(0, 260, 11) };
}
function drive(m, state, level, dt) {
  m.split.to(state === 'thinking' ? 1 : 0); m.flat.to(state === 'nomic' ? 1 : 0);
  m.smile.to(state === 'talking' ? 1 : 0); m.size.to(state === 'asleep' ? 0.45 : 1);
  m.voice.to(VOICED.includes(state) ? level : 0); m.energy.to(ENERGY[state]);
  for (const s of Object.values(m)) s.step(dt);
}


const C = 360; // center of the 720 px drawing square
const ctxHolder = { ctx: null, canvas: null };
let renderer = null;
// The 2D designs draw on a 720 px square; scale that to the orb's canvas.
function fit2d() {
  const k = ctxHolder.canvas.width / 720;
  ctxHolder.ctx.setTransform(k, 0, 0, k, 0, 0);
}
// ---------- Strands: colored strands with depth; three orbs think; voice ripples travel; it smiles ----------
function makeStrands() {
  const R = 150; const m = motion();
  const cols = [0, 1, 2, 3].map(() => hex('#8E8E93'));
  let nextBoop = 0;
  function ring(cx, cy, r, t, amp, n, alpha, flat, smile, voice, tiltX, spin, width) {
    for (let i = 0; i < n; i++) {
      const ph = i * 1.7; const sp = 0.6 + i * 0.23; const k = 2 + (i % 4);
      const back = new Path2D(); const front = new Path2D();
      let prevZ = null;
      for (let a = 0; a <= Math.PI * 2 + 0.02; a += Math.PI / 96) {
        let wob = Math.sin(k * a + t * sp * 2 + ph) * 0.55 + Math.sin((k + 3) * a - t * sp * 1.3 + ph * 2) * 0.45;
        wob += voice * 0.9 * Math.sin(7 * a - t * 10 + i); // a ripple that travels around while it talks
        const rr = r * (1 + amp * wob);
        let x = Math.cos(a) * rr * (1 + 0.05 * smile);
        let y = Math.sin(a) * rr * (1 - 0.04 * smile);
        if (y > 0) y -= smile * r * 0.12 * Math.cos(a) ** 2; // the bottom corners lift: a slight smile
        // depth: each strand sits on its own tilted plane, slowly turning
        const tx = tiltX * (0.6 + 0.4 * Math.sin(i * 1.3)); const sy = spin + i * 0.4;
        let z = Math.sin(sy) * x; x = Math.cos(sy) * x;
        const y2 = y * Math.cos(tx) - z * Math.sin(tx); z = y * Math.sin(tx) + z * Math.cos(tx); y = y2;
        y *= 1 - flat * 0.97; // can't hear: flattens into a line
        const p = 1 + z / 900; const px = cx + x * p; const py = cy + y * p;
        const path = z < 0 ? back : front;
        if (prevZ === null || (prevZ < 0) !== (z < 0)) path.moveTo(px, py); else path.lineTo(px, py);
        prevZ = z;
      }
      const c = cols[i % cols.length]; const w = width * (0.7 + 0.5 * ((i * 37) % 10) / 10);
      ctxHolder.ctx.lineWidth = w; ctxHolder.ctx.shadowColor = rgba(c, 0.9); ctxHolder.ctx.shadowBlur = 20;
      ctxHolder.ctx.strokeStyle = rgba(c, alpha * 0.25); ctxHolder.ctx.stroke(back);
      ctxHolder.ctx.strokeStyle = rgba(c, alpha * 0.65); ctxHolder.ctx.stroke(front);
    }
    if (smile > 0.05) {
      ctxHolder.ctx.beginPath(); ctxHolder.ctx.arc(cx, cy - smile * r * 0.04, r * 1.02, Math.PI * 0.2, Math.PI * 0.8);
      ctxHolder.ctx.strokeStyle = rgba(cols[0], 0.35 * smile * alpha); ctxHolder.ctx.lineWidth = width * 1.6; ctxHolder.ctx.stroke();
    }
  }
  return {
    kind: '2d',
    draw(dt, t, state, level) {
      if (state === 'thinking' && t > nextBoop) { m.boop.vel = 7; nextBoop = t + 2.6; }
      drive(m, state, level, dt); easeColors(cols, state, dt);
      fit2d(); ctxHolder.ctx.clearRect(0, 0, 720, 720); ctxHolder.ctx.globalCompositeOperation = 'lighter';
      const breathe = state === 'asleep' || state === 'nomic' ? 0.7 + 0.3 * Math.sin(t * 2.2) : 1;
      const amp = 0.025 + m.energy.v * 0.06 + m.voice.v * 0.12;
      const spin = t * (0.25 + m.energy.v * 0.6); const tiltX = 0.35 * Math.sin(t * 0.4) * (1 - m.flat.v);
      const split = clamp(m.split.v, 0, 1.2);
      if (split < 0.97) ring(C, C, R * m.size.v * (1 - 0.6 * split), t, amp, state === 'asleep' ? 3 : 7,
        (1 - split) * breathe, m.flat.v, m.smile.v, m.voice.v, tiltX, spin, 3.0);
      if (split > 0.03) {
        const rs = R * 0.26; const gap = R * 0.78;
        for (let i = 0; i < 3; i++) ring(C + (i - 1) * gap * split, C - snake(t, i) * 26 * split, rs, t + i, 0.06, 3, split, 0, 0, 0, tiltX, spin + i, 2.4);
        const b = clamp(m.boop.v * 0.25, 0, 1);
        if (b > 0.01) {
          ctxHolder.ctx.beginPath(); ctxHolder.ctx.arc(C, C + R * 0.62, 7 * b + 2, 0, Math.PI * 2);
          ctxHolder.ctx.fillStyle = rgba(cols[1], 0.7 * b); ctxHolder.ctx.shadowColor = rgba(cols[1], 1); ctxHolder.ctx.shadowBlur = 16; ctxHolder.ctx.fill();
        }
      }
      ctxHolder.ctx.globalCompositeOperation = 'source-over'; ctxHolder.ctx.shadowBlur = 0;
    },
  };
}

// ---------- Halo: a soft glow with a glassy rim; its colors swirl faster with the voice ----------
function makeHalo() {
  const R = 120; const m = motion(); let phase = 0;
  const cols = [0, 1, 2, 3].map(() => hex('#8E8E93'));
  const sx = new Spring(1, 160, 14); const sy = new Spring(1, 160, 14);
  let nextBoop = 0;
  function blob(cx, cy, r, alpha, n, rim, seed) {
    const g = ctxHolder.ctx.createRadialGradient(cx, cy, 0, cx, cy, r);
    g.addColorStop(0, rgba(cols[0], 0.55 * alpha)); g.addColorStop(1, rgba(cols[0], 0));
    ctxHolder.ctx.fillStyle = g; ctxHolder.ctx.beginPath(); ctxHolder.ctx.arc(cx, cy, r, 0, Math.PI * 2); ctxHolder.ctx.fill();
    for (let i = 1; i < n; i++) {
      const a = phase * (0.5 + i * 0.27) + i * 2.1 + seed; const d = r * 0.32;
      const x = cx + Math.cos(a) * d; const y = cy + Math.sin(a * 1.3) * d;
      const lg = ctxHolder.ctx.createRadialGradient(x, y, 0, x, y, r * 0.75);
      lg.addColorStop(0, rgba(cols[i % cols.length], 0.5 * alpha)); lg.addColorStop(1, rgba(cols[i % cols.length], 0));
      ctxHolder.ctx.fillStyle = lg; ctxHolder.ctx.beginPath(); ctxHolder.ctx.arc(x, y, r * 0.75, 0, Math.PI * 2); ctxHolder.ctx.fill();
    }
    if (rim > 0) { // a glassy rim, brighter on the upper left like light catching it
      const rg = ctxHolder.ctx.createLinearGradient(cx - r, cy - r, cx + r, cy + r);
      rg.addColorStop(0, rgba([255, 255, 255], 0.55 * rim * alpha)); rg.addColorStop(0.5, rgba(cols[1], 0.18 * rim * alpha));
      rg.addColorStop(1, rgba(cols[2 % cols.length], 0.35 * rim * alpha));
      ctxHolder.ctx.beginPath(); ctxHolder.ctx.arc(cx, cy, r * 0.66, 0, Math.PI * 2); ctxHolder.ctx.strokeStyle = rg; ctxHolder.ctx.lineWidth = 2.2; ctxHolder.ctx.stroke();
    }
  }
  return {
    kind: '2d',
    draw(dt, t, state, level) {
      if (state === 'thinking' && t > nextBoop) { m.boop.vel = 7; nextBoop = t + 2.6; }
      drive(m, state, level, dt); easeColors(cols, state, dt);
      phase += dt * (1 + m.energy.v * 0.8 + m.voice.v * 2.2);
      sx.to(1 + m.voice.v * 0.14).step(dt); sy.to(1 - m.voice.v * 0.09).step(dt);
      fit2d(); ctxHolder.ctx.clearRect(0, 0, 720, 720); ctxHolder.ctx.globalCompositeOperation = 'lighter';
      const breathe = state === 'asleep' || state === 'nomic' ? 0.65 + 0.35 * Math.sin(t * 2.2) : 1;
      const split = clamp(m.split.v, 0, 1.2);
      ctxHolder.ctx.save(); ctxHolder.ctx.translate(C, C); ctxHolder.ctx.scale(sx.v, sy.v * (1 - m.flat.v * 0.94)); ctxHolder.ctx.translate(-C, -C);
      if (split < 0.97) blob(C, C, R * m.size.v * (1 - 0.6 * split) * (1 + 0.04 * Math.sin(t * 1.1)), (1 - split) * breathe, 4, 0.5 + m.voice.v, 0);
      if (m.smile.v > 0.05) {
        ctxHolder.ctx.beginPath(); ctxHolder.ctx.arc(C, C - R * 0.05, R * 0.62, Math.PI * 0.22, Math.PI * 0.78);
        ctxHolder.ctx.strokeStyle = rgba(cols[1], 0.5 * m.smile.v); ctxHolder.ctx.lineWidth = 7; ctxHolder.ctx.lineCap = 'round';
        ctxHolder.ctx.shadowColor = rgba(cols[1], 1); ctxHolder.ctx.shadowBlur = 18; ctxHolder.ctx.stroke(); ctxHolder.ctx.shadowBlur = 0;
      }
      ctxHolder.ctx.restore();
      if (split > 0.03) {
        for (let i = 0; i < 3; i++) blob(C + (i - 1) * R * 0.95 * split, C - snake(t, i) * 24 * split, R * 0.42, split, 2, 0.6, i * 3);
        const b = clamp(m.boop.v * 0.25, 0, 1);
        if (b > 0.01) { ctxHolder.ctx.beginPath(); ctxHolder.ctx.arc(C, C + R * 0.8, 7 * b + 2, 0, Math.PI * 2); ctxHolder.ctx.fillStyle = rgba(cols[1], 0.75 * b); ctxHolder.ctx.fill(); }
      }
      ctxHolder.ctx.globalCompositeOperation = 'source-over';
    },
  };
}

// ---------- Three.js designs ----------
const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 50); camera.position.set(0, 0, 6.2);

const NOISE = `
float hash(vec3 p) { p = fract(p * 0.3183099 + 0.1); p *= 17.0; return fract(p.x * p.y * p.z * (p.x + p.y + p.z)); }
float noise(vec3 x) {
  vec3 i = floor(x); vec3 f = fract(x); f = f * f * (3.0 - 2.0 * f);
  return mix(mix(mix(hash(i), hash(i + vec3(1.0, 0.0, 0.0)), f.x), mix(hash(i + vec3(0.0, 1.0, 0.0)), hash(i + vec3(1.0, 1.0, 0.0)), f.x), f.y),
             mix(mix(hash(i + vec3(0.0, 0.0, 1.0)), hash(i + vec3(1.0, 0.0, 1.0)), f.x), mix(hash(i + vec3(0.0, 1.0, 1.0)), hash(i + vec3(1.0, 1.0, 1.0)), f.x), f.y), f.z);
}
// A U-shaped band on the lower front of the unit sphere: where the smile lights up.
float smileBand(vec3 n) {
  return exp(-pow((n.y + 0.42 - 0.32 * n.x * n.x) / 0.07, 2.0)) * smoothstep(0.2, 0.6, n.z) * smoothstep(0.75, 0.4, abs(n.x));
}`;
function sharedUniforms() {
  return { uTime: { value: 0 }, uVoice: { value: 0 }, uSplit: { value: 0 }, uFlat: { value: 0 }, uSmile: { value: 0 },
    uSize: { value: 1 }, uEnergy: { value: 0.2 }, uAlpha: { value: 1 },
    uC0: { value: new THREE.Vector3(0.5, 0.5, 0.5) }, uC1: { value: new THREE.Vector3(0.5, 0.5, 0.5) }, uC2: { value: new THREE.Vector3(0.5, 0.5, 0.5) } };
}
function feed(u, m, cols, t, state) {
  u.uTime.value = t; u.uVoice.value = m.voice.v; u.uSplit.value = clamp(m.split.v, 0, 1.15); u.uFlat.value = m.flat.v;
  u.uSmile.value = m.smile.v; u.uSize.value = m.size.v; u.uEnergy.value = m.energy.v;
  u.uAlpha.value = state === 'asleep' || state === 'nomic' ? 0.65 + 0.35 * Math.sin(t * 2.2) : 1;
  [u.uC0, u.uC1, u.uC2].forEach((c, i) => c.value.set(cols[i][0] / 255, cols[i][1] / 255, cols[i][2] / 255));
}
// A soft inner light behind the 3D designs.
function glowSprite() {
  const cv = document.createElement('canvas'); cv.width = cv.height = 128;
  const g = cv.getContext('2d'); const rg = g.createRadialGradient(64, 64, 0, 64, 64, 64);
  rg.addColorStop(0, 'rgba(255,255,255,0.9)'); rg.addColorStop(0.35, 'rgba(255,255,255,0.25)'); rg.addColorStop(1, 'rgba(255,255,255,0)');
  g.fillStyle = rg; g.fillRect(0, 0, 128, 128);
  const sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: new THREE.CanvasTexture(cv), blending: THREE.AdditiveBlending, depthWrite: false, transparent: true }));
  return sprite;
}

// Fur: thousands of glowing hairs grown from a ball, swaying in gusts that roll across the coat
// (after the fur coat in kollab's gem renderer): darker at the root, lighter at the tip.
function makeFur() {
  const scene = new THREE.Scene(); const m = motion(); const u = sharedUniforms();
  const cols = [0, 1, 2, 3].map(() => hex('#8E8E93')); let nextBoop = 0;
  const BLADES = 6000; const SEG = 6; const n = BLADES * SEG * 2;
  const pos = new Float32Array(n * 3); const sArr = new Float32Array(n); const rnd = new Float32Array(n * 4);
  let v = 0;
  for (let b = 0; b < BLADES; b++) {
    const z = Math.random() * 2 - 1; const th = Math.random() * Math.PI * 2; const r = Math.sqrt(1 - z * z);
    const root = [r * Math.cos(th), z, r * Math.sin(th)]; const rr = [Math.random(), Math.random(), Math.random(), Math.random()];
    for (let j = 0; j < SEG; j++) for (const s of [j / SEG, (j + 1) / SEG]) { pos.set(root, v * 3); sArr[v] = s; rnd.set(rr, v * 4); v++; }
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.BufferAttribute(pos, 3));
  geo.setAttribute('aS', new THREE.BufferAttribute(sArr, 1));
  geo.setAttribute('aRand', new THREE.BufferAttribute(rnd, 4));
  const mat = new THREE.ShaderMaterial({
    uniforms: u, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending,
    vertexShader: `uniform float uTime, uVoice, uSplit, uFlat, uSmile, uSize, uEnergy, uAlpha;
      uniform vec3 uC0, uC1, uC2;
      attribute float aS; attribute vec4 aRand;
      varying vec3 vCol; varying float vA;
      ${NOISE}
      void main() {
        vec3 n = normalize(position);
        float k = floor(aRand.x * 3.0);
        vec3 center = vec3((k - 1.0) * 0.88, sin(uTime * 4.2 - k * 0.95) * 0.2, 0.0) * uSplit;
        float r = mix(0.6, 0.24, clamp(uSplit, 0.0, 1.0)) * uSize;
        vec3 rootP = center + n * r;
        float len = (0.4 + 0.32 * aRand.y) * mix(1.0, 0.45, clamp(uSplit, 0.0, 1.0)) * uSize * (1.0 + uVoice * 0.9);
        vec3 side = normalize(cross(n, vec3(0.31, 1.0, 0.17)));
        vec3 up = normalize(cross(side, n));
        float g1 = noise(n * 2.3 + vec3(uTime * 0.55, uTime * 0.31, 0.0)) - 0.5;
        float g2 = noise(n * 4.1 - vec3(0.0, uTime * 0.8, uTime * 0.4)) - 0.5;
        float sway = 0.35 + uEnergy * 0.9 + uVoice * 1.6;
        float s = aS;
        vec3 p = rootP + n * len * s + (side * g1 + up * g2) * sway * len * s * s;
        // keep every hair inside the camera's view (half-height 1.66 at this distance): ease the radius
        // toward RMAX instead of letting voice and sway push tips past the canvas edge, where they were
        // cut flat and the coat read as a square. 1.4 stays inside even for tips leaning toward the camera.
        const float RMAX = 1.4;
        float L = length(p);
        float Lc = RMAX * tanh(L / RMAX);
        p *= Lc / max(L, 1e-4);
        p.y *= 1.0 - uFlat * 0.95;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(p, 1.0);
        vec3 base = mix(uC0, mix(uC1, uC2, step(0.5, aRand.z)), smoothstep(0.2, 0.8, aRand.w));
        vCol = base * (0.25 + 1.1 * s) * (1.0 + smileBand(n) * uSmile * 2.0);
        vA = uAlpha * (0.08 + 0.92 * s) * (0.35 + 0.65 * aRand.y) * (1.0 - uFlat * 0.85);
        vA *= 1.0 - smoothstep(RMAX * 0.82, RMAX, Lc);   // tips fade out before the limit: a soft round edge
      }`,
    fragmentShader: `varying vec3 vCol; varying float vA; void main() { gl_FragColor = vec4(vCol, vA); }`,
  });
  scene.add(new THREE.LineSegments(geo, mat));
  const glow = glowSprite(); scene.add(glow);
  return {
    kind: 'three',
    dispose() { freeScene(scene); },
    draw(dt, t, state, level) {
      if (state === 'thinking' && t > nextBoop) { m.boop.vel = 7; nextBoop = t + 2.6; }
      drive(m, state, level, dt); easeColors(cols, state, dt); feed(u, m, cols, t, state);
      const split = clamp(m.split.v, 0, 1);
      glow.material.color.setRGB(cols[0][0] / 255, cols[0][1] / 255, cols[0][2] / 255);
      glow.scale.setScalar(1.7 * m.size.v * (1 - 0.55 * split) * (1 + m.voice.v * 0.25));
      glow.scale.y *= 1 - m.flat.v * 0.9; glow.material.opacity = 0.45 * u.uAlpha.value;
      renderer.render(scene, camera);
    },
  };
}

// Nebula: a shell of glowing particles drifting in a slow current.
function makeNebula() {
  const scene = new THREE.Scene(); const m = motion(); const u = sharedUniforms(); u.uPixel = { value: 2 };
  const cols = [0, 1, 2, 3].map(() => hex('#8E8E93')); let nextBoop = 0;
  const N = 14000; const pos = new Float32Array(N * 3); const rnd = new Float32Array(N * 4);
  for (let i = 0; i < N; i++) {
    const z = Math.random() * 2 - 1; const th = Math.random() * Math.PI * 2; const r = Math.sqrt(1 - z * z);
    pos.set([r * Math.cos(th), z, r * Math.sin(th)], i * 3); rnd.set([Math.random(), Math.random(), Math.random(), Math.random()], i * 4);
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.BufferAttribute(pos, 3)); geo.setAttribute('aRand', new THREE.BufferAttribute(rnd, 4));
  const mat = new THREE.ShaderMaterial({
    uniforms: u, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending,
    vertexShader: `uniform float uTime, uVoice, uSplit, uFlat, uSmile, uSize, uEnergy, uAlpha, uPixel;
      uniform vec3 uC0, uC1, uC2;
      attribute vec4 aRand;
      varying vec3 vCol; varying float vA;
      ${NOISE}
      void main() {
        vec3 n = normalize(position);
        float k = floor(aRand.x * 3.0); float sp = clamp(uSplit, 0.0, 1.0);
        vec3 center = vec3((k - 1.0) * 1.15, sin(uTime * 4.2 - k * 0.95) * 0.22, 0.0) * uSplit;
        float r = mix(1.0, 0.36, sp) * uSize * (0.8 + 0.35 * aRand.y * aRand.y);
        vec3 drift = vec3(noise(n * 1.7 + uTime * 0.25), noise(n * 1.7 + 13.1 + uTime * 0.21), noise(n * 1.7 + 7.7 - uTime * 0.23)) - 0.5;
        float pulse = 1.0 + uVoice * 0.45 * noise(n * 3.0 + uTime * 2.2) + 0.03 * sin(uTime * 1.3);
        vec3 p = center + n * r * pulse + drift * (0.18 + uEnergy * 0.4) * mix(1.0, 0.4, sp);
        p.y *= 1.0 - uFlat * 0.96;
        vec4 mv = modelViewMatrix * vec4(p, 1.0);
        gl_Position = projectionMatrix * mv;
        gl_PointSize = (2.2 + 3.8 * aRand.z) * uPixel * (1.0 + uVoice * 0.6) * 3.0 / -mv.z;
        vec3 nv = normalize(mat3(modelMatrix) * n);
        vec3 base = mix(mix(uC0, uC1, smoothstep(-1.0, 1.0, nv.x)), uC2, smoothstep(0.3, 1.0, nv.y));
        float smile = smileBand(nv) * uSmile;
        vCol = base * (1.3 + smile * 2.4) + vec3(smile * 0.5);
        vA = uAlpha * (0.55 + 0.45 * aRand.w) * (0.55 + 0.45 * smoothstep(-0.5, 0.8, nv.z)) * (1.0 - uFlat * 0.7);
      }`,
    fragmentShader: `varying vec3 vCol; varying float vA;
      void main() { float d = length(gl_PointCoord - 0.5); float a = smoothstep(0.5, 0.0, d); gl_FragColor = vec4(vCol, a * vA); }`,
  });
  const points = new THREE.Points(geo, mat); scene.add(points);
  const glow = glowSprite(); scene.add(glow);
  return {
    kind: 'three',
    dispose() { freeScene(scene); },
    draw(dt, t, state, level) {
      if (state === 'thinking' && t > nextBoop) { m.boop.vel = 7; nextBoop = t + 2.6; }
      drive(m, state, level, dt); easeColors(cols, state, dt); feed(u, m, cols, t, state);
      points.rotation.y = t * 0.12 * (1 + m.energy.v);
      const split = clamp(m.split.v, 0, 1);
      glow.material.color.setRGB(cols[1][0] / 255, cols[1][1] / 255, cols[1][2] / 255);
      glow.scale.setScalar(2.2 * m.size.v * (1 - 0.6 * split)); glow.scale.y *= 1 - m.flat.v * 0.9;
      glow.material.opacity = 0.35 * u.uAlpha.value;
      renderer.render(scene, camera);
    },
  };
}

// Liquid: a glossy blob with an iridescent edge; ripples with the voice, dimples into a smile.
function makeLiquid() {
  const scene = new THREE.Scene(); const m = motion(); const u = sharedUniforms();
  const cols = [0, 1, 2, 3].map(() => hex('#8E8E93')); let nextBoop = 0;
  const geo = new THREE.IcosahedronGeometry(1, 48);
  const make = () => {
    const uu = { ...u, uOffset: { value: new THREE.Vector3() }, uScale: { value: 1 } };
    return new THREE.ShaderMaterial({
      uniforms: uu, transparent: true, depthWrite: false,
      vertexShader: `uniform float uTime, uVoice, uFlat, uSmile, uEnergy, uScale;
        uniform vec3 uOffset;
        varying vec3 vN; varying vec3 vView; varying float vMouth;
        ${NOISE}
        void main() {
          vec3 n = normalize(position);
          float d = (noise(n * 1.8 + uTime * 0.45) - 0.5) * (0.14 + uEnergy * 0.12)
                  + (noise(n * 4.0 - uTime * 1.6) - 0.5) * uVoice * 0.35;
          float mouth = smileBand(n);
          d -= mouth * uSmile * 0.12;
          vec3 p = uOffset + n * (1.0 + d) * uScale;
          p.y *= 1.0 - uFlat * 0.95;
          vec4 mv = modelViewMatrix * vec4(p, 1.0);
          gl_Position = projectionMatrix * mv;
          vN = normalize(normalMatrix * n); vView = -mv.xyz; vMouth = mouth;
        }`,
      fragmentShader: `uniform float uTime, uSmile, uAlpha; uniform vec3 uC0, uC1, uC2;
        varying vec3 vN; varying vec3 vView; varying float vMouth;
        void main() {
          vec3 N = normalize(vN); vec3 V = normalize(vView);
          float fres = pow(1.0 - max(dot(N, V), 0.0), 2.2);
          vec3 irid = mix(uC0, uC1, 0.5 + 0.5 * sin(N.y * 3.0 + uTime * 0.6));
          irid = mix(irid, uC2, 0.5 + 0.5 * sin(N.x * 2.4 - uTime * 0.4));
          vec3 L = normalize(vec3(-0.5, 0.7, 0.6));
          float spec = pow(max(dot(reflect(-L, N), V), 0.0), 40.0);
          vec3 col = irid * (0.1 + fres * 1.5) + vec3(1.0) * spec * 0.8 + uC1 * vMouth * uSmile * 1.2;
          gl_FragColor = vec4(col, (0.22 + fres * 0.78 + spec) * uAlpha);
        }`,
    });
  };
  const main = new THREE.Mesh(geo, make()); scene.add(main);
  const drops = [0, 1, 2].map(() => { const d = new THREE.Mesh(geo, make()); scene.add(d); return d; });
  return {
    kind: 'three',
    dispose() { freeScene(scene); },
    draw(dt, t, state, level) {
      if (state === 'thinking' && t > nextBoop) { m.boop.vel = 7; nextBoop = t + 2.6; }
      drive(m, state, level, dt); easeColors(cols, state, dt); feed(u, m, cols, t, state);
      const split = clamp(m.split.v, 0, 1.15);
      main.material.uniforms.uScale.value = 0.74 * m.size.v * Math.max(0.001, 1 - split);
      main.visible = split < 0.97;
      drops.forEach((d, i) => {
        d.visible = split > 0.03;
        d.material.uniforms.uScale.value = 0.32 * split;
        d.material.uniforms.uOffset.value.set((i - 1) * 0.95 * split, Math.sin(t * 4.2 - i * 0.95) * 0.2 * split, 0);
      });
      renderer.render(scene, camera);
    },
  };
}


// Frees a 3D design's GPU buffers when another design replaces it.
function freeScene(scene) {
  scene.traverse((o) => {
    if (o.geometry) o.geometry.dispose();
    if (o.material) { if (o.material.map) o.material.map.dispose(); o.material.dispose(); }
  });
}

// Only the design on screen is loaded: each is built the first time it is shown, and the one it
// replaces is freed, so switching moods never keeps every design in memory.
function create({ canvas2d, canvas3d }) {
  ctxHolder.canvas = canvas2d;
  ctxHolder.ctx = canvas2d.getContext('2d');
  const made = {};
  const makers = { strands: makeStrands, halo: makeHalo, fur: makeFur, nebula: makeNebula, liquid: makeLiquid };
  function design(name) {
    if (!makers[name]) return null;
    if (!made[name]) {
      if (makers[name] !== makeStrands && makers[name] !== makeHalo && !renderer) {
        renderer = new THREE.WebGLRenderer({ canvas: canvas3d, alpha: true, antialias: true });
        renderer.setPixelRatio(canvas3d.width / 200); renderer.setSize(200, 200, false); renderer.setClearColor(0x000000, 0);
      }
      made[name] = makers[name]();
    }
    return made[name];
  }
  return {
    names: Object.keys(makers),
    draw(name, dt, t, state, level) {
      for (const other of Object.keys(made)) {
        if (other !== name) { if (made[other].dispose) made[other].dispose(); delete made[other]; }
      }
      const d = design(name);
      canvas2d.style.display = d && d.kind === '2d' ? 'block' : 'none';
      canvas3d.style.display = d && d.kind === 'three' ? 'block' : 'none';
      if (d) d.draw(dt, t, state, level);
      return !!d;
    },
    hide() { canvas2d.style.display = 'none'; canvas3d.style.display = 'none'; },
  };
}

window.ZanaOrbDesigns = { create };
