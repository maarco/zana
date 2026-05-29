/**
 * Fire Orb v6 - THE ULTIMATE FUSION
 *
 * ALL OF IT. EVERYTHING. COMBINED.
 *
 * From the void, a microscopic spark breathes.
 * When you speak, it EXPLODES into a blazing sun:
 * - Swirling fire clouds orbit the core
 * - 100 stars travel in 3D tilted orbits
 * - 5 rotating rings pulse with your voice
 * - Surface flares erupt from the corona
 * - Sparkle particles dance in the flames
 * - Voice pulses ripple outward
 * When done, it POOFS - shockwaves, scattered embers
 * Then slowly fades back to a sleeping spark.
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

// Genesis: 0 = void spark, 1 = blazing sun
let genesis = 0;
let targetGenesis = 0;

// Color blend: 0 = fire (recording), 1 = gold (processing)
let colorBlend = 0;

// Cloud angles for swirling nebula clouds
const cloudAngles = [0, 0, 0, 0, 0, 0, 0, 0, 0];
const NUM_CLOUDS = 9;

// Traveling stars with 3D orbits
const stars = [];
const NUM_STARS = 100;
for (let i = 0; i < NUM_STARS; i++) {
  stars.push({
    angle: Math.random() * Math.PI * 2,
    baseDist: 0.15 + Math.random() * 0.9,
    speed: 0.2 + Math.random() * 0.7,
    size: 0.2 + Math.random() * 1.0,  // Smaller stars
    twinkleSpeed: 1.2 + Math.random() * 4.5,
    twinkleOffset: Math.random() * Math.PI * 2,
    orbitTilt: (Math.random() - 0.5) * 0.4,
    layer: Math.floor(Math.random() * 3) // 0=inner, 1=mid, 2=outer
  });
}

// Orbital rings
const rings = [];
const NUM_RINGS = 5;
for (let i = 0; i < NUM_RINGS; i++) {
  rings.push({
    radius: 0.25 + i * 0.2,
    speed: 0.3 + i * 0.15,
    width: 2.5 - i * 0.35,
    opacity: 0.55 - i * 0.08,
    dashLength: 6 + i * 5,
    phase: Math.random() * Math.PI * 2
  });
}

// Sparkle particles (from nebula)
const sparkles = [];
const NUM_SPARKLES = 50;
for (let i = 0; i < NUM_SPARKLES; i++) {
  sparkles.push({
    angle: Math.random() * Math.PI * 2,
    dist: 0.1 + Math.random() * 0.8,
    speed: 0.05 + Math.random() * 0.15,
    size: 0.8 + Math.random() * 1.5,
    twinkleSpeed: 2 + Math.random() * 4,
    twinkleOffset: Math.random() * Math.PI * 2
  });
}

// Color palettes
const COLORS = {
  fire: {
    core: [255, 255, 250],
    inner: [255, 200, 100],
    mid: [255, 130, 40],
    outer: [255, 80, 20],
    corona: [200, 50, 10],
    clouds: [[255,120,40],[255,80,20],[255,160,60],[255,100,30],[255,140,50],[255,90,25],[255,130,45],[255,110,35],[255,150,55]],
    sparkle: [255, 240, 200],
    ring: [255, 180, 80],
    star: [255, 250, 220],
    flare: [255, 200, 80]
  },
  gold: {
    core: [255, 255, 255],
    inner: [255, 240, 150],
    mid: [255, 200, 80],
    outer: [255, 170, 50],
    corona: [220, 140, 30],
    clouds: [[255,200,80],[255,180,60],[255,220,100],[255,190,70],[255,210,90],[255,185,65],[255,205,85],[255,195,75],[255,215,95]],
    sparkle: [255, 255, 230],
    ring: [255, 220, 120],
    star: [255, 255, 240],
    flare: [255, 230, 120]
  },
  poof: {
    ring: [255, 220, 140],
    particle: [255, 200, 100],
    flash: [255, 240, 180]
  }
};

// Helper: blend between fire and gold colors
function lerpColor(fireColor, goldColor, t) {
  return [
    Math.round(fireColor[0] + (goldColor[0] - fireColor[0]) * t),
    Math.round(fireColor[1] + (goldColor[1] - fireColor[1]) * t),
    Math.round(fireColor[2] + (goldColor[2] - fireColor[2]) * t)
  ];
}

function getColor(name) {
  return lerpColor(COLORS.fire[name], COLORS.gold[name], colorBlend);
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
  colorBlend = 0;
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

  // Genesis: fast birth (0.055), slow death (0.015)
  const genSpeed = targetGenesis > genesis ? 0.055 : 0.015;
  genesis += (targetGenesis - genesis) * genSpeed;

  // Color blend: shift to gold when processing
  const targetBlend = isProcessing ? 1 : 0;
  colorBlend += (targetBlend - colorBlend) * 0.03;

  ctx.clearRect(0, 0, w, h);
  ctx.globalAlpha = globalOpacity;

  const breathe = Math.sin(t * 0.6) * 0.5 + 0.5;
  const audio = Math.min(audioLevel * 2.5, 1);
  const scale = 0.06 + genesis * 0.94;

  // ========================================
  // THE VOID SPARK (genesis ~ 0)
  // ========================================
  const sparkAlpha = Math.pow(1 - genesis, 1.5);
  if (sparkAlpha > 0.01) {
    const sparkSize = maxR * 0.008 * (1 + breathe * 0.35);  // Smaller spark

    // White-hot point
    const sparkGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, sparkSize * 6);
    sparkGrad.addColorStop(0, `rgba(255, 255, 255, ${sparkAlpha})`);
    sparkGrad.addColorStop(0.1, `rgba(255, 245, 210, ${sparkAlpha * 0.9})`);
    sparkGrad.addColorStop(0.3, `rgba(255, 190, 90, ${sparkAlpha * 0.6})`);
    sparkGrad.addColorStop(0.6, `rgba(255, 110, 35, ${sparkAlpha * 0.25})`);
    sparkGrad.addColorStop(1, 'rgba(180, 50, 10, 0)');
    ctx.fillStyle = sparkGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, sparkSize * 6, 0, Math.PI * 2);
    ctx.fill();

    // Breathing glow in the void
    const voidPulse = maxR * 0.035 * (1 + breathe * 0.5);  // Smaller glow
    const voidGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, voidPulse);
    voidGrad.addColorStop(0, `rgba(255, 130, 50, ${sparkAlpha * 0.25})`);
    voidGrad.addColorStop(0.6, `rgba(200, 70, 25, ${sparkAlpha * 0.1})`);
    voidGrad.addColorStop(1, 'rgba(150, 40, 15, 0)');
    ctx.fillStyle = voidGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, voidPulse, 0, Math.PI * 2);
    ctx.fill();
  }

  // ========================================
  // THE BLAZING SUN (genesis ~ 1)
  // ========================================
  if (genesis > 0.01) {
    const alive = genesis;
    const baseSize = maxR * 0.13;  // Half the size at rest
    const orbSize = baseSize * scale * (1 + audio * 1.8 + breathe * 0.04);  // Expands more with voice

    // ========================================
    // SWIRLING FIRE CLOUDS (from nebula)
    // ========================================
    const cloudColors = COLORS.fire.clouds.map((fc, i) =>
      lerpColor(fc, COLORS.gold.clouds[i], colorBlend)
    );

    for (let i = 0; i < NUM_CLOUDS; i++) {
      const [r, g, b] = cloudColors[i % cloudColors.length];
      const baseAngle = (i * Math.PI * 2 / NUM_CLOUDS);
      const audioBoost = 1 + audio * 4;
      const orbitSpeed = (0.25 + (i % 3) * 0.15) * audioBoost * 0.016;
      cloudAngles[i] += orbitSpeed;
      const angle = baseAngle + cloudAngles[i];

      const cloudDist = orbSize * (0.6 + Math.sin(t * 0.7 + i) * 0.25 + audio * 0.3);
      const cloudSize = orbSize * (0.35 + audio * 0.25) * alive;

      const cloudX = cx + Math.cos(angle) * cloudDist;
      const cloudY = cy + Math.sin(angle) * cloudDist;

      const cloudGrad = ctx.createRadialGradient(cloudX, cloudY, 0, cloudX, cloudY, cloudSize);
      const cloudAlpha = (0.35 + audio * 0.35) * alive;
      cloudGrad.addColorStop(0, `rgba(${r}, ${g}, ${b}, ${cloudAlpha})`);
      cloudGrad.addColorStop(0.35, `rgba(${r}, ${g}, ${b}, ${cloudAlpha * 0.55})`);
      cloudGrad.addColorStop(0.7, `rgba(${r}, ${g}, ${b}, ${cloudAlpha * 0.2})`);
      cloudGrad.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0)`);

      ctx.fillStyle = cloudGrad;
      ctx.beginPath();
      ctx.arc(cloudX, cloudY, cloudSize, 0, Math.PI * 2);
      ctx.fill();
    }

    // ========================================
    // MULTI-LAYER MOLTEN CORE
    // ========================================
    const coreCol = getColor('core');
    const innerCol = getColor('inner');
    const midCol = getColor('mid');
    const outerCol = getColor('outer');
    const coronaCol = getColor('corona');

    const coreGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, orbSize);
    coreGrad.addColorStop(0, `rgba(${coreCol[0]}, ${coreCol[1]}, ${coreCol[2]}, ${alive})`);
    coreGrad.addColorStop(0.1, `rgba(${innerCol[0]}, ${innerCol[1]}, ${innerCol[2]}, ${alive * 0.97})`);
    coreGrad.addColorStop(0.28, `rgba(${midCol[0]}, ${midCol[1]}, ${midCol[2]}, ${alive * 0.85})`);
    coreGrad.addColorStop(0.5, `rgba(${outerCol[0]}, ${outerCol[1]}, ${outerCol[2]}, ${alive * 0.6})`);
    coreGrad.addColorStop(0.72, `rgba(${coronaCol[0]}, ${coronaCol[1]}, ${coronaCol[2]}, ${alive * 0.3})`);
    coreGrad.addColorStop(1, 'rgba(150, 30, 5, 0)');
    ctx.fillStyle = coreGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, orbSize, 0, Math.PI * 2);
    ctx.fill();

    // Outer corona glow
    const coronaSize = orbSize * 2.4;
    const coronaGrad = ctx.createRadialGradient(cx, cy, orbSize * 0.5, cx, cy, coronaSize);
    coronaGrad.addColorStop(0, `rgba(${midCol[0]}, ${midCol[1]}, ${midCol[2]}, ${alive * 0.3})`);
    coronaGrad.addColorStop(0.35, `rgba(${outerCol[0]}, ${outerCol[1]}, ${outerCol[2]}, ${alive * 0.15})`);
    coronaGrad.addColorStop(0.7, `rgba(${coronaCol[0]}, ${coronaCol[1]}, ${coronaCol[2]}, ${alive * 0.06})`);
    coronaGrad.addColorStop(1, 'rgba(100, 30, 10, 0)');
    ctx.fillStyle = coronaGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, coronaSize, 0, Math.PI * 2);
    ctx.fill();

    // ========================================
    // ROTATING RINGS
    // ========================================
    const ringCol = getColor('ring');
    for (let i = 0; i < rings.length; i++) {
      const ring = rings[i];
      const ringRadius = orbSize * (1.25 + ring.radius * (0.7 + audio * 0.5));
      const ringAlpha = ring.opacity * alive * (0.35 + audio * 0.65) * scale;

      if (ringAlpha > 0.015) {
        ctx.save();
        ctx.translate(cx, cy);
        ctx.rotate(t * ring.speed * (i % 2 === 0 ? 1 : -1) + ring.phase);

        ctx.setLineDash([ring.dashLength, ring.dashLength * 1.4]);
        const ringR = Math.max(0, ringCol[0] - i * 15);
        const ringG = Math.max(0, ringCol[1] - i * 20);
        const ringB = Math.max(0, ringCol[2] - i * 10);
        ctx.strokeStyle = `rgba(${ringR}, ${ringG}, ${ringB}, ${ringAlpha})`;
        ctx.lineWidth = ring.width * (1 + audio * 0.9) * scale;
        ctx.beginPath();
        ctx.arc(0, 0, ringRadius, 0, Math.PI * 2);
        ctx.stroke();
        ctx.setLineDash([]);

        ctx.restore();
      }
    }

    // ========================================
    // ORBITING STARS (3D tilt)
    // ========================================
    const starCol = getColor('star');
    for (let i = 0; i < stars.length; i++) {
      const star = stars[i];

      star.angle += star.speed * 0.016 * (0.7 + audio * 3);

      // Layer-based distance
      const layerMult = 1 + star.layer * 0.4;
      const starDist = orbSize * (1.3 + star.baseDist * 1.1) * (0.25 + genesis * 0.75) * layerMult;

      // 3D orbit tilt
      const tiltedY = Math.sin(star.angle) * (1 + star.orbitTilt);
      const sx = cx + Math.cos(star.angle) * starDist;
      const sy = cy + tiltedY * starDist * 0.85;

      // Twinkle
      const twinkle = Math.sin(t * star.twinkleSpeed + star.twinkleOffset);
      const starAlpha = (0.2 + twinkle * 0.55) * alive * (0.4 + audio * 0.6) * scale;
      const starSize = star.size * (1 + audio * 0.7 + twinkle * 0.3) * (0.4 + genesis * 0.6);

      if (starAlpha > 0.015 && starSize > 0.25) {
        // Star glow
        const starGrad = ctx.createRadialGradient(sx, sy, 0, sx, sy, starSize * 3);
        starGrad.addColorStop(0, `rgba(${starCol[0]}, ${starCol[1]}, ${starCol[2]}, ${starAlpha})`);
        starGrad.addColorStop(0.2, `rgba(${starCol[0]}, ${Math.max(0, starCol[1]-30)}, ${Math.max(0, starCol[2]-60)}, ${starAlpha * 0.55})`);
        starGrad.addColorStop(0.5, `rgba(${starCol[0]}, ${Math.max(0, starCol[1]-60)}, ${Math.max(0, starCol[2]-100)}, ${starAlpha * 0.2})`);
        starGrad.addColorStop(1, 'rgba(255, 150, 60, 0)');
        ctx.fillStyle = starGrad;
        ctx.beginPath();
        ctx.arc(sx, sy, starSize * 3, 0, Math.PI * 2);
        ctx.fill();

        // Star core
        ctx.fillStyle = `rgba(255, 255, 250, ${starAlpha * 1.3})`;
        ctx.beginPath();
        ctx.arc(sx, sy, starSize, 0, Math.PI * 2);
        ctx.fill();
      }
    }

    // ========================================
    // SPARKLE PARTICLES (from nebula)
    // ========================================
    const sparkleCol = getColor('sparkle');
    for (let i = 0; i < sparkles.length; i++) {
      const sp = sparkles[i];
      sp.angle += sp.speed * 0.016 * (1 + audio * 2);

      const spDist = orbSize * (0.8 + sp.dist * 1.5) * alive;
      const spX = cx + Math.cos(sp.angle) * spDist;
      const spY = cy + Math.sin(sp.angle) * spDist;

      const twinkle = Math.sin(t * sp.twinkleSpeed + sp.twinkleOffset);
      const spAlpha = (0.15 + twinkle * 0.45) * alive * (0.5 + audio * 0.5);
      const spSize = sp.size * (1 + twinkle * 0.3);

      if (spAlpha > 0.02) {
        ctx.fillStyle = `rgba(${sparkleCol[0]}, ${sparkleCol[1]}, ${sparkleCol[2]}, ${spAlpha})`;
        ctx.beginPath();
        ctx.arc(spX, spY, spSize, 0, Math.PI * 2);
        ctx.fill();
      }
    }

    // ========================================
    // VOICE PULSE WAVES
    // ========================================
    if (audio > 0.06) {
      for (let i = 0; i < 4; i++) {
        const pulsePhase = (t * 2.2 + i * 0.35) % 1;
        const pulseRadius = orbSize * (1 + pulsePhase * 2.2);
        const pulseAlpha = (1 - pulsePhase) * audio * 0.45 * alive;

        const pCol = getColor('ring');
        ctx.strokeStyle = `rgba(${pCol[0]}, ${pCol[1]}, ${pCol[2]}, ${pulseAlpha})`;
        ctx.lineWidth = 2.2 * (1 - pulsePhase);
        ctx.beginPath();
        ctx.arc(cx, cy, pulseRadius, 0, Math.PI * 2);
        ctx.stroke();
      }
    }

    // ========================================
    // SURFACE FLARES
    // ========================================
    if (audio > 0.12) {
      const flareCol = getColor('flare');
      const numFlares = 6;
      for (let i = 0; i < numFlares; i++) {
        const flareAngle = (i / numFlares) * Math.PI * 2 + t * 0.25;
        const flareLen = orbSize * (0.25 + audio * 0.6) * (0.6 + Math.sin(t * 3.5 + i * 2.3) * 0.4);
        const flareBase = orbSize * 0.88;

        const fx1 = cx + Math.cos(flareAngle) * flareBase;
        const fy1 = cy + Math.sin(flareAngle) * flareBase;
        const fx2 = cx + Math.cos(flareAngle) * (flareBase + flareLen);
        const fy2 = cy + Math.sin(flareAngle) * (flareBase + flareLen);

        const flareGrad = ctx.createLinearGradient(fx1, fy1, fx2, fy2);
        flareGrad.addColorStop(0, `rgba(${flareCol[0]}, ${flareCol[1]}, ${flareCol[2]}, ${audio * 0.65 * alive})`);
        flareGrad.addColorStop(0.5, `rgba(${flareCol[0]}, ${Math.max(0, flareCol[1]-40)}, ${Math.max(0, flareCol[2]-30)}, ${audio * 0.3 * alive})`);
        flareGrad.addColorStop(1, 'rgba(255, 100, 30, 0)');

        ctx.strokeStyle = flareGrad;
        ctx.lineWidth = 2.5 + audio * 4.5;
        ctx.lineCap = 'round';
        ctx.beginPath();
        ctx.moveTo(fx1, fy1);
        ctx.lineTo(fx2, fy2);
        ctx.stroke();
      }
    }
  }

  // ========================================
  // POOF EXPLOSION
  // ========================================
  if (isPoofing) {
    poofTime += 0.016;
    const poofDuration = 1.1;
    const p = poofTime / poofDuration;
    const ease = 1 - Math.pow(1 - p, 4);

    if (p < 1) {
      const poofRing = COLORS.poof.ring;
      const poofPart = COLORS.poof.particle;
      const poofFlash = COLORS.poof.flash;

      // Shockwave rings
      for (let i = 0; i < 5; i++) {
        const ringP = Math.max(0, (p - i * 0.06) / 0.82);
        if (ringP > 0 && ringP < 1) {
          const ringR = maxR * (0.06 + ringP * 0.88);
          const ringA = (1 - ringP) * 0.65;
          ctx.strokeStyle = `rgba(${poofRing[0]}, ${poofRing[1]}, ${poofRing[2]}, ${ringA})`;
          ctx.lineWidth = 4.5 * (1 - ringP);
          ctx.beginPath();
          ctx.arc(cx, cy, ringR, 0, Math.PI * 2);
          ctx.stroke();
        }
      }

      // Scattered embers
      for (let i = 0; i < 40; i++) {
        const angle = (i / 40) * Math.PI * 2 + Math.sin(i * 13) * 0.45;
        const speed = 0.45 + Math.sin(i * 19) * 0.55;
        const dist = maxR * ease * speed * 0.92;
        const px = cx + Math.cos(angle) * dist;
        const py = cy + Math.sin(angle) * dist;
        const pAlpha = (1 - ease) * 0.85;
        const pSize = (2.5 + Math.sin(i * 7) * 1.8) * (1 - ease);

        if (pAlpha > 0.04) {
          const pr = Math.min(255, poofPart[0] + i * 1.5);
          const pg = Math.min(255, poofPart[1] + i * 2);
          const pb = Math.min(255, poofPart[2] + i * 3);
          ctx.fillStyle = `rgba(${pr}, ${pg}, ${pb}, ${pAlpha})`;
          ctx.beginPath();
          ctx.arc(px, py, pSize, 0, Math.PI * 2);
          ctx.fill();
        }
      }

      // Central flash
      const flashA = (1 - ease) * 0.75;
      const flashR = maxR * 0.28 * (1 - ease * 0.65);
      const flashGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, flashR);
      flashGrad.addColorStop(0, `rgba(255, 255, 250, ${flashA})`);
      flashGrad.addColorStop(0.35, `rgba(${poofFlash[0]}, ${poofFlash[1]}, ${poofFlash[2]}, ${flashA * 0.6})`);
      flashGrad.addColorStop(1, 'rgba(255, 140, 50, 0)');
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

console.log('[Orb-Fire-V6] THE ULTIMATE FUSION - All of it. Everything. Combined.');

window.addEventListener('resize', resize);
start();
