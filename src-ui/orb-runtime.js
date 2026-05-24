(function () {
  const nativeRequestAnimationFrame = window.requestAnimationFrame.bind(window);
  const nativeCancelAnimationFrame = window.cancelAnimationFrame.bind(window);
  const pausedFrames = new Map();
  const nativeFrames = new Map();

  let nextFrameId = 1;
  let active = false;
  let sleepTimer = null;

  function scheduleNativeFrame(frameId, callback) {
    const nativeId = nativeRequestAnimationFrame((timestamp) => {
      nativeFrames.delete(frameId);
      callback(timestamp);
    });
    nativeFrames.set(frameId, nativeId);
  }

  function flushPausedFrames() {
    if (!active || pausedFrames.size === 0) {
      return;
    }

    const frames = Array.from(pausedFrames.entries());
    pausedFrames.clear();
    for (const [frameId, callback] of frames) {
      scheduleNativeFrame(frameId, callback);
    }
  }

  function wake() {
    active = true;
    if (sleepTimer) {
      clearTimeout(sleepTimer);
      sleepTimer = null;
    }
    flushPausedFrames();
  }

  function sleepNow() {
    active = false;
    if (sleepTimer) {
      clearTimeout(sleepTimer);
      sleepTimer = null;
    }
  }

  function sleepSoon(delayMs = 2200) {
    if (sleepTimer) {
      clearTimeout(sleepTimer);
    }
    sleepTimer = setTimeout(sleepNow, delayMs);
  }

  window.requestAnimationFrame = function (callback) {
    const frameId = nextFrameId++;

    if (!active) {
      pausedFrames.set(frameId, callback);
      return frameId;
    }

    scheduleNativeFrame(frameId, callback);
    return frameId;
  };

  window.cancelAnimationFrame = function (frameId) {
    const nativeId = nativeFrames.get(frameId);
    if (nativeId !== undefined) {
      nativeCancelAnimationFrame(nativeId);
      nativeFrames.delete(frameId);
    }
    pausedFrames.delete(frameId);
  };

  function wrapControl(name, after) {
    const original = window[name];
    if (typeof original !== 'function' || original.__orbRuntimeWrapped) {
      return;
    }

    const wrapped = function (...args) {
      wake();
      const result = original.apply(this, args);
      if (after) {
        after();
      }
      return result;
    };
    wrapped.__orbRuntimeWrapped = true;
    window[name] = wrapped;
  }

  function installControlBridge() {
    wrapControl('setAudioLevel');
    wrapControl('setRecordingState');
    wrapControl('resetOrb');
    wrapControl('fadeIn');
    wrapControl('fadeOut', () => sleepSoon(2200));
    wrapControl('setTranscriptionComplete', () => sleepSoon(4000));
  }

  window.__orbRuntime = {
    wake,
    sleepNow,
    sleepSoon,
    installControlBridge,
    isActive: () => active,
  };
})();
