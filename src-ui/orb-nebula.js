/**
 * Nebula Orb - Purple/Cyan Canvas2D Implementation
 * Extracted from orb.html for easier editing
 *
 * This is the original purple nebula orb visualization.
 */

const canvas = document.getElementById('canvas');
const ctx = canvas.getContext('2d', { alpha: true });
const statusEl = document.getElementById('status');

// Default config (matches orb_config.json)
let config = {
  animation: { fadeInSpeed: 0.08, fadeOutSpeed: 0.27, colorBlendSpeed: 0.02, audioSmoothingFactor: 0.2, poofDuration: 1.2 },
  audio: { baselineLevel: 0.2, levelMultiplier: 1.4, levelCap: 24 },
  visuals: {
    cloudCount: 7, particleMultiplier: 40, processingTimeScale: 4.5, cloudOrbitAudioBoost: 3,
    colors: {
      normal: { clouds: [[200,80,255],[140,60,220],[255,100,200],[100,80,255],[180,40,200],[255,100,200],[100,80,255]], core: [180,100,255], sparkle: [255,255,255] },
      processing: { clouds: [[80,200,255],[60,160,220],[100,220,255],[40,140,200],[80,180,240],[100,220,255],[40,140,200]], core: [100,180,255], sparkle: [200,255,255] },
      poof: { ring: [200,255,230], particle: [180,255,220], flash: [200,255,230] }
    }
  },
  poof: { ringCount: 1, particleCount: 3, ringDelay: 0.05 },
  window: { size: 300, panelLevel: 1000, fadeOutDelay: 1000, animationCompleteDelay: 3600 }
};

// Load config from JSON file when the app serves assets. File previews cannot
// fetch sibling files, so they keep the bundled defaults.
if (window.location.protocol !== 'file:') {
  fetch('orb_config.json')
    .then(r => r.json())
    .then(c => { config = c; console.log('[Orb] Config loaded'); })
    .catch(e => console.warn('[Orb] Using default config:', e));
}

// Hot reload: Rust pushes config updates via this function
window.updateConfig = function(jsonStr) {
  try {
    config = JSON.parse(jsonStr);
    console.log('[Orb] Config hot reloaded');
  } catch (e) {
    console.warn('[Orb] Invalid config update:', e);
  }
};

let time = 0;
let audioLevel = 0;
let targetAudioLevel = 0;
let cloudAngles = [0, 0, 0, 0, 0, 0, 0];
let isRecording = false;
let isProcessing = false;
let animationId = null;
let poofTime = 0;
let isPoofing = false;
let colorBlend = 0;
let globalOpacity = 0;
let targetOpacity = 1;
let isFadingOut = false;

// Global functions for Rust to call directly via eval()
window.setAudioLevel = function(level, peak) {
  targetAudioLevel = level;
  if (level > 0.01) {
    console.log('[Orb] setAudioLevel called: level=' + level.toFixed(4));
  }
};

window.setRecordingState = function(recording, processing) {
  console.log('[Orb] setRecordingState: recording=' + recording + ', processing=' + processing);
  isRecording = recording;
  isProcessing = processing;
  if (recording) {
    targetAudioLevel = 0;
    audioLevel = 0;
  } else if (processing) {
    targetAudioLevel = 0;
  } else {
    targetAudioLevel = 0;
  }
  updateStatus('');
};

window.setTranscriptionComplete = function(text) {
  console.log('[Orb] Transcription complete - poof!');
  isProcessing = false;
  targetAudioLevel = 0;
  isPoofing = true;
  poofTime = 0;
  updateStatus('');
};

window.fadeIn = function() {
  console.log('[Orb] Fading in');
  targetOpacity = 1;
  isFadingOut = false;
};

window.fadeOut = function() {
  console.log('[Orb] Fading out');
  targetOpacity = 0;
  isFadingOut = true;
};

window.isFadeComplete = function() {
  return isFadingOut && globalOpacity < 0.01;
};

window.resetOrb = function() {
  console.log('[Orb] Resetting orb state');
  globalOpacity = 0;
  targetOpacity = 1;
  isFadingOut = false;
  isRecording = false;
  isProcessing = false;
  isPoofing = false;
  poofTime = 0;
  colorBlend = 0;
  audioLevel = 0;
  targetAudioLevel = 0;
  cloudAngles = [0, 0, 0, 0, 0, 0, 0];
};

function resize() {
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
  ctx.scale(dpr, dpr);
}

function getAudioLevel() {
  const smoothing = config.animation.audioSmoothingFactor;
  audioLevel = audioLevel * smoothing + targetAudioLevel * (1 - smoothing);
  return audioLevel;
}

function render() {
  const w = canvas.getBoundingClientRect().width;
  const h = canvas.getBoundingClientRect().height;
  const cx = w / 2;
  const cy = h / 2;
  const maxRadius = Math.min(w, h) / 2 - 1;
  const graphicSize = Math.min(w/3, h/3);

  const opacitySpeed = Math.min(1.0, isFadingOut ? config.animation.fadeOutSpeed : config.animation.fadeInSpeed);
  globalOpacity += (targetOpacity - globalOpacity) * opacitySpeed;

  time += 0.016;
  const t = time;
  const rawLevel = getAudioLevel();
  const level = config.audio.baselineLevel + Math.min(rawLevel * config.audio.levelMultiplier, config.audio.levelCap);

  ctx.clearRect(0, 0, w, h);
  ctx.globalAlpha = globalOpacity;

  const targetBlend = (isProcessing || isPoofing || isFadingOut) ? 1.0 : 0.0;
  colorBlend += (targetBlend - colorBlend) * config.animation.colorBlendSpeed;

  const timeScale = 1.0 + colorBlend * (config.visuals.processingTimeScale - 1.0);
  const breathe = 1.0 - colorBlend * 0.3 + colorBlend * Math.sin(t * 3) * 0.3;

  function blendColor(purple, cyan) {
    return Math.round(purple + (cyan - purple) * colorBlend);
  }

  const idleGlow = 0.15 + Math.sin(t * 0.5 * timeScale) * 0.105;

  // Outer ambient nebula
  const ambientSize = Math.min(graphicSize * 0.045, maxRadius);
  const ambientGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, ambientSize);
  const ambientAlpha = idleGlow + level * 4.3;
  ambientGrad.addColorStop(0, `rgba(${blendColor(120, 60)}, ${blendColor(60, 140)}, ${blendColor(180, 180)}, ${ambientAlpha * 0.8})`);
  ambientGrad.addColorStop(0.5, `rgba(${blendColor(80, 40)}, ${blendColor(40, 100)}, ${blendColor(140, 160)}, ${ambientAlpha * 0.4})`);
  ambientGrad.addColorStop(1, 'rgba(40, 40, 80, 0)');
  ctx.fillStyle = ambientGrad;
  ctx.beginPath();
  ctx.arc(cx, cy, ambientSize, 0, Math.PI * 2);
  ctx.fill();

  // Swirling nebula clouds
  const cloudCount = config.visuals.cloudCount;
  const normalClouds = config.visuals.colors.normal.clouds;
  const processingClouds = config.visuals.colors.processing.clouds;

  for (let i = 0; i < cloudCount; i++) {
    const [pr, pg, pb] = normalClouds[i % normalClouds.length];
    const [cr, cg, cb] = processingClouds[i % processingClouds.length];
    const r = blendColor(pr, cr);
    const g = blendColor(pg, cg);
    const b = blendColor(pb, cb);
    const baseAngle = (i * Math.PI * 2 / cloudCount);
    const audioBoost = 1 + rawLevel * (config.visuals.cloudOrbitAudioBoost || 8);
    const orbitSpeed = (0.3 + (i % 2) * 0.2) * timeScale * audioBoost * 0.016;
    cloudAngles[i] = (cloudAngles[i] || 0) + orbitSpeed;
    const angle = baseAngle + cloudAngles[i];

    const baseDist = graphicSize * 0.015;
    const audioDist = level * graphicSize * 1.5;
    const breathDist = Math.sin(t * 0.8 + i) * graphicSize * 0.503;
    let dist = baseDist + audioDist + breathDist;

    const baseSize = graphicSize * level;
    const audioSize = level * graphicSize * 0.5;
    let size = baseSize + audioSize;

    const totalExtent = dist + size;
    if (totalExtent > maxRadius) {
      const scale = maxRadius / totalExtent;
      dist *= scale;
      size *= scale;
    }

    const x = cx + Math.cos(angle) * dist;
    const y = cy + Math.sin(angle) * dist;

    const grad = ctx.createRadialGradient(x, y, 0, x, y, size);
    const alpha = 0.3 + level * 0.5;
    grad.addColorStop(0, `rgba(${r}, ${g}, ${b}, ${alpha})`);
    grad.addColorStop(0.3, `rgba(${r}, ${g}, ${b}, ${alpha * 0.6})`);
    grad.addColorStop(0.6, `rgba(${r}, ${g}, ${b}, ${alpha * 0.2})`);
    grad.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0)`);

    ctx.fillStyle = grad;
    ctx.beginPath();
    ctx.arc(x, y, size, 0, Math.PI * 2);
    ctx.fill();
  }

  // Central core
  const coreBaseSize = graphicSize * (0.02 + colorBlend * 0.06 * breathe);
  const coreAudioSize = level * graphicSize * 1.4;
  const coreSize = Math.min(coreBaseSize + coreAudioSize, maxRadius * 0.8);
  const coreGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, coreSize);
  const coreAlpha = 0.4 + level * 2.6;
  const nc = config.visuals.colors.normal.core;
  const pc = config.visuals.colors.processing.core;
  coreGrad.addColorStop(0, `rgba(255, 255, 255, ${coreAlpha})`);
  coreGrad.addColorStop(0.2, `rgba(${blendColor(nc[0], pc[0])}, ${blendColor(nc[1], pc[1])}, 255, ${coreAlpha * 0.8})`);
  coreGrad.addColorStop(0.5, `rgba(${blendColor(nc[0]-60, pc[0]-60)}, ${blendColor(nc[1]-60, pc[1]-60)}, 255, ${coreAlpha * 0.4})`);
  coreGrad.addColorStop(1, `rgba(${blendColor(nc[0]-80, pc[0]-50)}, ${blendColor(nc[1]-50, pc[1]-80)}, 180, 0)`);

  ctx.fillStyle = coreGrad;
  ctx.beginPath();
  ctx.arc(cx, cy, coreSize, 0, Math.PI * 2);
  ctx.fill();

  // Sparkle particles
  const particleCount = Math.floor(config.visuals.particleMultiplier * level);
  for (let i = 0; i < particleCount; i++) {
    const seed1 = Math.sin(i * 12.9898) * 43758.5453;
    const seed2 = Math.cos(i * 78.233) * 23421.3411;
    const randomOffset1 = seed1 - Math.floor(seed1);
    const randomOffset2 = seed2 - Math.floor(seed2);

    const angle = randomOffset1 * Math.PI * 2 + t * (0.02 + randomOffset2 * 0.05);
    const baseDist = randomOffset2 * graphicSize * 0.8 + graphicSize * 0.1;
    const dist = Math.min(baseDist + level * graphicSize * 0.5, maxRadius - 1);

    const x = cx + Math.cos(angle) * dist;
    const y = cy + Math.sin(angle) * dist;

    const twinkle = Math.sin(t * (2 + randomOffset1 * 3) + i * 0.5);
    const alpha = (0.2 + twinkle * 0.30) * (0.5 + level * 0.05);
    const size = 1 + level * 2.2 + twinkle * 0.05;

    if (alpha > 0) {
      const ns = config.visuals.colors.normal.sparkle;
      const ps = config.visuals.colors.processing.sparkle;
      const sparkleR = blendColor(ns[0], ps[0]);
      const sparkleG = blendColor(ns[1], ps[1]);
      const sparkleB = blendColor(ns[2], ps[2]);
      ctx.fillStyle = `rgba(${sparkleR}, ${sparkleG}, ${sparkleB}, ${alpha})`;
      ctx.beginPath();
      ctx.arc(x, y, size, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  // Pulsing ring
  if (level > 0.14 && !isProcessing && !isPoofing) {
    const ringSize = Math.min(graphicSize * 0.04 + level * graphicSize * 1.3, maxRadius - 3);
    const ringAlpha = (level - 0.05) * 0.092;
    ctx.strokeStyle = `rgba(200, 150, 255, ${Math.min(ringAlpha, 1)})`;
    ctx.lineWidth = 2 + level * 4;
    ctx.beginPath();
    ctx.arc(cx, cy, ringSize, 0, Math.PI * 2);
    ctx.stroke();
  }

  // Poof burst effect
  if (isPoofing) {
    poofTime += 0.016;
    const poofDuration = config.animation.poofDuration;
    const poofProgress = poofTime / poofDuration;
    const easedProgress = 1 - Math.pow(1 - poofProgress, 2);

    if (poofProgress < 1) {
      const poofRingColor = config.visuals.colors.poof.ring;
      for (let ring = 0; ring < config.poof.ringCount; ring++) {
        const ringDelay = ring * config.poof.ringDelay;
        const ringProgress = Math.max(0, (poofProgress - ringDelay) / (1 - ringDelay));
        if (ringProgress > 0 && ringProgress < 1) {
          const wobble = Math.sin(t * 8 + ring * 2) * 5;
          const expandSize = graphicSize * 0.15 + ringProgress * graphicSize * 0.9 + wobble;
          const ringAlpha = (1 - ringProgress) * 0.5;
          ctx.strokeStyle = `rgba(${poofRingColor[0]}, ${poofRingColor[1]}, ${poofRingColor[2]}, ${ringAlpha})`;
          ctx.lineWidth = 3 * (1 - ringProgress);
          ctx.beginPath();
          ctx.arc(cx, cy, expandSize, 0, Math.PI * 2);
          ctx.stroke();
        }
      }

      const burstCount = config.poof.particleCount;
      const poofParticleColor = config.visuals.colors.poof.particle;
      for (let i = 0; i < burstCount; i++) {
        const seed = Math.sin(i * 127.1) * 43758.5453;
        const randomOffset = (seed - Math.floor(seed)) * 0.5 - 0.25;
        const baseAngle = (i / burstCount) * Math.PI * 2;
        const angle = baseAngle + randomOffset + poofProgress * 0.3;

        const speedVar = 0.7 + (Math.sin(i * 31.7) * 0.5 + 0.5) * 0.6;
        const burstDist = graphicSize * 0.1 + easedProgress * graphicSize * 1.2 * speedVar;

        const bx = cx + Math.cos(angle) * burstDist;
        const by = cy + Math.sin(angle) * burstDist;

        const fadeDelay = (Math.sin(i * 17.3) * 0.5 + 0.5) * 0.3;
        const particleAlpha = Math.max(0, (1 - (poofProgress - fadeDelay) / (1 - fadeDelay))) * 0.7;
        const burstSize = 2 + (1 - easedProgress) * 3 + Math.sin(i * 7.1) * 1.5;

        if (particleAlpha > 0) {
          ctx.fillStyle = `rgba(${poofParticleColor[0]}, ${poofParticleColor[1]}, ${poofParticleColor[2]}, ${particleAlpha})`;
          ctx.beginPath();
          ctx.arc(bx, by, burstSize, 0, Math.PI * 2);
          ctx.fill();
        }
      }

      const flashColor = config.visuals.colors.poof.flash;
      const flashAlpha = (1 - easedProgress) * 0.4;
      const flashSize = graphicSize * 0.25 * (1 - easedProgress * 0.3);
      const flashGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, flashSize);
      flashGrad.addColorStop(0, `rgba(255, 255, 255, ${flashAlpha})`);
      flashGrad.addColorStop(0.4, `rgba(${flashColor[0]}, ${flashColor[1]}, ${flashColor[2]}, ${flashAlpha * 0.6})`);
      flashGrad.addColorStop(1, `rgba(${flashColor[0] - 50}, ${flashColor[1]}, ${flashColor[2] - 30}, 0)`);
      ctx.fillStyle = flashGrad;
      ctx.beginPath();
      ctx.arc(cx, cy, flashSize, 0, Math.PI * 2);
      ctx.fill();
    } else {
      isPoofing = false;
    }
  }

  // Fade the canvas to transparent before the rectangular panel edge.
  ctx.globalCompositeOperation = 'destination-in';
  const fadeGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, maxRadius * 0.98);
  fadeGrad.addColorStop(0, 'rgba(255,255,255,1)');
  fadeGrad.addColorStop(0.52, 'rgba(255,255,255,1)');
  fadeGrad.addColorStop(0.78, 'rgba(255,255,255,0.22)');
  fadeGrad.addColorStop(1, 'rgba(255,255,255,0)');
  ctx.fillStyle = fadeGrad;
  ctx.fillRect(0, 0, w, h);
  ctx.globalCompositeOperation = 'source-over';

  animationId = requestAnimationFrame(render);
}

function updateStatus(text, className) {
  statusEl.textContent = text;
  statusEl.className = 'status ' + (className || '');
}

function start() {
  resize();
  if (!animationId) render();
}

console.log('[Orb-Nebula] READY - Rust controls via eval, drag handled natively');

window.addEventListener('resize', resize);
start();
