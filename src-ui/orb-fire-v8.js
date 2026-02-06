/**
 * Fire Orb v8 - FULLSCREEN COSMOS
 *
 * Fills the entire screen with:
 * - Corner nebulas that breathe and pulse
 * - Stars scattered across the whole canvas
 * - Vignette darkening at the edges
 * - Central orb that responds to voice
 * - All transparent, overlays beautifully
 */

const canvas = document.getElementById('canvas');
const ctx = canvas.getContext('2d', { alpha: true });

let time = 0;
let audioLevel = 0;
let targetAudioLevel = 0;
let isRecording = false;
let isProcessing = false;
let animationId = null;
let poofTime = 0;
let isPoofing = false;
let globalOpacity = 0;
let targetOpacity = 1;
let isFadingOut = false;

// Genesis: 0 = dormant, 1 = active
let genesis = 0;
let targetGenesis = 0;

// Screen dimensions (updated on resize)
let screenW = 400;
let screenH = 400;

// Fullscreen stars - distributed across entire canvas
const stars = [];
const NUM_STARS = 200;

function initStars() {
  stars.length = 0;
  for (let i = 0; i < NUM_STARS; i++) {
    stars.push({
      x: Math.random(),  // 0-1 normalized position
      y: Math.random(),
      size: 0.3 + Math.random() * 1.5,
      brightness: 0.2 + Math.random() * 0.8,
      twinkleSpeed: 0.5 + Math.random() * 3,
      twinkleOffset: Math.random() * Math.PI * 2,
      driftX: (Math.random() - 0.5) * 0.0003,
      driftY: (Math.random() - 0.5) * 0.0003
    });
  }
}
initStars();

// Corner nebulas - one for each corner + edges
const nebulas = [
  // Corners
  { x: 0, y: 0, size: 0.4, hue: 0, speed: 0.3 },      // top-left
  { x: 1, y: 0, size: 0.35, hue: 30, speed: 0.25 },   // top-right
  { x: 0, y: 1, size: 0.38, hue: 15, speed: 0.28 },   // bottom-left
  { x: 1, y: 1, size: 0.42, hue: 45, speed: 0.32 },   // bottom-right
  // Edges
  { x: 0.5, y: 0, size: 0.25, hue: 20, speed: 0.2 },  // top-center
  { x: 0.5, y: 1, size: 0.28, hue: 35, speed: 0.22 }, // bottom-center
  { x: 0, y: 0.5, size: 0.22, hue: 10, speed: 0.18 }, // left-center
  { x: 1, y: 0.5, size: 0.24, hue: 40, speed: 0.2 },  // right-center
];

// Floating particles across screen
const particles = [];
const NUM_PARTICLES = 80;
for (let i = 0; i < NUM_PARTICLES; i++) {
  particles.push({
    x: Math.random(),
    y: Math.random(),
    size: 0.5 + Math.random() * 2,
    speed: 0.0002 + Math.random() * 0.0008,
    angle: Math.random() * Math.PI * 2,
    brightness: 0.1 + Math.random() * 0.4,
    hue: Math.random() * 60  // orange to yellow range
  });
}

// API for Rust
window.setAudioLevel = function(level) {
  targetAudioLevel = level;
};

window.setRecordingState = function(recording, processing) {
  isRecording = recording;
  isProcessing = processing;
  if (recording) {
    targetGenesis = 1;
    audioLevel = 0;
    targetAudioLevel = 0;
  }
};

window.setTranscriptionComplete = function() {
  isProcessing = false;
  isPoofing = true;
  poofTime = 0;
};

window.fadeIn = function() {
  targetOpacity = 1;
  isFadingOut = false;
};

window.fadeOut = function() {
  targetOpacity = 0;
  isFadingOut = true;
  targetGenesis = 0;
};

window.resetOrb = function() {
  globalOpacity = 0;
  targetOpacity = 1;
  isFadingOut = false;
  isRecording = false;
  isProcessing = false;
  isPoofing = false;
  genesis = 0;
  targetGenesis = 0;
  audioLevel = 0;
};

window.isFadeComplete = function() {
  return isFadingOut && globalOpacity < 0.01;
};

function resize() {
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
  ctx.scale(dpr, dpr);
  screenW = rect.width;
  screenH = rect.height;
}

// Convert hue (0-60 for fire colors) to RGB
function fireColor(hue, saturation, lightness) {
  // Map 0-60 to orange-yellow-white fire spectrum
  const h = hue / 60;
  const r = 255;
  const g = Math.floor(100 + h * 155);
  const b = Math.floor(20 + h * 80 + lightness * 100);
  return [
    Math.min(255, r),
    Math.min(255, g),
    Math.min(255, b)
  ];
}

function render() {
  const w = screenW;
  const h = screenH;
  const cx = w / 2;
  const cy = h / 2;
  const maxR = Math.min(w, h) / 2;

  time += 0.016;
  const t = time;

  // Smooth transitions
  globalOpacity += (targetOpacity - globalOpacity) * 0.08;
  audioLevel += (targetAudioLevel - audioLevel) * 0.18;

  // Genesis
  const genSpeed = targetGenesis > genesis ? 0.05 : 0.015;
  genesis += (targetGenesis - genesis) * genSpeed;

  ctx.clearRect(0, 0, w, h);
  ctx.globalAlpha = globalOpacity;

  const audio = Math.min(audioLevel * 2.5, 1);
  const breathe = Math.sin(t * 0.4) * 0.5 + 0.5;

  // ========================================
  // FULLSCREEN STARS
  // ========================================
  for (let i = 0; i < stars.length; i++) {
    const star = stars[i];

    // Gentle drift
    star.x += star.driftX;
    star.y += star.driftY;

    // Wrap around
    if (star.x < 0) star.x = 1;
    if (star.x > 1) star.x = 0;
    if (star.y < 0) star.y = 1;
    if (star.y > 1) star.y = 0;

    const sx = star.x * w;
    const sy = star.y * h;

    // Twinkle - more transparent
    const twinkle = Math.sin(t * star.twinkleSpeed + star.twinkleOffset) * 0.5 + 0.5;
    const alpha = star.brightness * (0.15 + twinkle * 0.35) * (0.3 + genesis * 0.2 + audio * 0.2);
    const size = star.size * (0.8 + twinkle * 0.4 + audio * 0.3);

    if (alpha > 0.03) {
      // Warm star color
      const warmth = 180 + twinkle * 75;
      ctx.fillStyle = `rgba(255, ${warmth}, ${100 + twinkle * 50}, ${alpha})`;
      ctx.beginPath();
      ctx.arc(sx, sy, size, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  // ========================================
  // CORNER & EDGE NEBULAS
  // ========================================
  for (let i = 0; i < nebulas.length; i++) {
    const neb = nebulas[i];

    const nx = neb.x * w;
    const ny = neb.y * h;

    // Pulsing size
    const pulse = Math.sin(t * neb.speed + i) * 0.15 + 1;
    const nebSize = Math.max(w, h) * neb.size * pulse * (0.6 + genesis * 0.2 + audio * 0.2);

    // Nebula alpha - more transparent
    const nebAlpha = (0.08 + genesis * 0.08 + audio * 0.1) * (0.6 + breathe * 0.2);

    // Fire-colored nebula
    const col = fireColor(neb.hue + breathe * 20, 0.8, 0.3);

    const nebGrad = ctx.createRadialGradient(nx, ny, 0, nx, ny, nebSize);
    nebGrad.addColorStop(0, `rgba(${col[0]}, ${col[1]}, ${col[2]}, ${nebAlpha * 0.4})`);
    nebGrad.addColorStop(0.3, `rgba(${col[0]}, ${col[1] * 0.7}, ${col[2] * 0.5}, ${nebAlpha * 0.2})`);
    nebGrad.addColorStop(0.6, `rgba(${col[0] * 0.8}, ${col[1] * 0.5}, ${col[2] * 0.3}, ${nebAlpha * 0.08})`);
    nebGrad.addColorStop(1, 'rgba(100, 40, 10, 0)');

    ctx.fillStyle = nebGrad;
    ctx.beginPath();
    ctx.arc(nx, ny, nebSize, 0, Math.PI * 2);
    ctx.fill();
  }

  // ========================================
  // FLOATING PARTICLES
  // ========================================
  for (let i = 0; i < particles.length; i++) {
    const p = particles[i];

    // Float around
    p.x += Math.cos(p.angle) * p.speed * (1 + audio * 2);
    p.y += Math.sin(p.angle) * p.speed * (1 + audio * 2);
    p.angle += (Math.random() - 0.5) * 0.02;

    // Wrap
    if (p.x < -0.1) p.x = 1.1;
    if (p.x > 1.1) p.x = -0.1;
    if (p.y < -0.1) p.y = 1.1;
    if (p.y > 1.1) p.y = -0.1;

    const px = p.x * w;
    const py = p.y * h;

    const pAlpha = p.brightness * (0.2 + genesis * 0.15 + audio * 0.15);
    const pSize = p.size * (1 + audio * 0.5);

    if (pAlpha > 0.02) {
      const col = fireColor(p.hue, 0.7, 0.4);
      ctx.fillStyle = `rgba(${col[0]}, ${col[1]}, ${col[2]}, ${pAlpha})`;
      ctx.beginPath();
      ctx.arc(px, py, pSize, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  // ========================================
  // CENTRAL ORB
  // ========================================
  const orbBaseSize = maxR * 0.12;
  const orbSize = orbBaseSize * (0.3 + genesis * 0.7) * (1 + audio * 1.5 + breathe * 0.1);

  // Dormant ember - more transparent
  const dormantAlpha = Math.pow(1 - genesis, 1.5) * 0.6;
  if (dormantAlpha > 0.01) {
    const emberSize = maxR * 0.02 * (1 + breathe * 0.3);
    const emberGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, emberSize * 6);
    emberGrad.addColorStop(0, `rgba(255, 220, 180, ${dormantAlpha})`);
    emberGrad.addColorStop(0.2, `rgba(255, 160, 100, ${dormantAlpha * 0.7})`);
    emberGrad.addColorStop(0.5, `rgba(255, 100, 50, ${dormantAlpha * 0.3})`);
    emberGrad.addColorStop(1, 'rgba(200, 60, 20, 0)');
    ctx.fillStyle = emberGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, emberSize * 6, 0, Math.PI * 2);
    ctx.fill();
  }

  // Active orb - more transparent
  if (genesis > 0.01) {
    const alive = genesis * 0.7;  // Overall more transparent

    // Core
    const coreGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, orbSize);
    coreGrad.addColorStop(0, `rgba(255, 255, 250, ${alive * 0.9})`);
    coreGrad.addColorStop(0.15, `rgba(255, 230, 180, ${alive * 0.8})`);
    coreGrad.addColorStop(0.35, `rgba(255, 180, 100, ${alive * 0.6})`);
    coreGrad.addColorStop(0.55, `rgba(255, 130, 50, ${alive * 0.35})`);
    coreGrad.addColorStop(0.75, `rgba(255, 80, 25, ${alive * 0.15})`);
    coreGrad.addColorStop(1, 'rgba(200, 50, 15, 0)');
    ctx.fillStyle = coreGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, orbSize, 0, Math.PI * 2);
    ctx.fill();

    // Outer glow
    const glowSize = orbSize * 2.5;
    const glowGrad = ctx.createRadialGradient(cx, cy, orbSize * 0.5, cx, cy, glowSize);
    glowGrad.addColorStop(0, `rgba(255, 150, 80, ${alive * 0.25})`);
    glowGrad.addColorStop(0.4, `rgba(255, 100, 50, ${alive * 0.1})`);
    glowGrad.addColorStop(1, 'rgba(200, 60, 25, 0)');
    ctx.fillStyle = glowGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, glowSize, 0, Math.PI * 2);
    ctx.fill();

    // Audio pulse rings
    if (audio > 0.08) {
      for (let i = 0; i < 3; i++) {
        const pulsePhase = (t * 2 + i * 0.4) % 1;
        const pulseRadius = orbSize * (1 + pulsePhase * 2);
        const pulseAlpha = (1 - pulsePhase) * audio * 0.4 * alive;

        ctx.strokeStyle = `rgba(255, 200, 120, ${pulseAlpha})`;
        ctx.lineWidth = 2 * (1 - pulsePhase);
        ctx.beginPath();
        ctx.arc(cx, cy, pulseRadius, 0, Math.PI * 2);
        ctx.stroke();
      }
    }
  }

  // ========================================
  // POOF
  // ========================================
  if (isPoofing) {
    poofTime += 0.016;
    const poofDuration = 1.0;
    const p = poofTime / poofDuration;
    const ease = 1 - Math.pow(1 - p, 3);

    if (p < 1) {
      // Expanding rings
      for (let i = 0; i < 4; i++) {
        const ringP = Math.max(0, (p - i * 0.07) / 0.85);
        if (ringP > 0 && ringP < 1) {
          const ringR = maxR * (0.05 + ringP * 0.5);
          const ringA = (1 - ringP) * 0.6;
          ctx.strokeStyle = `rgba(255, 200, 120, ${ringA})`;
          ctx.lineWidth = 3 * (1 - ringP);
          ctx.beginPath();
          ctx.arc(cx, cy, ringR, 0, Math.PI * 2);
          ctx.stroke();
        }
      }

      // Burst particles fly to edges
      for (let i = 0; i < 30; i++) {
        const angle = (i / 30) * Math.PI * 2 + Math.sin(i * 11) * 0.3;
        const dist = maxR * ease * (0.5 + Math.sin(i * 17) * 0.5);
        const px = cx + Math.cos(angle) * dist;
        const py = cy + Math.sin(angle) * dist;
        const pAlpha = (1 - ease) * 0.8;
        const pSize = (2 + Math.sin(i * 7)) * (1 - ease);

        if (pAlpha > 0.03) {
          ctx.fillStyle = `rgba(255, ${180 + i * 2}, ${100 + i * 3}, ${pAlpha})`;
          ctx.beginPath();
          ctx.arc(px, py, pSize, 0, Math.PI * 2);
          ctx.fill();
        }
      }

      // Central flash
      const flashA = (1 - ease) * 0.6;
      const flashR = maxR * 0.2 * (1 - ease * 0.5);
      const flashGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, flashR);
      flashGrad.addColorStop(0, `rgba(255, 255, 240, ${flashA})`);
      flashGrad.addColorStop(0.4, `rgba(255, 200, 120, ${flashA * 0.5})`);
      flashGrad.addColorStop(1, 'rgba(255, 150, 60, 0)');
      ctx.fillStyle = flashGrad;
      ctx.beginPath();
      ctx.arc(cx, cy, flashR, 0, Math.PI * 2);
      ctx.fill();
    } else {
      isPoofing = false;
      targetGenesis = 0;
    }
  }

  // ========================================
  // CORNER VIGNETTE (black corners)
  // ========================================
  // Top-left
  const vigSize = Math.max(w, h) * 0.6;
  const corners = [
    [0, 0],
    [w, 0],
    [0, h],
    [w, h]
  ];

  for (const [vx, vy] of corners) {
    const vigGrad = ctx.createRadialGradient(vx, vy, 0, vx, vy, vigSize);
    vigGrad.addColorStop(0, 'rgba(0, 0, 0, 0.25)');
    vigGrad.addColorStop(0.3, 'rgba(0, 0, 0, 0.12)');
    vigGrad.addColorStop(0.6, 'rgba(0, 0, 0, 0.04)');
    vigGrad.addColorStop(1, 'rgba(0, 0, 0, 0)');
    ctx.fillStyle = vigGrad;
    ctx.fillRect(0, 0, w, h);
  }

  // ========================================
  // EDGE FADE (for the window boundary)
  // ========================================
  ctx.globalCompositeOperation = 'destination-in';
  const fadeGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, Math.max(w, h) * 0.7);
  fadeGrad.addColorStop(0, 'rgba(255,255,255,1)');
  fadeGrad.addColorStop(0.6, 'rgba(255,255,255,1)');
  fadeGrad.addColorStop(0.85, 'rgba(255,255,255,0.5)');
  fadeGrad.addColorStop(1, 'rgba(255,255,255,0)');
  ctx.fillStyle = fadeGrad;
  ctx.fillRect(0, 0, w, h);
  ctx.globalCompositeOperation = 'source-over';

  animationId = requestAnimationFrame(render);
}

function start() {
  resize();
  if (!animationId) render();
}

console.log('[Orb-Fire-V8] FULLSCREEN COSMOS - Stars, nebulas, and vignette across entire screen');

window.addEventListener('resize', resize);
start();
