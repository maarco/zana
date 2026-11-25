'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const onboardingPath = path.join(__dirname, '../../src-ui/onboarding.html');

function loadAudioVisualization() {
  const html = fs.readFileSync(onboardingPath, 'utf8');
  const start = html.indexOf('    async function startAudioVisualization() {');
  assert.notEqual(start, -1, 'audio visualization function exists');

  let depth = 0;
  let end = -1;
  for (let index = html.indexOf('{', start); index < html.length; index += 1) {
    if (html[index] === '{') depth += 1;
    if (html[index] === '}' && --depth === 0) {
      end = index + 1;
      break;
    }
  }
  assert.notEqual(end, -1, 'audio visualization function closes');

  let intervalCount = 0;
  class AudioContext {
    createMediaStreamSource() { return { connect() {} }; }
    createAnalyser() {
      return { fftSize: 0, frequencyBinCount: 4, getByteFrequencyData() {} };
    }
    close() {}
  }

  let microphoneRequestCount = 0;
  const context = {
    document: {
      getElementById() {
        return { classList: { add() {}, remove() {} }, style: {}, textContent: '' };
      },
    },
    navigator: {
      mediaDevices: {
        getUserMedia: async () => {
          microphoneRequestCount += 1;
          return {};
        },
      },
    },
    AudioContext,
    Uint8Array,
    setInterval() { intervalCount += 1; return intervalCount; },
    clearInterval() {},
    setTimeout,
    console: { warn() {}, error() {} },
    state: { audioLevel: 0, genesisProgress: 0 },
  };
  vm.runInNewContext(
    `${html.slice(start, end)}; globalThis.startAudioVisualization = startAudioVisualization;`,
    context,
  );
  return {
    context,
    intervalCount: () => intervalCount,
    microphoneRequestCount: () => microphoneRequestCount,
  };
}

test('overlapping microphone retries start one audio visualization', async () => {
  const { context, intervalCount, microphoneRequestCount } = loadAudioVisualization();
  await Promise.all([
    context.startAudioVisualization(),
    context.startAudioVisualization(),
  ]);
  assert.equal(microphoneRequestCount(), 1);
  assert.equal(intervalCount(), 1);
});

test('completion marker failure is shown in onboarding', async () => {
  const html = fs.readFileSync(onboardingPath, 'utf8');
  const start = html.indexOf('window.completeGenesis = async function() {');
  assert.notEqual(start, -1, 'completion handler exists');

  let depth = 0;
  let end = -1;
  for (let index = html.indexOf('{', start); index < html.length; index += 1) {
    if (html[index] === '{') depth += 1;
    if (html[index] === '}' && --depth === 0) {
      end = index + 1;
      break;
    }
  }
  assert.notEqual(end, -1, 'completion handler closes');

  const completionError = { hidden: true, textContent: '' };
  const context = {
    window: {
      __TAURI__: {
        core: {
          invoke: async () => ({ success: false, error: 'permission denied' }),
        },
      },
    },
    document: { getElementById: () => completionError },
    console: { error() {} },
  };
  vm.runInNewContext(
    `${html.slice(start, end)}; globalThis.completeGenesis = window.completeGenesis;`,
    context,
  );

  await context.completeGenesis();
  assert.equal(completionError.hidden, false);
  assert.match(completionError.textContent, /permission denied/);
});

test('macOS permission recovery checks and settings actions stay wired', () => {
  const html = fs.readFileSync(onboardingPath, 'utf8');

  for (const command of [
    'check_accessibility_permission',
    'check_screen_recording_permission',
    'check_automation_permission',
    'open_accessibility_settings',
    'open_screen_recording_settings',
    'open_automation_settings',
  ]) {
    assert.match(html, new RegExp(`['"]${command}['"]`), `${command} is wired`);
  }

  assert.match(html, /Screen Recording: awaiting permission/);
  assert.match(html, /Automation: allow on first use/);
  assert.match(html, /openScreenRecordingSettings/);
  assert.match(html, /openAutomationSettings/);
});

test('screenshot permission denial opens the Screen Recording settings pane', () => {
  const voicebotPath = path.join(__dirname, '../src/voicebot.rs');
  const source = fs.readFileSync(voicebotPath, 'utf8');

  assert.match(
    source,
    /if !crate::onboarding::check_screen_recording\(\)\s*\{\s*crate::onboarding::open_screen_recording_settings\(\);/,
  );
});
