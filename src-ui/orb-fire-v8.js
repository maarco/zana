/**
 * Fire Orb v8 - FULLSCREEN COSMOS (Lifecycle)
 *
 * Fills the entire screen with:
 * - Corner nebulas that breathe and pulse
 * - Stars scattered across the whole canvas
 * - Vignette darkening at the edges
 * - Central orb that responds to voice
 * - All transparent, overlays beautifully
 *
 * Processing state: warm fire palette shifts to cool blue/white
 * to give clear visual feedback that transcription is running.
 */

const canvas = document.getElementById('canvas');
const ctx = canvas.getContext('2d', { alpha: true });

// Screen dimensions (updated by lifecycle onResize)
let screenW = 400;
let screenH = 400;

// Processing color blend (0 = warm fire, 1 = cool blue)
let processingBlend = 0;
let targetProcessingBlend = 0;

// Fullscreen stars - distributed across entire canvas
const stars = [];
const NUM_STARS = 200;

function initStars() {
  stars.length = 0;
  for (let i = 0; i < NUM_STARS; i++) {
    stars.push({
      x: Math.random(),
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
  { x: 0, y: 0, size: 0.4, hue: 0, speed: 0.3 },
  { x: 1, y: 0, size: 0.35, hue: 30, speed: 0.25 },
  { x: 0, y: 1, size: 0.38, hue: 15, speed: 0.28 },
  { x: 1, y: 1, size: 0.42, hue: 45, speed: 0.32 },
  { x: 0.5, y: 0, size: 0.25, hue: 20, speed: 0.2 },
  { x: 0.5, y: 1, size: 0.28, hue: 35, speed: 0.22 },
  { x: 0, y: 0.5, size: 0.22, hue: 10, speed: 0.18 },
  { x: 1, y: 0.5, size: 0.24, hue: 40, speed: 0.2 },
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
    hue: Math.random() * 60
  });
}

// Color helpers
// Blend between warm fire (hue 0-60) and cool processing (blue/white)
function fireColor(hue, _saturation, lightness) {
  const h = hue / 60;
  // Warm fire colors
  const warmR = 255;
  const warmG = Math.floor(100 + h * 155);
  const warmB = Math.floor(20 + h * 80 + lightness * 100);

  // Cool processing colors (blue/white shifted)
  const coolR = Math.floor(120 + h * 60);
  const coolG = Math.floor(160 + h * 80);
  const coolB = 255;

  const b = processingBlend;
  return [
    Math.min(255, Math.round(warmR * (1 - b) + coolR * b)),
    Math.min(255, Math.round(warmG * (1 - b) + coolG * b)),
    Math.min(255, Math.round(warmB * (1 - b) + coolB * b))
  ];
}

function render(fc) {
  const w = fc.width;
  const h = fc.height;
  const cx = fc.cx;
  const cy = fc.cy;
  const maxR = Math.min(w, h) / 2;
  const t = fc.time;
  const genesis = fc.genesis;
  const globalOpacity = fc.globalOpacity;
  const audioLevel = fc.audioLevel;

  // Smooth processing color blend
  targetProcessingBlend = fc.isProcessing ? 1 : 0;
  processingBlend += (targetProcessingBlend - processingBlend) * 0.04;

  ctx.clearRect(0, 0, w, h);
  ctx.globalAlpha = globalOpacity;

  const audio = Math.min(audioLevel * 2.5, 1);
  const breathe = Math.sin(t * 0.4) * 0.5 + 0.5;

  // ========================================
  // FULLSCREEN STARS
  // ========================================
  for (let i = 0; i < stars.length; i++) {
    const star = stars[i];

    star.x += star.driftX;
    star.y += star.driftY;

    if (star.x < 0) star.x = 1;
    if (star.x > 1) star.x = 0;
    if (star.y < 0) star.y = 1;
    if (star.y > 1) star.y = 0;

    const sx = star.x * w;
    const sy = star.y * h;

    const twinkle = Math.sin(t * star.twinkleSpeed + star.twinkleOffset) * 0.5 + 0.5;
    const alpha = star.brightness * (0.15 + twinkle * 0.35) * (0.3 + genesis * 0.2 + audio * 0.2);
    const size = star.size * (0.8 + twinkle * 0.4 + audio * 0.3);

    if (alpha > 0.03) {
      const warmth = 180 + twinkle * 75;
      // Blend star color toward blue during processing
      const sr = Math.round(255 * (1 - processingBlend) + 180 * processingBlend);
      const sg = Math.round(warmth * (1 - processingBlend) + (200 + twinkle * 55) * processingBlend);
      const sb = Math.round((100 + twinkle * 50) * (1 - processingBlend) + 255 * processingBlend);
      ctx.fillStyle = `rgba(${sr}, ${sg}, ${sb}, ${alpha})`;
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

    const pulse = Math.sin(t * neb.speed + i) * 0.15 + 1;
    const nebSize = Math.max(w, h) * neb.size * pulse * (0.6 + genesis * 0.2 + audio * 0.2);
    const nebAlpha = (0.08 + genesis * 0.08 + audio * 0.1) * (0.6 + breathe * 0.2);

    const col = fireColor(neb.hue + breathe * 20, 0.8, 0.3);

    const nebGrad = ctx.createRadialGradient(nx, ny, 0, nx, ny, nebSize);
    nebGrad.addColorStop(0, `rgba(${col[0]}, ${col[1]}, ${col[2]}, ${nebAlpha * 0.4})`);
    nebGrad.addColorStop(0.3, `rgba(${col[0]}, ${Math.floor(col[1] * 0.7)}, ${Math.floor(col[2] * 0.5)}, ${nebAlpha * 0.2})`);
    nebGrad.addColorStop(0.6, `rgba(${Math.floor(col[0] * 0.8)}, ${Math.floor(col[1] * 0.5)}, ${Math.floor(col[2] * 0.3)}, ${nebAlpha * 0.08})`);
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

    p.x += Math.cos(p.angle) * p.speed * (1 + audio * 2);
    p.y += Math.sin(p.angle) * p.speed * (1 + audio * 2);
    p.angle += (Math.random() - 0.5) * 0.02;

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

  // Dormant ember
  const dormantAlpha = Math.pow(1 - genesis, 1.5) * 0.6;
  if (dormantAlpha > 0.01) {
    const emberSize = maxR * 0.02 * (1 + breathe * 0.3);
    // Ember color shifts too during processing blend-out
    const eR = Math.round(255 * (1 - processingBlend * 0.3));
    const eG = Math.round(220 * (1 - processingBlend * 0.2) + processingBlend * 30);
    const eB = Math.round(180 * (1 - processingBlend * 0.1) + processingBlend * 75);
    const emberGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, emberSize * 6);
    emberGrad.addColorStop(0, `rgba(${eR}, ${eG}, ${eB}, ${dormantAlpha})`);
    emberGrad.addColorStop(0.2, `rgba(${eR}, ${Math.floor(eG * 0.7)}, ${Math.floor(eB * 0.6)}, ${dormantAlpha * 0.7})`);
    emberGrad.addColorStop(0.5, `rgba(${eR}, ${Math.floor(eG * 0.45)}, ${Math.floor(eB * 0.3)}, ${dormantAlpha * 0.3})`);
    emberGrad.addColorStop(1, 'rgba(200, 60, 20, 0)');
    ctx.fillStyle = emberGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, emberSize * 6, 0, Math.PI * 2);
    ctx.fill();
  }

  // Active orb
  if (genesis > 0.01) {
    const alive = genesis * 0.7;

    // Core - blends warm->cool
    const coreGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, orbSize);
    const b = processingBlend;
    coreGrad.addColorStop(0, `rgba(${Math.round(255*(1-b*0.2))}, ${Math.round(255*(1-b*0.1))}, ${Math.round(250*(1-b*0.1)+b*5)}, ${alive * 0.9})`);
    coreGrad.addColorStop(0.15, `rgba(${Math.round(255*(1-b*0.3))}, ${Math.round(230*(1-b*0.1)+b*25)}, ${Math.round(180*(1-b)+255*b)}, ${alive * 0.8})`);
    coreGrad.addColorStop(0.35, `rgba(${Math.round(255*(1-b*0.5))}, ${Math.round(180*(1-b*0.2)+b*40)}, ${Math.round(100*(1-b)+220*b)}, ${alive * 0.6})`);
    coreGrad.addColorStop(0.55, `rgba(${Math.round(255*(1-b*0.6))}, ${Math.round(130*(1-b*0.1)+b*60)}, ${Math.round(50*(1-b)+200*b)}, ${alive * 0.35})`);
    coreGrad.addColorStop(0.75, `rgba(${Math.round(255*(1-b*0.7))}, ${Math.round(80*(1-b)+b*120)}, ${Math.round(25*(1-b)+180*b)}, ${alive * 0.15})`);
    coreGrad.addColorStop(1, `rgba(${Math.round(200*(1-b*0.5))}, ${Math.round(50*(1-b)+b*80)}, ${Math.round(15*(1-b)+120*b)}, 0)`);
    ctx.fillStyle = coreGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, orbSize, 0, Math.PI * 2);
    ctx.fill();

    // Outer glow
    const glowSize = orbSize * 2.5;
    const glowGrad = ctx.createRadialGradient(cx, cy, orbSize * 0.5, cx, cy, glowSize);
    glowGrad.addColorStop(0, `rgba(${Math.round(255*(1-b*0.5))}, ${Math.round(150*(1-b*0.2)+b*40)}, ${Math.round(80*(1-b)+200*b)}, ${alive * 0.25})`);
    glowGrad.addColorStop(0.4, `rgba(${Math.round(255*(1-b*0.6))}, ${Math.round(100*(1-b)+b*80)}, ${Math.round(50*(1-b)+180*b)}, ${alive * 0.1})`);
    glowGrad.addColorStop(1, `rgba(${Math.round(200*(1-b*0.5))}, ${Math.round(60*(1-b)+b*60)}, ${Math.round(25*(1-b)+100*b)}, 0)`);
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

        const pr = Math.round(255 * (1 - b * 0.4));
        const pg = Math.round(200 * (1 - b * 0.1) + b * 30);
        const pb = Math.round(120 * (1 - b) + 255 * b);
        ctx.strokeStyle = `rgba(${pr}, ${pg}, ${pb}, ${pulseAlpha})`;
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
  if (fc.isPoofing) {
    const p = fc.poofProgress;
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
        const bx = cx + Math.cos(angle) * dist;
        const by = cy + Math.sin(angle) * dist;
        const bAlpha = (1 - ease) * 0.8;
        const bSize = (2 + Math.sin(i * 7)) * (1 - ease);

        if (bAlpha > 0.03) {
          ctx.fillStyle = `rgba(255, ${180 + i * 2}, ${100 + i * 3}, ${bAlpha})`;
          ctx.beginPath();
          ctx.arc(bx, by, bSize, 0, Math.PI * 2);
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
    }
  }

  // ========================================
  // CORNER VIGNETTE (black corners)
  // ========================================
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
}

function onResize(w, h) {
  screenW = w;
  screenH = h;
}

console.log('[Orb-Fire-V8] FULLSCREEN COSMOS (Lifecycle) - Stars, nebulas, vignette with processing color shift');

// Register as lifecycle renderer
window.__orbRenderer = {
  render: render,
  onResize: onResize,
  onEnter: function(state) {
    if (state === 'PROCESSING') {
      targetProcessingBlend = 1;
    }
  },
  onExit: function(state) {
    if (state === 'PROCESSING') {
      targetProcessingBlend = 0;
    }
  }
};
