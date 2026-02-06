/**
 * Fire Orb v7 - SINGULARITY
 *
 * Based on state-of-the-art black hole visualizations with:
 * - Gravitational lensing (light bending around the center)
 * - Accretion disk (swirling matter with doppler shift)
 * - Photon sphere (ring of trapped light)
 * - Event horizon (the void within)
 *
 * When dormant: a cold singularity, barely visible
 * When speaking: the accretion disk ignites, matter swirls
 *
 * Inspired by:
 * - Interstellar's Gargantua
 * - Shadertoy black hole shaders
 * - Real relativistic rendering techniques
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

// Genesis: 0 = cold singularity, 1 = active black hole
let genesis = 0;
let targetGenesis = 0;

// Disk rotation angle (continuous)
let diskRotation = 0;

// Lensed background stars
const lensedStars = [];
const NUM_LENSED_STARS = 150;
for (let i = 0; i < NUM_LENSED_STARS; i++) {
  const angle = Math.random() * Math.PI * 2;
  const dist = 0.3 + Math.random() * 0.7;
  lensedStars.push({
    baseAngle: angle,
    baseDist: dist,
    size: 0.3 + Math.random() * 1.2,
    brightness: 0.3 + Math.random() * 0.7,
    twinkleSpeed: 1 + Math.random() * 3,
    twinkleOffset: Math.random() * Math.PI * 2
  });
}

// Accretion disk particles
const diskParticles = [];
const NUM_DISK_PARTICLES = 200;
for (let i = 0; i < NUM_DISK_PARTICLES; i++) {
  const angle = Math.random() * Math.PI * 2;
  const dist = 0.35 + Math.random() * 0.5; // Between inner and outer radius
  diskParticles.push({
    angle: angle,
    dist: dist,
    speed: 1.5 / (dist * dist), // Kepler: closer = faster
    size: 0.5 + Math.random() * 1.5,
    brightness: 0.4 + Math.random() * 0.6,
    layer: Math.random() // For depth sorting
  });
}

// Photon ring particles (trapped light orbiting at photon sphere)
const photonRing = [];
const NUM_PHOTON_PARTICLES = 80;
for (let i = 0; i < NUM_PHOTON_PARTICLES; i++) {
  photonRing.push({
    angle: (i / NUM_PHOTON_PARTICLES) * Math.PI * 2,
    speed: 2.5 + Math.random() * 0.5,
    brightness: 0.5 + Math.random() * 0.5,
    size: 0.4 + Math.random() * 0.8
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
  diskRotation = 0;
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

// Gravitational lensing: displaces point toward/around center
function lensPoint(x, y, cx, cy, strength) {
  const dx = x - cx;
  const dy = y - cy;
  const dist = Math.sqrt(dx * dx + dy * dy);
  if (dist < 0.001) return { x, y };

  // Einstein ring effect: light bends around the mass
  const bendFactor = strength / (dist + strength * 0.5);
  const angle = Math.atan2(dy, dx);
  const newDist = dist + bendFactor * 0.3;

  return {
    x: cx + Math.cos(angle) * newDist,
    y: cy + Math.sin(angle) * newDist
  };
}

// Doppler shift color based on velocity toward/away from viewer
function dopplerColor(baseColor, angle, intensity) {
  // Objects moving toward us (right side of disk) are blue-shifted
  // Objects moving away (left side) are red-shifted
  const dopplerFactor = Math.cos(angle) * intensity;

  const r = Math.min(255, Math.max(0, baseColor[0] + dopplerFactor * -30));
  const g = Math.min(255, Math.max(0, baseColor[1] + dopplerFactor * -15));
  const b = Math.min(255, Math.max(0, baseColor[2] + dopplerFactor * 40));

  // Relativistic beaming: approaching side is brighter
  const beaming = 1 + dopplerFactor * 0.5;

  return {
    color: [r, g, b],
    brightness: beaming
  };
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
  audioLevel += (targetAudioLevel - audioLevel) * 0.2;

  // Genesis: fast ignition, slow cooling
  const genSpeed = targetGenesis > genesis ? 0.045 : 0.018;
  genesis += (targetGenesis - genesis) * genSpeed;

  // Disk rotation - faster with audio
  diskRotation += 0.008 * (1 + audioLevel * 3);

  ctx.clearRect(0, 0, w, h);
  ctx.globalAlpha = globalOpacity;

  const audio = Math.min(audioLevel * 2.5, 1);

  // Sizing
  const eventHorizonRadius = maxR * 0.06 * (0.8 + genesis * 0.2);
  const photonSphereRadius = maxR * 0.09 * (0.8 + genesis * 0.2);
  const diskInnerRadius = maxR * 0.12 * (0.7 + genesis * 0.3);
  const diskOuterRadius = maxR * 0.35 * (0.5 + genesis * 0.5 + audio * 0.3);
  const lensStrength = maxR * 0.15 * genesis;

  // ========================================
  // GRAVITATIONALLY LENSED BACKGROUND STARS
  // ========================================
  for (let i = 0; i < lensedStars.length; i++) {
    const star = lensedStars[i];

    // Original position
    const origX = cx + Math.cos(star.baseAngle) * star.baseDist * maxR;
    const origY = cy + Math.sin(star.baseAngle) * star.baseDist * maxR;

    // Apply gravitational lensing
    const lensed = lensPoint(origX, origY, cx, cy, lensStrength);

    // Check if behind event horizon
    const distFromCenter = Math.sqrt((lensed.x - cx) ** 2 + (lensed.y - cy) ** 2);
    if (distFromCenter < eventHorizonRadius) continue;

    // Twinkle
    const twinkle = Math.sin(t * star.twinkleSpeed + star.twinkleOffset) * 0.5 + 0.5;
    const alpha = star.brightness * (0.4 + twinkle * 0.6) * (0.5 + genesis * 0.5);
    const size = star.size * (0.8 + twinkle * 0.4);

    if (alpha > 0.05) {
      // Lensed stars get stretched/distorted near the hole
      const distortion = 1 + (lensStrength / (distFromCenter + 1)) * 0.5;

      ctx.fillStyle = `rgba(200, 220, 255, ${alpha})`;
      ctx.beginPath();
      ctx.arc(lensed.x, lensed.y, size * distortion, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  // ========================================
  // ACCRETION DISK (behind the hole)
  // ========================================
  // Draw back half first (particles with layer < 0.5 and y > cy after rotation)

  const diskAlpha = genesis * (0.4 + audio * 0.6);
  if (diskAlpha > 0.02) {
    // Sort particles by depth for proper layering
    const sortedParticles = [...diskParticles].sort((a, b) => {
      const aY = Math.sin(a.angle + diskRotation);
      const bY = Math.sin(b.angle + diskRotation);
      return aY - bY; // Draw far particles first
    });

    for (let i = 0; i < sortedParticles.length; i++) {
      const p = sortedParticles[i];

      // Update angle based on orbital speed
      p.angle += p.speed * 0.016 * (1 + audio * 2);

      const angle = p.angle + diskRotation;

      // 3D projection of tilted disk (viewed at ~75 degrees)
      const diskTilt = 0.25; // How edge-on the disk appears
      const x3d = Math.cos(angle) * p.dist;
      const y3d = Math.sin(angle) * p.dist * diskTilt;

      const px = cx + x3d * diskOuterRadius;
      const py = cy + y3d * diskOuterRadius;

      // Skip if inside event horizon projection
      const distFromCenter = Math.sqrt((px - cx) ** 2 + (py - cy) ** 2);
      if (distFromCenter < eventHorizonRadius * 0.9) continue;

      // Doppler shift based on orbital velocity direction
      const baseColor = [255, 180, 100];
      const doppler = dopplerColor(baseColor, angle, 0.7 * genesis);

      // Particles in front of hole are brighter
      const depthFactor = Math.sin(angle) > 0 ? 1.2 : 0.7;
      const alpha = p.brightness * diskAlpha * doppler.brightness * depthFactor;
      const size = p.size * (1 + audio * 0.5);

      if (alpha > 0.03) {
        const c = doppler.color;
        const grad = ctx.createRadialGradient(px, py, 0, px, py, size * 3);
        grad.addColorStop(0, `rgba(${c[0]}, ${c[1]}, ${c[2]}, ${alpha})`);
        grad.addColorStop(0.4, `rgba(${c[0]}, ${c[1] * 0.8}, ${c[2] * 0.6}, ${alpha * 0.5})`);
        grad.addColorStop(1, `rgba(${c[0] * 0.7}, ${c[1] * 0.5}, ${c[2] * 0.3}, 0)`);

        ctx.fillStyle = grad;
        ctx.beginPath();
        ctx.arc(px, py, size * 3, 0, Math.PI * 2);
        ctx.fill();
      }
    }

    // Continuous disk glow (ellipse)
    ctx.save();
    ctx.translate(cx, cy);

    const diskGrad = ctx.createRadialGradient(0, 0, diskInnerRadius, 0, 0, diskOuterRadius);
    diskGrad.addColorStop(0, `rgba(255, 200, 120, ${diskAlpha * 0.3})`);
    diskGrad.addColorStop(0.3, `rgba(255, 150, 80, ${diskAlpha * 0.4})`);
    diskGrad.addColorStop(0.6, `rgba(255, 100, 50, ${diskAlpha * 0.25})`);
    diskGrad.addColorStop(1, 'rgba(200, 60, 30, 0)');

    ctx.scale(1, 0.25); // Flatten to ellipse
    ctx.fillStyle = diskGrad;
    ctx.beginPath();
    ctx.arc(0, 0, diskOuterRadius, 0, Math.PI * 2);
    ctx.fill();

    ctx.restore();
  }

  // ========================================
  // PHOTON SPHERE (ring of light at 1.5x event horizon)
  // ========================================
  const photonAlpha = genesis * (0.5 + audio * 0.5);
  if (photonAlpha > 0.03) {
    for (let i = 0; i < photonRing.length; i++) {
      const p = photonRing[i];
      p.angle += p.speed * 0.016 * (1 + audio);

      const px = cx + Math.cos(p.angle) * photonSphereRadius;
      const py = cy + Math.sin(p.angle) * photonSphereRadius * 0.3; // Tilted view

      const alpha = p.brightness * photonAlpha * (0.6 + Math.sin(t * 5 + i) * 0.4);

      if (alpha > 0.02) {
        ctx.fillStyle = `rgba(255, 250, 230, ${alpha})`;
        ctx.beginPath();
        ctx.arc(px, py, p.size, 0, Math.PI * 2);
        ctx.fill();
      }
    }

    // Photon ring glow
    ctx.save();
    ctx.translate(cx, cy);
    ctx.scale(1, 0.3);

    ctx.strokeStyle = `rgba(255, 240, 200, ${photonAlpha * 0.4})`;
    ctx.lineWidth = 2 + audio * 2;
    ctx.beginPath();
    ctx.arc(0, 0, photonSphereRadius, 0, Math.PI * 2);
    ctx.stroke();

    ctx.restore();
  }

  // ========================================
  // DORMANT STATE - warm ember (visible when genesis is low)
  // ========================================
  const dormantAlpha = Math.pow(1 - genesis, 1.5);
  if (dormantAlpha > 0.01) {
    const breathe = Math.sin(t * 0.5) * 0.5 + 0.5;

    // Warm ember core
    const emberSize = maxR * 0.025 * (1 + breathe * 0.3);
    const emberGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, emberSize * 5);
    emberGrad.addColorStop(0, `rgba(255, 200, 150, ${dormantAlpha})`);
    emberGrad.addColorStop(0.15, `rgba(255, 150, 100, ${dormantAlpha * 0.85})`);
    emberGrad.addColorStop(0.4, `rgba(255, 100, 60, ${dormantAlpha * 0.5})`);
    emberGrad.addColorStop(0.7, `rgba(200, 60, 30, ${dormantAlpha * 0.2})`);
    emberGrad.addColorStop(1, 'rgba(100, 30, 15, 0)');
    ctx.fillStyle = emberGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, emberSize * 5, 0, Math.PI * 2);
    ctx.fill();

    // Breathing outer glow
    const glowSize = maxR * 0.08 * (1 + breathe * 0.4);
    const glowGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, glowSize);
    glowGrad.addColorStop(0, `rgba(255, 120, 60, ${dormantAlpha * 0.35})`);
    glowGrad.addColorStop(0.5, `rgba(200, 80, 40, ${dormantAlpha * 0.15})`);
    glowGrad.addColorStop(1, 'rgba(150, 50, 25, 0)');
    ctx.fillStyle = glowGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, glowSize, 0, Math.PI * 2);
    ctx.fill();
  }

  // ========================================
  // EVENT HORIZON (the void) - only when active
  // ========================================
  if (genesis > 0.1) {
    const holeAlpha = genesis;
    const holeGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, eventHorizonRadius * 1.3);
    holeGrad.addColorStop(0, `rgba(0, 0, 0, ${holeAlpha})`);
    holeGrad.addColorStop(0.7, `rgba(0, 0, 0, ${holeAlpha})`);
    holeGrad.addColorStop(0.85, `rgba(0, 0, 0, ${holeAlpha * 0.8})`);
    holeGrad.addColorStop(1, 'rgba(0, 0, 0, 0)');

    ctx.fillStyle = holeGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, eventHorizonRadius * 1.3, 0, Math.PI * 2);
    ctx.fill();

    // Inner glow (frame dragging / ergosphere)
    const ergoAlpha = genesis * 0.4 * (0.5 + audio * 0.5);
    const ergoGrad = ctx.createRadialGradient(cx, cy, eventHorizonRadius * 0.7, cx, cy, eventHorizonRadius * 1.6);
    ergoGrad.addColorStop(0, 'rgba(0, 0, 0, 0)');
    ergoGrad.addColorStop(0.4, `rgba(150, 80, 30, ${ergoAlpha * 0.4})`);
    ergoGrad.addColorStop(0.75, `rgba(255, 180, 100, ${ergoAlpha})`);
    ergoGrad.addColorStop(1, 'rgba(255, 120, 60, 0)');

    ctx.fillStyle = ergoGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, eventHorizonRadius * 1.6, 0, Math.PI * 2);
    ctx.fill();
  }

  // ========================================
  // HAWKING RADIATION (subtle particle emission)
  // ========================================
  if (genesis > 0.3 && audio > 0.1) {
    const numHawking = Math.floor(audio * 15);
    for (let i = 0; i < numHawking; i++) {
      const angle = (t * 2 + i * 0.7) % (Math.PI * 2);
      const dist = eventHorizonRadius * (1.1 + (t * 0.5 + i * 0.3) % 1 * 2);

      const hx = cx + Math.cos(angle) * dist;
      const hy = cy + Math.sin(angle) * dist;

      const hAlpha = (1 - ((t * 0.5 + i * 0.3) % 1)) * audio * genesis * 0.5;

      if (hAlpha > 0.02 && dist < maxR * 0.5) {
        ctx.fillStyle = `rgba(200, 220, 255, ${hAlpha})`;
        ctx.beginPath();
        ctx.arc(hx, hy, 1 + audio, 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  // ========================================
  // POOF - Hawking evaporation burst
  // ========================================
  if (isPoofing) {
    poofTime += 0.016;
    const poofDuration = 1.0;
    const p = poofTime / poofDuration;
    const ease = 1 - Math.pow(1 - p, 3);

    if (p < 1) {
      // Evaporation rings (opposite of accretion)
      for (let i = 0; i < 4; i++) {
        const ringP = Math.max(0, (p - i * 0.08) / 0.8);
        if (ringP > 0 && ringP < 1) {
          const ringR = eventHorizonRadius * (1 + ringP * 8);
          const ringA = (1 - ringP) * 0.7;

          ctx.strokeStyle = `rgba(150, 200, 255, ${ringA})`;
          ctx.lineWidth = 3 * (1 - ringP);
          ctx.beginPath();
          ctx.arc(cx, cy, ringR, 0, Math.PI * 2);
          ctx.stroke();
        }
      }

      // Escaping particles (matter finally free)
      for (let i = 0; i < 50; i++) {
        const angle = (i / 50) * Math.PI * 2 + Math.sin(i * 11) * 0.3;
        const speed = 0.3 + Math.sin(i * 17) * 0.7;
        const dist = maxR * ease * speed * 0.8;
        const px = cx + Math.cos(angle) * dist;
        const py = cy + Math.sin(angle) * dist;
        const pAlpha = (1 - ease) * 0.8;
        const pSize = (1.5 + Math.sin(i * 7)) * (1 - ease);

        if (pAlpha > 0.03) {
          // Blue-shifted escaping radiation
          ctx.fillStyle = `rgba(${180 + i}, ${200 + i * 0.5}, 255, ${pAlpha})`;
          ctx.beginPath();
          ctx.arc(px, py, pSize, 0, Math.PI * 2);
          ctx.fill();
        }
      }

      // Final flash
      const flashA = (1 - ease) * 0.6;
      const flashR = eventHorizonRadius * (2 - ease);
      const flashGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, flashR);
      flashGrad.addColorStop(0, `rgba(255, 255, 255, ${flashA})`);
      flashGrad.addColorStop(0.4, `rgba(200, 230, 255, ${flashA * 0.5})`);
      flashGrad.addColorStop(1, 'rgba(150, 180, 255, 0)');
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
  // EDGE FADE
  // ========================================
  ctx.globalCompositeOperation = 'destination-in';
  const fadeGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, maxR);
  fadeGrad.addColorStop(0, 'rgba(255,255,255,1)');
  fadeGrad.addColorStop(0.55, 'rgba(255,255,255,1)');
  fadeGrad.addColorStop(0.8, 'rgba(255,255,255,0.6)');
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

console.log('[Orb-Fire-V7] SINGULARITY - Black hole with gravitational lensing & accretion disk');

window.addEventListener('resize', resize);
start();
