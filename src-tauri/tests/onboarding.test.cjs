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

  const context = {
    document: {
      getElementById() {
        return { classList: { add() {}, remove() {} }, style: {}, textContent: '' };
      },
    },
    navigator: { mediaDevices: { getUserMedia: async () => ({}) } },
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
  return { context, intervalCount: () => intervalCount };
}

test('granted microphone starts onboarding audio visualization', async () => {
  const { context, intervalCount } = loadAudioVisualization();
  await context.startAudioVisualization();
  assert.equal(intervalCount(), 1);
});
