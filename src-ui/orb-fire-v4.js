/**
 * Fire Orb v4 - EPIC Genesis Edition
 *
 * Stars, rings, edge fade, traveling particles
 * Birth from spark, death back to ember
 */

const canvas = document.getElementById('canvas');
const ctx = canvas.getContext('2d', { alpha: true });
const statusEl = document.getElementById('status');

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

// Genesis: 0 = tiny ember, 1 = full blazing orb
let genesis = 0;
let targetGenesis = 0;

// Traveling stars - persistent across frames
const stars = [];
const NUM_STARS = 60;
for (let i = 0; i < NUM_STARS; i++) {
  stars.push({
    angle: Math.random() * Math.PI * 2,
    dist: 0.3 + Math.random() * 0.6,
    speed: 0.2 + Math.random() * 0.4,
    size: 1 + Math.random() * 2,
    twinkleSpeed: 2 + Math.random() * 3,
    twinkleOffset: Math.random() * Math.PI * 2
  });
}

// Rings
const rings = [];
const NUM_RINGS = 3;
for (let i = 0; i < NUM_RINGS; i++) {
  rings.push({
    radius: 0.4 + i * 0.2,
    speed: 0.5 + i * 0.3,
    width: 1.5 - i * 0.3,
    opacity: 0.4 - i * 0.1
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
    targetGenesis = 1; // BIRTH
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
}

function render() {
  const w = canvas.getBoundingClientRect().width;
  const h = canvas.getBoundingClientRect().height;
  const cx = w / 2;
  const cy = h / 2;
  const maxR = Math.min(w, h) / 2;

  time += 0.016;
  const t = time;

  // Smooth transitions
  globalOpacity += (targetOpacity - globalOpacity) * 0.08;
  audioLevel += (targetAudioLevel - audioLevel) * 0.15;

  // Genesis animation - slow dramatic birth, even slower death
  const genSpeed = targetGenesis > genesis ? 0.02 : 0.008;
  genesis += (targetGenesis - genesis) * genSpeed;

  ctx.clearRect(0, 0, w, h);
  ctx.globalAlpha = globalOpacity;

  const breathe = Math.sin(t * 0.5) * 0.5 + 0.5;
  const audio = Math.min(audioLevel * 2, 1);


  // ========================================
  // DORMANT EMBER (genesis ~ 0)
  // ========================================
  const emberAlpha = 1 - genesis;
  if (emberAlpha > 0.01) {
    // Core ember
    const emberSize = maxR * (0.04 + breathe * 0.015);
    const emberGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, emberSize * 4);
    emberGrad.addColorStop(0, `rgba(255, 200, 100, ${emberAlpha})`);
    emberGrad.addColorStop(0.2, `rgba(255, 140, 50, ${emberAlpha * 0.9})`);
    emberGrad.addColorStop(0.5, `rgba(255, 80, 20, ${emberAlpha * 0.5})`);
    emberGrad.addColorStop(1, 'rgba(255, 40, 10, 0)');
    ctx.fillStyle = emberGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, emberSize * 4, 0, Math.PI * 2);
    ctx.fill();

    // Ember glow pulse
    const pulseSize = maxR * (0.12 + breathe * 0.04);
    const pulseGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, pulseSize);
    pulseGrad.addColorStop(0, `rgba(255, 120, 40, ${emberAlpha * 0.4})`);
    pulseGrad.addColorStop(0.5, `rgba(255, 60, 20, ${emberAlpha * 0.2})`);
    pulseGrad.addColorStop(1, 'rgba(200, 40, 10, 0)');
    ctx.fillStyle = pulseGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, pulseSize, 0, Math.PI * 2);
    ctx.fill();
  }

  // ========================================
  // ALIVE ORB (genesis ~ 1)
  // ========================================
  if (genesis > 0.01) {
    const alive = genesis;
    const orbSize = maxR * (0.25 + audio * 0.15 + breathe * 0.03) * alive;

    // Deep fire core
    const coreGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, orbSize);
    coreGrad.addColorStop(0, `rgba(255, 255, 220, ${alive})`);
    coreGrad.addColorStop(0.15, `rgba(255, 220, 120, ${alive * 0.95})`);
    coreGrad.addColorStop(0.3, `rgba(255, 160, 60, ${alive * 0.8})`);
    coreGrad.addColorStop(0.5, `rgba(255, 100, 30, ${alive * 0.5})`);
    coreGrad.addColorStop(0.7, `rgba(255, 60, 15, ${alive * 0.3})`);
    coreGrad.addColorStop(1, 'rgba(200, 40, 10, 0)');
    ctx.fillStyle = coreGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, orbSize, 0, Math.PI * 2);
    ctx.fill();

    // Outer fire glow
    const glowSize = orbSize * 2.5;
    const glowGrad = ctx.createRadialGradient(cx, cy, orbSize * 0.5, cx, cy, glowSize);
    glowGrad.addColorStop(0, `rgba(255, 140, 50, ${alive * 0.4})`);
    glowGrad.addColorStop(0.4, `rgba(255, 80, 30, ${alive * 0.2})`);
    glowGrad.addColorStop(0.7, `rgba(200, 50, 20, ${alive * 0.1})`);
    glowGrad.addColorStop(1, 'rgba(150, 30, 10, 0)');
    ctx.fillStyle = glowGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, glowSize, 0, Math.PI * 2);
    ctx.fill();

    // ========================================
    // ROTATING RINGS
    // ========================================
    for (let i = 0; i < rings.length; i++) {
      const ring = rings[i];
      const ringRadius = orbSize * (1.2 + ring.radius * audio * 0.5);
      const ringAlpha = ring.opacity * alive * (0.5 + audio * 0.5);

      ctx.save();
      ctx.translate(cx, cy);
      ctx.rotate(t * ring.speed * (i % 2 === 0 ? 1 : -1));

      // Dashed ring for style
      ctx.setLineDash([10 + i * 5, 15 + i * 3]);
      ctx.strokeStyle = `rgba(255, ${150 - i * 30}, ${60 - i * 20}, ${ringAlpha})`;
      ctx.lineWidth = ring.width * (1 + audio);
      ctx.beginPath();
      ctx.arc(0, 0, ringRadius, 0, Math.PI * 2);
      ctx.stroke();
      ctx.setLineDash([]);

      ctx.restore();
    }

    // ========================================
    // TRAVELING STARS
    // ========================================
    for (let i = 0; i < stars.length; i++) {
      const star = stars[i];

      // Update star position - orbit around center
      star.angle += star.speed * 0.016 * (1 + audio * 2);

      // Stars move outward with audio, inward when quiet
      const targetDist = 0.3 + star.dist * genesis + audio * 0.3;
      const starDist = orbSize * (1.5 + targetDist);

      const sx = cx + Math.cos(star.angle) * starDist;
      const sy = cy + Math.sin(star.angle) * starDist;

      // Twinkle
      const twinkle = Math.sin(t * star.twinkleSpeed + star.twinkleOffset);
      const starAlpha = (0.3 + twinkle * 0.4) * alive * (0.6 + audio * 0.4);
      const starSize = star.size * (1 + audio * 0.5 + twinkle * 0.3);

      if (starAlpha > 0) {
        // Star glow
        const starGrad = ctx.createRadialGradient(sx, sy, 0, sx, sy, starSize * 3);
        starGrad.addColorStop(0, `rgba(255, 240, 200, ${starAlpha})`);
        starGrad.addColorStop(0.3, `rgba(255, 200, 120, ${starAlpha * 0.6})`);
        starGrad.addColorStop(1, 'rgba(255, 150, 80, 0)');
        ctx.fillStyle = starGrad;
        ctx.beginPath();
        ctx.arc(sx, sy, starSize * 3, 0, Math.PI * 2);
        ctx.fill();

        // Star core
        ctx.fillStyle = `rgba(255, 255, 240, ${starAlpha})`;
        ctx.beginPath();
        ctx.arc(sx, sy, starSize, 0, Math.PI * 2);
        ctx.fill();
      }
    }

    // ========================================
    // AUDIO PULSE RINGS
    // ========================================
    if (audio > 0.1) {
      for (let i = 0; i < 2; i++) {
        const pulsePhase = (t * 2 + i * 0.5) % 1;
        const pulseRadius = orbSize * (1 + pulsePhase * 1.5);
        const pulseAlpha = (1 - pulsePhase) * audio * 0.4 * alive;

        ctx.strokeStyle = `rgba(255, 180, 80, ${pulseAlpha})`;
        ctx.lineWidth = 2 * (1 - pulsePhase);
        ctx.beginPath();
        ctx.arc(cx, cy, pulseRadius, 0, Math.PI * 2);
        ctx.stroke();
      }
    }
  }

  // ========================================
  // POOF EXPLOSION
  // ========================================
  if (isPoofing) {
    poofTime += 0.016;
    const poofDuration = 1.2;
    const p = poofTime / poofDuration;
    const ease = 1 - Math.pow(1 - p, 3);

    if (p < 1) {
      // Expanding rings
      for (let i = 0; i < 3; i++) {
        const ringP = Math.max(0, (p - i * 0.1) / 0.9);
        if (ringP > 0 && ringP < 1) {
          const ringR = maxR * (0.1 + ringP * 0.8);
          const ringA = (1 - ringP) * 0.6;
          ctx.strokeStyle = `rgba(255, 200, 100, ${ringA})`;
          ctx.lineWidth = 3 * (1 - ringP);
          ctx.beginPath();
          ctx.arc(cx, cy, ringR, 0, Math.PI * 2);
          ctx.stroke();
        }
      }

      // Burst particles
      for (let i = 0; i < 20; i++) {
        const angle = (i / 20) * Math.PI * 2 + Math.sin(i * 7) * 0.3;
        const speed = 0.6 + Math.sin(i * 13) * 0.4;
        const dist = maxR * ease * speed * 0.9;
        const px = cx + Math.cos(angle) * dist;
        const py = cy + Math.sin(angle) * dist;
        const pAlpha = (1 - ease) * 0.8;
        const pSize = 3 * (1 - ease);

        if (pAlpha > 0) {
          ctx.fillStyle = `rgba(255, ${180 + i * 3}, ${80 + i * 5}, ${pAlpha})`;
          ctx.beginPath();
          ctx.arc(px, py, pSize, 0, Math.PI * 2);
          ctx.fill();
        }
      }

      // Central flash
      const flashA = (1 - ease) * 0.5;
      const flashR = maxR * 0.3 * (1 - ease * 0.5);
      const flashGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, flashR);
      flashGrad.addColorStop(0, `rgba(255, 255, 200, ${flashA})`);
      flashGrad.addColorStop(0.5, `rgba(255, 200, 100, ${flashA * 0.5})`);
      flashGrad.addColorStop(1, 'rgba(255, 150, 50, 0)');
      ctx.fillStyle = flashGrad;
      ctx.beginPath();
      ctx.arc(cx, cy, flashR, 0, Math.PI * 2);
      ctx.fill();
    } else {
      isPoofing = false;
      targetGenesis = 0; // DEATH - back to ember
    }
  }

  // ========================================
  // EDGE FADE (transparent at edges)
  // ========================================
  // Use destination-in compositing to fade edges to transparent
  ctx.globalCompositeOperation = 'destination-in';
  const fadeGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, maxR);
  fadeGrad.addColorStop(0, 'rgba(255,255,255,1)');
  fadeGrad.addColorStop(0.6, 'rgba(255,255,255,1)');
  fadeGrad.addColorStop(0.85, 'rgba(255,255,255,0.5)');
  fadeGrad.addColorStop(1, 'rgba(255,255,255,0)');
  ctx.fillStyle = fadeGrad;
  ctx.fillRect(0, 0, w, h);
  ctx.globalCompositeOperation = 'source-over';

  animationId = requestAnimationFrame(render);
}

function updateStatus() {}

function start() {
  resize();
  if (!animationId) render();
}

console.log('[Orb-Fire-V4] EPIC Genesis with stars, rings, and edge fade');

window.addEventListener('resize', resize);
start();
