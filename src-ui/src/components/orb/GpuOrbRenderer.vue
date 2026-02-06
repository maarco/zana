<!--
  GpuOrbRenderer.vue

  WebGPU/WebGL2 accelerated orb visualization renderer.
  Loads GPU plugins and renders them with hardware acceleration.
-->

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from 'vue';

// Types
interface GpuOrbProps {
  pluginId: string;
  width?: number;
  height?: number;
  audioLevel?: number;
  fftData?: Float32Array;
  config?: Record<string, any>;
  isRecording?: boolean;
}

interface GpuCapabilities {
  webgpu: boolean;
  webgl2: boolean;
  vendor: string | null;
  renderer: string | null;
}

const props = withDefaults(defineProps<GpuOrbProps>(), {
  width: 500,
  height: 500,
  audioLevel: 0,
  fftData: () => new Float32Array(32),
  config: () => ({}),
  isRecording: false,
});

const emit = defineEmits<{
  'renderer-ready': [capabilities: GpuCapabilities];
  'renderer-error': [error: string];
  'frame-rendered': [fps: number];
}>();

// Refs
const canvasRef = ref<HTMLCanvasElement | null>(null);
const renderer = ref<any>(null);
const capabilities = ref<GpuCapabilities>({
  webgpu: false,
  webgl2: false,
  vendor: null,
  renderer: null,
});
const isInitialized = ref(false);
const error = ref<string | null>(null);
const fps = ref(0);

// FPS tracking
let frameCount = 0;
let lastFpsTime = performance.now();

// Computed
const rendererType = computed(() => {
  if (capabilities.value.webgpu) return 'WebGPU';
  if (capabilities.value.webgl2) return 'WebGL2';
  return 'Canvas2D';
});

// Initialize renderer
async function initRenderer() {
  if (!canvasRef.value) return;

  try {
    // Detect GPU capabilities
    capabilities.value = await detectGpuCapabilities();

    // Dynamically import the plugin renderer
    const pluginModule = await import(`../../plugins/${props.pluginId}/src/renderer.js`);

    // Initialize the plugin
    await pluginModule.init({
      canvas: canvasRef.value,
      width: props.width,
      height: props.height,
      config: props.config,
    });

    renderer.value = pluginModule;
    isInitialized.value = true;

    emit('renderer-ready', capabilities.value);

    console.log(`[GpuOrb] Initialized ${props.pluginId} with ${rendererType.value}`);
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Unknown error';
    error.value = message;
    emit('renderer-error', message);
    console.error('[GpuOrb] Initialization failed:', err);
  }
}

// Detect GPU capabilities
async function detectGpuCapabilities(): Promise<GpuCapabilities> {
  const caps: GpuCapabilities = {
    webgpu: false,
    webgl2: false,
    vendor: null,
    renderer: null,
  };

  // Check WebGPU
  if ('gpu' in navigator) {
    try {
      const adapter = await navigator.gpu.requestAdapter();
      if (adapter) {
        caps.webgpu = true;
        const info = await adapter.requestAdapterInfo?.();
        if (info) {
          caps.vendor = info.vendor || null;
          caps.renderer = info.device || null;
        }
      }
    } catch {
      // WebGPU not available
    }
  }

  // Check WebGL2
  const testCanvas = document.createElement('canvas');
  const gl = testCanvas.getContext('webgl2');
  if (gl) {
    caps.webgl2 = true;
    const debugInfo = gl.getExtension('WEBGL_debug_renderer_info');
    if (debugInfo) {
      caps.vendor = caps.vendor || gl.getParameter(debugInfo.UNMASKED_VENDOR_WEBGL);
      caps.renderer = caps.renderer || gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL);
    }
  }

  return caps;
}

// Handle resize
function handleResize() {
  if (!canvasRef.value || !renderer.value) return;

  const dpr = window.devicePixelRatio || 1;
  canvasRef.value.width = props.width * dpr;
  canvasRef.value.height = props.height * dpr;
  canvasRef.value.style.width = `${props.width}px`;
  canvasRef.value.style.height = `${props.height}px`;

  renderer.value.onResize?.(props.width, props.height);
}

// Update FPS counter
function updateFps() {
  frameCount++;
  const now = performance.now();
  const elapsed = now - lastFpsTime;

  if (elapsed >= 1000) {
    fps.value = Math.round((frameCount * 1000) / elapsed);
    frameCount = 0;
    lastFpsTime = now;
    emit('frame-rendered', fps.value);
  }
}

// Watchers
watch(
  () => props.audioLevel,
  (level) => {
    if (renderer.value?.setAudioData) {
      renderer.value.setAudioData(level, level, props.fftData);
    }
  }
);

watch(
  () => props.fftData,
  (fft) => {
    if (renderer.value?.setAudioData) {
      renderer.value.setAudioData(props.audioLevel, props.audioLevel, fft);
    }
  }
);

watch(
  () => props.config,
  (config) => {
    renderer.value?.onConfigChange?.(config);
  },
  { deep: true }
);

watch(
  () => [props.width, props.height],
  () => handleResize()
);

// Lifecycle
onMounted(() => {
  initRenderer();
});

onUnmounted(() => {
  renderer.value?.destroy?.();
  renderer.value = null;
  isInitialized.value = false;
});
</script>

<template>
  <div class="gpu-orb-renderer" :style="{ width: `${width}px`, height: `${height}px` }">
    <!-- Main canvas -->
    <canvas
      ref="canvasRef"
      :width="width * (typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1)"
      :height="height * (typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1)"
      :style="{ width: `${width}px`, height: `${height}px` }"
    />

    <!-- Error overlay -->
    <div v-if="error" class="error-overlay">
      <div class="error-content">
        <span class="error-icon">!</span>
        <p>{{ error }}</p>
      </div>
    </div>

    <!-- Debug info (dev only) -->
    <div v-if="false" class="debug-info">
      <span>{{ rendererType }}</span>
      <span>{{ fps }} FPS</span>
      <span v-if="capabilities.renderer">{{ capabilities.renderer }}</span>
    </div>
  </div>
</template>

<style scoped>
.gpu-orb-renderer {
  position: relative;
  overflow: hidden;
}

canvas {
  display: block;
  background: transparent;
}

.error-overlay {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.8);
  color: white;
}

.error-content {
  text-align: center;
  padding: 20px;
}

.error-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: #ef4444;
  font-size: 24px;
  font-weight: bold;
  margin-bottom: 10px;
}

.debug-info {
  position: absolute;
  bottom: 10px;
  left: 10px;
  display: flex;
  gap: 10px;
  font-size: 10px;
  font-family: monospace;
  color: rgba(255, 255, 255, 0.5);
}
</style>
