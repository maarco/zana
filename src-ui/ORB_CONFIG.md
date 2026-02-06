# Orb Configuration Reference

All values hot-reload when you save `orb_config.json`.

## animation

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `fadeInSpeed` | float | 0.01-1.0 | How fast orb fades in. Higher = faster. 1.0 = instant |
| `fadeOutSpeed` | float | 0.01-1.0 | How fast orb fades out. Higher = faster. 1.0 = instant |
| `colorBlendSpeed` | float | 0.01-1.0 | How fast colors transition (purple <-> cyan) |
| `audioSmoothingFactor` | float | 0.0-0.99 | Audio level smoothing. Higher = smoother/slower response |
| `poofDuration` | float | seconds | Duration of the completion burst animation |

## audio

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `baselineLevel` | float | 0.0-1.0 | Minimum orb visibility even in silence |
| `levelMultiplier` | float | 0.1-10 | How much audio affects orb size. Higher = more reactive |
| `levelCap` | float | 1-100 | Maximum audio level effect (prevents over-expansion) |

## visuals

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `cloudCount` | int | 1-20 | Number of orbiting nebula clouds |
| `particleMultiplier` | int | 1-100 | Sparkle particle density |
| `processingTimeScale` | float | 1.0-10 | Animation speed multiplier during processing |
| `cloudOrbitAudioBoost` | float | 0-20 | How much audio speeds up cloud orbit |

### visuals.colors

Colors are RGB arrays: `[R, G, B]` where each value is 0-255.

- `normal.clouds` - Array of cloud colors during recording (purple theme)
- `normal.core` - Core glow color during recording
- `normal.sparkle` - Sparkle color during recording
- `processing.clouds` - Cloud colors during transcription (cyan theme)
- `processing.core` - Core glow during transcription
- `processing.sparkle` - Sparkle color during transcription
- `poof.ring` - Expanding ring color on completion
- `poof.particle` - Burst particle color on completion
- `poof.flash` - Central flash color on completion

## poof

| Parameter | Type | Range | Description |
|-----------|------|-------|-------------|
| `ringCount` | int | 0-10 | Number of expanding rings on completion |
| `particleCount` | int | 0-100 | Number of burst particles on completion |
| `ringDelay` | float | 0.0-0.5 | Stagger delay between rings (seconds) |

## window (Rust-side, requires restart)

| Parameter | Type | Description |
|-----------|------|-------------|
| `size` | int | Window size in pixels (not hot-reloaded) |
| `panelLevel` | int | macOS window level (1000 = above fullscreen) |
| `fadeOutDelay` | int | ms to wait after fade before hiding window |
| `animationCompleteDelay` | int | ms to wait for poof before starting fade |
