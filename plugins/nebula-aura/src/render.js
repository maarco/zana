/**
 * Nebula Aura - Zana Default Visualization
 *
 * A cosmic nebula with swirling particles that respond to your voice.
 * Ported from kollabor-app-v1 NebulaAuraOrb.vue
 */

// Plugin state
let time = 0;
let audioLevel = 0;
let config = {
  particle_density: 1.0,
  glow_intensity: 1.0,
  color_scheme: 'purple',
  sparkle_count: 40,
  cloud_count: 7,
};

// Color palettes
const COLOR_PALETTES = {
  purple: [
    [200, 80, 255],  // Bright magenta
    [140, 60, 220],  // Purple
    [255, 100, 200], // Pink
    [100, 80, 255],  // Blue-purple
    [180, 40, 200],  // Deep purple
    [255, 100, 200], // Pink
    [100, 80, 255],  // Blue-purple
  ],
  cyan: [
    [0, 255, 255],   // Cyan
    [0, 200, 255],   // Sky blue
    [100, 255, 200], // Turquoise
    [0, 150, 255],   // Blue
    [50, 255, 150],  // Mint
    [0, 200, 255],   // Sky blue
    [100, 255, 200], // Turquoise
  ],
  fire: [
    [255, 100, 0],   // Orange
    [255, 50, 0],    // Red-orange
    [255, 200, 0],   // Yellow
    [255, 0, 50],    // Red
    [255, 150, 0],   // Amber
    [255, 50, 0],    // Red-orange
    [255, 200, 0],   // Yellow
  ],
  rainbow: [
    [255, 0, 0],     // Red
    [255, 127, 0],   // Orange
    [255, 255, 0],   // Yellow
    [0, 255, 0],     // Green
    [0, 0, 255],     // Blue
    [75, 0, 130],    // Indigo
    [148, 0, 211],   // Violet
  ],
};

/**
 * Initialize the plugin
 */
export function init(ctx) {
  time = 0;
  audioLevel = 0;
  config = { ...config, ...ctx.config };
}

/**
 * Update animation state
 */
export function update(dt, level, fft) {
  time += dt;
  // Smooth audio level with less lag for better responsiveness
  audioLevel = audioLevel * 0.4 + level * 0.6;
}

/**
 * Render a frame
 */
export function render(ctx, width, height, level) {
  const c = ctx;
  const cx = width / 2;
  const cy = height / 2;

  // Max radius - nothing can extend beyond this
  const maxRadius = Math.min(width, height) / 2 - 1;

  // Scale graphic to fit window
  const graphicSize = Math.min(width / 3, height / 3);

  const t = time;

  // Boost the level significantly for better visual response
  const boostedLevel = Math.min(audioLevel * 3, 30);

  // Clear to transparent
  c.clearRect(0, 0, width, height);

  // Base idle glow - always visible even without audio
  const idleGlow = 0.15 + Math.sin(t * 0.5) * 0.105;
  const glowMult = config.glow_intensity;

  // Get color palette
  const colors = COLOR_PALETTES[config.color_scheme] || COLOR_PALETTES.purple;

  // Outer ambient nebula - soft background glow
  const ambientSize = Math.min(graphicSize * 0.045, maxRadius);
  const ambientAlpha = (idleGlow + boostedLevel * 4.3) * glowMult;

  const ambientGrad = c.createRadialGradient(cx, cy, 0, cx, cy, ambientSize);
  ambientGrad.addColorStop(0, `rgba(120, 60, 180, ${ambientAlpha * 0.8})`);
  ambientGrad.addColorStop(0.5, `rgba(80, 40, 140, ${ambientAlpha * 0.4})`);
  ambientGrad.addColorStop(1, 'rgba(40, 20, 80, 0)');

  c.fillStyle = ambientGrad;
  c.beginPath();
  c.arc(cx, cy, ambientSize, 0, Math.PI * 2);
  c.fill();

  // Swirling nebula clouds - orbit around center
  const cloudCount = Math.round(config.cloud_count);

  for (let i = 0; i < cloudCount; i++) {
    const colorIdx = i % colors.length;
    const [r, g, b] = colors[colorIdx];
    const baseAngle = (i * Math.PI * 2) / cloudCount;
    const orbitSpeed = 0.3 + (i % 2) * 0.2;
    const angle = baseAngle + t * orbitSpeed;

    // Clouds expand outward with audio
    const baseDist = graphicSize * 0.015;
    const audioDist = boostedLevel * graphicSize * 1.5;
    const breathDist = Math.sin(t * 0.8 + i) * graphicSize * 0.503;
    let dist = baseDist + audioDist + breathDist;

    // Cloud size pulses with audio
    const baseSize = graphicSize * boostedLevel;
    const audioSize = boostedLevel * graphicSize * 0.5;
    let size = baseSize + audioSize;

    // Constrain: dist + size must not exceed maxRadius
    const totalExtent = dist + size;
    if (totalExtent > maxRadius) {
      const scale = maxRadius / totalExtent;
      dist *= scale;
      size *= scale;
    }

    const x = cx + Math.cos(angle) * dist;
    const y = cy + Math.sin(angle) * dist;

    const alpha = (0.3 + boostedLevel * 0.5) * glowMult;

    const grad = c.createRadialGradient(x, y, 0, x, y, size);
    grad.addColorStop(0, `rgba(${r}, ${g}, ${b}, ${alpha})`);
    grad.addColorStop(0.3, `rgba(${r}, ${g}, ${b}, ${alpha * 0.6})`);
    grad.addColorStop(0.6, `rgba(${r}, ${g}, ${b}, ${alpha * 0.2})`);
    grad.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0)`);

    c.fillStyle = grad;
    c.beginPath();
    c.arc(x, y, size, 0, Math.PI * 2);
    c.fill();
  }

  // Central core - bright when speaking
  const coreBaseSize = graphicSize * 0.02;
  const coreAudioSize = boostedLevel * graphicSize * 1.4;
  const coreSize = Math.min(coreBaseSize + coreAudioSize, maxRadius * 0.8);
  const coreAlpha = (0.4 + boostedLevel * 2.6) * glowMult;

  const coreGrad = c.createRadialGradient(cx, cy, 0, cx, cy, coreSize);
  coreGrad.addColorStop(0, `rgba(255, 255, 255, ${coreAlpha})`);
  coreGrad.addColorStop(0.2, `rgba(240, 200, 255, ${coreAlpha * 0.8})`);
  coreGrad.addColorStop(0.5, `rgba(180, 100, 255, ${coreAlpha * 0.4})`);
  coreGrad.addColorStop(1, 'rgba(100, 50, 180, 0)');

  c.fillStyle = coreGrad;
  c.beginPath();
  c.arc(cx, cy, coreSize, 0, Math.PI * 2);
  c.fill();

  // Sparkle particles - scattered randomly
  const particleCount = Math.round(config.sparkle_count * config.particle_density * boostedLevel);

  for (let i = 0; i < particleCount; i++) {
    // Pseudo-random offset based on particle index (consistent per frame)
    const seed1 = Math.sin(i * 12.9898) * 43758.5453;
    const seed2 = Math.cos(i * 78.233) * 23421.3411;
    const randomOffset1 = seed1 - Math.floor(seed1);
    const randomOffset2 = seed2 - Math.floor(seed2);

    // Random angle with slow drift
    const angle = randomOffset1 * Math.PI * 2 + t * (0.02 + randomOffset2 * 0.05);

    // Random distance with audio expansion
    const baseDist = randomOffset2 * graphicSize * 0.8 + graphicSize * 0.1;
    const particleDist = Math.min(baseDist + boostedLevel * graphicSize * 0.5, maxRadius - 1);

    const x = cx + Math.cos(angle) * particleDist;
    const y = cy + Math.sin(angle) * particleDist;

    // Twinkle effect with random phase offset
    const twinkle = Math.sin(t * (2 + randomOffset1 * 3) + i * 0.5);
    const alpha = (0.2 + twinkle * 0.3) * (0.5 + boostedLevel * 0.05);
    const particleSize = 1 + boostedLevel * 2.2 + twinkle * 0.05;

    if (alpha > 0) {
      c.fillStyle = `rgba(255, 255, 255, ${alpha})`;
      c.beginPath();
      c.arc(x, y, particleSize, 0, Math.PI * 2);
      c.fill();
    }
  }

  // Pulsing ring - responds to audio peaks
  if (boostedLevel > 0.14) {
    const ringSize = Math.min(graphicSize * 0.04 + boostedLevel * graphicSize * 1.3, maxRadius - 3);
    const ringAlpha = (boostedLevel - 0.05) * 0.092 * glowMult;

    c.strokeStyle = `rgba(200, 150, 255, ${Math.min(ringAlpha, 1)})`;
    c.lineWidth = 2 + boostedLevel * 4;
    c.beginPath();
    c.arc(cx, cy, ringSize, 0, Math.PI * 2);
    c.stroke();
  }
}

/**
 * Handle window resize
 */
export function onResize(width, height) {
  // No state updates needed - render uses passed dimensions
}

/**
 * Handle configuration change
 */
export function onConfigChange(newConfig) {
  config = { ...config, ...newConfig };
}

/**
 * Cleanup
 */
export function destroy() {
  time = 0;
  audioLevel = 0;
}
