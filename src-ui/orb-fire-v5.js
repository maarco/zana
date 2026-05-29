/**
 * Fire Orb v5 - VOID AWAKENING
 *
 * A tiny spark sleeps in infinite darkness.
 * When you speak, it IGNITES - fast, fierce, alive.
 * Stars orbit, rings pulse, fire breathes.
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

// Genesis: 0 = microscopic spark in void, 1 = blazing sun
let genesis = 0;
let targetGenesis = 0;

// Traveling stars - born with the orb
const stars = [];
const NUM_STARS = 80;
for (let i = 0; i < NUM_STARS; i++) {
  stars.push({
    angle: Math.random() * Math.PI * 2,
    baseDist: 0.2 + Math.random() * 0.8,
    speed: 0.3 + Math.random() * 0.6,
    size: 0.5 + Math.random() * 2.5,
    twinkleSpeed: 1.5 + Math.random() * 4,
    twinkleOffset: Math.random() * Math.PI * 2,
    orbitTilt: (Math.random() - 0.5) * 0.3
  });
}

// Orbital rings
const rings = [];
const NUM_RINGS = 4;
for (let i = 0; i < NUM_RINGS; i++) {
  rings.push({
    radius: 0.3 + i * 0.25,
    speed: 0.4 + i * 0.2,
    width: 2 - i * 0.3,
    opacity: 0.5 - i * 0.1,
    dashLength: 8 + i * 6
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
    targetGenesis = 1; // AWAKEN
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
  audioLevel += (targetAudioLevel - audioLevel) * 0.18;

  // Genesis: FAST awakening (0.06), slow fade to sleep (0.012)
  const genSpeed = targetGenesis > genesis ? 0.06 : 0.012;
  genesis += (targetGenesis - genesis) * genSpeed;

  ctx.clearRect(0, 0, w, h);
  ctx.globalAlpha = globalOpacity;

  const breathe = Math.sin(t * 0.6) * 0.5 + 0.5;
  const audio = Math.min(audioLevel * 2.5, 1);

  // Scale factor - tiny when dormant, full when alive
  const scale = 0.08 + genesis * 0.92;

  // ========================================
  // THE VOID SPARK (genesis ~ 0)
  // ========================================
  const sparkAlpha = Math.pow(1 - genesis, 2);
  if (sparkAlpha > 0.01) {
    // Microscopic core
    const sparkSize = maxR * 0.015 * (1 + breathe * 0.3);

    // Inner white-hot point
    const sparkGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, sparkSize * 6);
    sparkGrad.addColorStop(0, `rgba(255, 255, 255, ${sparkAlpha})`);
    sparkGrad.addColorStop(0.1, `rgba(255, 240, 200, ${sparkAlpha * 0.95})`);
    sparkGrad.addColorStop(0.3, `rgba(255, 180, 80, ${sparkAlpha * 0.7})`);
    sparkGrad.addColorStop(0.6, `rgba(255, 100, 30, ${sparkAlpha * 0.3})`);
    sparkGrad.addColorStop(1, 'rgba(200, 50, 10, 0)');
    ctx.fillStyle = sparkGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, sparkSize * 6, 0, Math.PI * 2);
    ctx.fill();

    // Faint pulse in the void
    const voidPulse = maxR * 0.06 * (1 + breathe * 0.5);
    const voidGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, voidPulse);
    voidGrad.addColorStop(0, `rgba(255, 120, 40, ${sparkAlpha * 0.25})`);
    voidGrad.addColorStop(0.5, `rgba(200, 60, 20, ${sparkAlpha * 0.1})`);
    voidGrad.addColorStop(1, 'rgba(150, 30, 10, 0)');
    ctx.fillStyle = voidGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, voidPulse, 0, Math.PI * 2);
    ctx.fill();
  }

  // ========================================
  // THE AWAKENED SUN (genesis ~ 1)
  // ========================================
  if (genesis > 0.01) {
    const alive = genesis;
    const baseSize = maxR * 0.28;
    const orbSize = baseSize * scale * (1 + audio * 0.4 + breathe * 0.08);

    // Deep molten core
    const coreGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, orbSize);
    coreGrad.addColorStop(0, `rgba(255, 255, 250, ${alive})`);
    coreGrad.addColorStop(0.1, `rgba(255, 250, 200, ${alive * 0.98})`);
    coreGrad.addColorStop(0.25, `rgba(255, 200, 100, ${alive * 0.9})`);
    coreGrad.addColorStop(0.45, `rgba(255, 130, 40, ${alive * 0.7})`);
    coreGrad.addColorStop(0.65, `rgba(255, 80, 20, ${alive * 0.4})`);
    coreGrad.addColorStop(0.85, `rgba(200, 50, 10, ${alive * 0.15})`);
    coreGrad.addColorStop(1, 'rgba(150, 30, 5, 0)');
    ctx.fillStyle = coreGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, orbSize, 0, Math.PI * 2);
    ctx.fill();

    // Corona / outer fire
    const coronaSize = orbSize * 2.2;
    const coronaGrad = ctx.createRadialGradient(cx, cy, orbSize * 0.6, cx, cy, coronaSize);
    coronaGrad.addColorStop(0, `rgba(255, 160, 60, ${alive * 0.35})`);
    coronaGrad.addColorStop(0.3, `rgba(255, 100, 30, ${alive * 0.2})`);
    coronaGrad.addColorStop(0.6, `rgba(220, 60, 15, ${alive * 0.1})`);
    coronaGrad.addColorStop(1, 'rgba(180, 40, 10, 0)');
    ctx.fillStyle = coronaGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, coronaSize, 0, Math.PI * 2);
    ctx.fill();

    // ========================================
    // ORBITAL RINGS
    // ========================================
    for (let i = 0; i < rings.length; i++) {
      const ring = rings[i];
      const ringRadius = orbSize * (1.3 + ring.radius * (0.8 + audio * 0.4));
      const ringAlpha = ring.opacity * alive * (0.4 + audio * 0.6) * scale;

      if (ringAlpha > 0.02) {
        ctx.save();
        ctx.translate(cx, cy);
        ctx.rotate(t * ring.speed * (i % 2 === 0 ? 1 : -1));

        ctx.setLineDash([ring.dashLength, ring.dashLength * 1.5]);
        ctx.strokeStyle = `rgba(255, ${180 - i * 25}, ${80 - i * 15}, ${ringAlpha})`;
        ctx.lineWidth = ring.width * (1 + audio * 0.8) * scale;
        ctx.beginPath();
        ctx.arc(0, 0, ringRadius, 0, Math.PI * 2);
        ctx.stroke();
        ctx.setLineDash([]);

        ctx.restore();
      }
    }

    // ========================================
    // ORBITING STARS
    // ========================================
    for (let i = 0; i < stars.length; i++) {
      const star = stars[i];

      // Stars orbit faster with audio
      star.angle += star.speed * 0.016 * (0.8 + audio * 2.5);

      // Distance scales with genesis - stars emerge from center
      const starDist = orbSize * (1.4 + star.baseDist * 1.2) * (0.3 + genesis * 0.7);

      // 3D-ish orbit with tilt
      const tiltedY = Math.sin(star.angle) * (1 + star.orbitTilt);
      const sx = cx + Math.cos(star.angle) * starDist;
      const sy = cy + tiltedY * starDist;

      // Twinkle effect
      const twinkle = Math.sin(t * star.twinkleSpeed + star.twinkleOffset);
      const starAlpha = (0.25 + twinkle * 0.5) * alive * (0.5 + audio * 0.5) * scale;
      const starSize = star.size * (1 + audio * 0.6 + twinkle * 0.25) * (0.5 + genesis * 0.5);

      if (starAlpha > 0.02 && starSize > 0.3) {
        // Star glow
        const starGrad = ctx.createRadialGradient(sx, sy, 0, sx, sy, starSize * 4);
        starGrad.addColorStop(0, `rgba(255, 250, 220, ${starAlpha})`);
        starGrad.addColorStop(0.25, `rgba(255, 210, 140, ${starAlpha * 0.6})`);
        starGrad.addColorStop(0.6, `rgba(255, 160, 80, ${starAlpha * 0.2})`);
        starGrad.addColorStop(1, 'rgba(255, 120, 50, 0)');
        ctx.fillStyle = starGrad;
        ctx.beginPath();
        ctx.arc(sx, sy, starSize * 4, 0, Math.PI * 2);
        ctx.fill();

        // Star core
        ctx.fillStyle = `rgba(255, 255, 250, ${starAlpha * 1.2})`;
        ctx.beginPath();
        ctx.arc(sx, sy, starSize, 0, Math.PI * 2);
        ctx.fill();
      }
    }

    // ========================================
    // VOICE PULSE WAVES
    // ========================================
    if (audio > 0.08) {
      for (let i = 0; i < 3; i++) {
        const pulsePhase = (t * 2.5 + i * 0.4) % 1;
        const pulseRadius = orbSize * (1 + pulsePhase * 2);
        const pulseAlpha = (1 - pulsePhase) * audio * 0.5 * alive;

        ctx.strokeStyle = `rgba(255, 200, 100, ${pulseAlpha})`;
        ctx.lineWidth = 2.5 * (1 - pulsePhase);
        ctx.beginPath();
        ctx.arc(cx, cy, pulseRadius, 0, Math.PI * 2);
        ctx.stroke();
      }
    }

    // ========================================
    // SURFACE FLARES (audio reactive)
    // ========================================
    if (audio > 0.15) {
      const numFlares = 5;
      for (let i = 0; i < numFlares; i++) {
        const flareAngle = (i / numFlares) * Math.PI * 2 + t * 0.3;
        const flareLen = orbSize * (0.3 + audio * 0.5) * (0.7 + Math.sin(t * 3 + i * 2) * 0.3);
        const flareBase = orbSize * 0.85;

        const fx1 = cx + Math.cos(flareAngle) * flareBase;
        const fy1 = cy + Math.sin(flareAngle) * flareBase;
        const fx2 = cx + Math.cos(flareAngle) * (flareBase + flareLen);
        const fy2 = cy + Math.sin(flareAngle) * (flareBase + flareLen);

        const flareGrad = ctx.createLinearGradient(fx1, fy1, fx2, fy2);
        flareGrad.addColorStop(0, `rgba(255, 200, 80, ${audio * 0.6 * alive})`);
        flareGrad.addColorStop(0.5, `rgba(255, 120, 40, ${audio * 0.3 * alive})`);
        flareGrad.addColorStop(1, 'rgba(255, 80, 20, 0)');

        ctx.strokeStyle = flareGrad;
        ctx.lineWidth = 3 + audio * 4;
        ctx.lineCap = 'round';
        ctx.beginPath();
        ctx.moveTo(fx1, fy1);
        ctx.lineTo(fx2, fy2);
        ctx.stroke();
      }
    }
  }

  // ========================================
  // DEATH POOF - RETURN TO VOID
  // ========================================
  if (isPoofing) {
    poofTime += 0.016;
    const poofDuration = 1.0;
    const p = poofTime / poofDuration;
    const ease = 1 - Math.pow(1 - p, 4);

    if (p < 1) {
      // Shockwave rings
      for (let i = 0; i < 4; i++) {
        const ringP = Math.max(0, (p - i * 0.08) / 0.85);
        if (ringP > 0 && ringP < 1) {
          const ringR = maxR * (0.08 + ringP * 0.85);
          const ringA = (1 - ringP) * 0.7;
          ctx.strokeStyle = `rgba(255, 220, 120, ${ringA})`;
          ctx.lineWidth = 4 * (1 - ringP);
          ctx.beginPath();
          ctx.arc(cx, cy, ringR, 0, Math.PI * 2);
          ctx.stroke();
        }
      }

      // Scatter particles
      for (let i = 0; i < 30; i++) {
        const angle = (i / 30) * Math.PI * 2 + Math.sin(i * 11) * 0.4;
        const speed = 0.5 + Math.sin(i * 17) * 0.5;
        const dist = maxR * ease * speed * 0.95;
        const px = cx + Math.cos(angle) * dist;
        const py = cy + Math.sin(angle) * dist;
        const pAlpha = (1 - ease) * 0.9;
        const pSize = (2 + Math.sin(i * 7) * 1.5) * (1 - ease);

        if (pAlpha > 0.05) {
          ctx.fillStyle = `rgba(255, ${200 + i * 2}, ${100 + i * 4}, ${pAlpha})`;
          ctx.beginPath();
          ctx.arc(px, py, pSize, 0, Math.PI * 2);
          ctx.fill();
        }
      }

      // Collapsing core flash
      const flashA = (1 - ease) * 0.8;
      const flashR = maxR * 0.25 * (1 - ease * 0.7);
      const flashGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, flashR);
      flashGrad.addColorStop(0, `rgba(255, 255, 240, ${flashA})`);
      flashGrad.addColorStop(0.4, `rgba(255, 200, 100, ${flashA * 0.6})`);
      flashGrad.addColorStop(1, 'rgba(255, 120, 40, 0)');
      ctx.fillStyle = flashGrad;
      ctx.beginPath();
      ctx.arc(cx, cy, flashR, 0, Math.PI * 2);
      ctx.fill();
    } else {
      isPoofing = false;
      targetGenesis = 0; // Return to void
    }
  }

  // ========================================
  // EDGE FADE (transparent at edges)
  // ========================================
  ctx.globalCompositeOperation = 'destination-in';
  const fadeGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, maxR * 0.98);
  fadeGrad.addColorStop(0, 'rgba(255,255,255,1)');
  fadeGrad.addColorStop(0.52, 'rgba(255,255,255,1)');
  fadeGrad.addColorStop(0.78, 'rgba(255,255,255,0.22)');
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

console.log('[Orb-Fire-V5] VOID AWAKENING - spark becomes sun');

window.addEventListener('resize', resize);
start();
