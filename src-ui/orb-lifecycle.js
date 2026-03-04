/**
 * Orb Lifecycle Framework
 *
 * Shared state machine for orb renderers. Owns the 5-state lifecycle,
 * easing, window.* bridge, and rAF loop. Renderers just draw.
 *
 * States: DORMANT -> LISTENING -> PROCESSING -> COMPLETE -> HIDING -> DORMANT
 *
 * Detection: only activates if window.__orbRenderer is set by the
 * loaded renderer script. Un-migrated renderers work unchanged.
 */

const OrbState = Object.freeze({
  DORMANT: 'DORMANT',
  LISTENING: 'LISTENING',
  PROCESSING: 'PROCESSING',
  COMPLETE: 'COMPLETE',
  HIDING: 'HIDING'
});

class OrbLifecycle {
  constructor(canvas, renderer, options = {}) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d', { alpha: true });
    this.renderer = renderer;
    this.state = OrbState.DORMANT;
    this.animationId = null;
    this.lastFrameTime = 0;

    // configurable speeds
    const opts = Object.assign({
      genesis: { birthSpeed: 0.05, deathSpeed: 0.015 },
      opacity: { fadeIn: 0.08, fadeOut: 0.27 },
      audio: { smoothing: 0.18 },
      poof: { duration: 1.0 }
    }, options);

    this.config = opts;

    // animated values
    this.genesis = 0;
    this.targetGenesis = 0;
    this.globalOpacity = 0;
    this.targetOpacity = 1;
    this.audioLevel = 0;
    this.targetAudioLevel = 0;
    this.time = 0;
    this.poofTime = 0;
    this.poofDuration = opts.poof.duration;

    // screen
    this.width = 0;
    this.height = 0;

    // install window.* bridge
    this._installBridge();

    // resize + start
    this._resize();
    window.addEventListener('resize', () => this._resize());
    this._loop(0);

    console.log('[OrbLifecycle] initialized, state:', this.state);
  }

  // ── state transitions ──

  startListening() {
    if (this.state === OrbState.DORMANT || this.state === OrbState.HIDING) {
      this._transition(OrbState.LISTENING);
      this.targetGenesis = 1;
      this.targetOpacity = 1;
      this.audioLevel = 0;
      this.targetAudioLevel = 0;
    }
  }

  startProcessing() {
    if (this.state === OrbState.LISTENING) {
      this._transition(OrbState.PROCESSING);
      this.targetAudioLevel = 0;
    }
  }

  complete() {
    if (this.state === OrbState.PROCESSING || this.state === OrbState.LISTENING) {
      this._transition(OrbState.COMPLETE);
      this.poofTime = 0;
    }
  }

  hide() {
    if (this.state !== OrbState.DORMANT) {
      this._transition(OrbState.HIDING);
      this.targetOpacity = 0;
      this.targetGenesis = 0;
    }
  }

  reset() {
    const prev = this.state;
    this.state = OrbState.DORMANT;
    this.globalOpacity = 0;
    this.targetOpacity = 1;
    this.genesis = 0;
    this.targetGenesis = 0;
    this.audioLevel = 0;
    this.targetAudioLevel = 0;
    this.poofTime = 0;
    if (prev !== OrbState.DORMANT && this.renderer.onExit) {
      this.renderer.onExit(prev);
    }
    if (this.renderer.onEnter) {
      this.renderer.onEnter(OrbState.DORMANT);
    }
  }

  setAudioLevel(level) {
    this.targetAudioLevel = level;
  }

  // ── internal ──

  _transition(newState) {
    const prev = this.state;
    if (prev === newState) return;
    if (this.renderer.onExit) this.renderer.onExit(prev);
    this.state = newState;
    if (this.renderer.onEnter) this.renderer.onEnter(newState);
    console.log('[OrbLifecycle]', prev, '->', newState);
  }

  _installBridge() {
    const lc = this;

    window.setAudioLevel = function(level) {
      lc.setAudioLevel(level);
    };

    window.setRecordingState = function(recording, processing) {
      if (recording && !processing) {
        lc.startListening();
      } else if (processing) {
        lc.startProcessing();
      }
    };

    window.setTranscriptionComplete = function() {
      lc.complete();
    };

    window.fadeIn = function() {
      lc.targetOpacity = 1;
      if (lc.state === OrbState.HIDING) {
        lc._transition(OrbState.DORMANT);
      }
    };

    window.fadeOut = function() {
      lc.hide();
    };

    window.resetOrb = function() {
      lc.reset();
    };

    window.isFadeComplete = function() {
      return lc.state === OrbState.DORMANT && lc.globalOpacity < 0.01;
    };
  }

  _resize() {
    const dpr = window.devicePixelRatio || 1;
    const rect = this.canvas.getBoundingClientRect();
    this.canvas.width = rect.width * dpr;
    this.canvas.height = rect.height * dpr;
    this.ctx.scale(dpr, dpr);
    this.width = rect.width;
    this.height = rect.height;
    if (this.renderer.onResize) {
      this.renderer.onResize(this.width, this.height);
    }
  }

  _loop(timestamp) {
    const dt = this.lastFrameTime ? (timestamp - this.lastFrameTime) / 1000 : 0.016;
    this.lastFrameTime = timestamp;
    this.time += dt;

    this._updateEasing(dt);
    this._checkAutoTransitions();

    const frameCtx = {
      state: this.state,
      genesis: this.genesis,
      globalOpacity: this.globalOpacity,
      audioLevel: this.audioLevel,
      time: this.time,
      dt: dt,
      isRecording: this.state === OrbState.LISTENING,
      isProcessing: this.state === OrbState.PROCESSING,
      isPoofing: this.state === OrbState.COMPLETE,
      poofProgress: this.state === OrbState.COMPLETE
        ? Math.min(this.poofTime / this.poofDuration, 1)
        : 0,
      isFadingOut: this.state === OrbState.HIDING,
      canvas: this.canvas,
      width: this.width,
      height: this.height,
      cx: this.width / 2,
      cy: this.height / 2,
      ctx2d: this.ctx
    };

    this.renderer.render(frameCtx);

    this.animationId = requestAnimationFrame((ts) => this._loop(ts));
  }

  _updateEasing(dt) {
    const cfg = this.config;

    // opacity
    if (this.state === OrbState.HIDING) {
      this.globalOpacity += (0 - this.globalOpacity) * cfg.opacity.fadeOut;
    } else {
      this.globalOpacity += (this.targetOpacity - this.globalOpacity) * cfg.opacity.fadeIn;
    }

    // genesis
    const genSpeed = this.targetGenesis > this.genesis
      ? cfg.genesis.birthSpeed
      : cfg.genesis.deathSpeed;
    this.genesis += (this.targetGenesis - this.genesis) * genSpeed;

    // audio smoothing
    this.audioLevel += (this.targetAudioLevel - this.audioLevel) * cfg.audio.smoothing;

    // poof timer
    if (this.state === OrbState.COMPLETE) {
      this.poofTime += dt;
    }
  }

  _checkAutoTransitions() {
    // COMPLETE -> HIDING when poof done
    if (this.state === OrbState.COMPLETE && this.poofTime >= this.poofDuration) {
      this._transition(OrbState.HIDING);
      this.targetGenesis = 0;
      this.targetOpacity = 0;
    }

    // HIDING -> DORMANT when faded
    if (this.state === OrbState.HIDING && this.globalOpacity < 0.01) {
      this._transition(OrbState.DORMANT);
      this.globalOpacity = 0;
    }
  }
}
