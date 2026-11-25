'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');
const orb = require('../../src-ui/voicebot-orb-state.js');
const { Participant, ParticipantKind } = require('../../src-ui/vendor/livekit-client.umd.min.js');

function participant(identity, kind, attributes = {}, permissions = {}) {
  const value = Object.create(Participant.prototype);
  value.identity = identity;
  value._kind = kind;
  value.permissions = permissions;
  Object.defineProperty(value, 'attributes', { value: attributes });
  return value;
}

// FROZEN orb wire contract, 2026-11-17, UI owner: aquamarine.
// The LiveKit agent participant's retained `zana.orb.state` attribute is a
// string: idle | listening | thinking | talking | working. It is updated on
// each transition and read as a snapshot after join/rejoin. The producer
// resolves overlap as talking > working > thinking > listening > idle.
// The UI clears the snapshot on agent/room disconnect. Mute, dictation, sleep,
// voiceprint, and the local preview are UI-only overlays, not wire values.

test('each retained state maps to its own named color and animation', () => {
  const states = ['idle', 'listening', 'thinking', 'talking', 'working'];
  const styles = states.map((agentOrbState) => {
    const state = orb.resolveState({ connected: true, agentOrbState });
    assert.equal(state, agentOrbState);
    return orb.styleForState(state);
  });
  assert.deepEqual(styles.map((style) => style.label), [
    'Idle', 'Listening', 'Thinking', 'Talking', 'Working',
  ]);
  assert.equal(new Set(styles.map((style) => style.color)).size, states.length);
  assert.deepEqual(styles.map((style) => style.auraState), [
    'idle', 'listening', 'thinking', 'speaking', 'working',
  ]);
});

test('privacy and preview overlays override the retained state without mutating it', () => {
  const active = { connected: true, agentOrbState: 'working' };
  for (const quiet of [
    { connected: false }, { muted: true }, { asleep: true },
  ]) {
    assert.equal(orb.resolveState({ ...active, ...quiet }), 'idle');
  }
  assert.equal(orb.resolveState({ ...active, voicePrint: { seconds: 2 } }), 'voiceprint');
  assert.equal(orb.resolveState({ ...active, muted: true, preview: 'speaking' }), 'talking');
  assert.equal(orb.resolveState(active), 'working');
  assert.equal(orb.styleForState('voiceprint').color, '#FF3B30');
});

test('a silent microphone pulses red, even asleep, but not when muted on purpose', () => {
  const active = { connected: true, agentOrbState: 'listening', micSilent: true };
  assert.equal(orb.resolveState(active), 'nomic');
  assert.equal(orb.resolveState({ ...active, asleep: true }), 'nomic');
  assert.equal(orb.resolveState({ ...active, muted: true }), 'idle');
  assert.equal(orb.resolveState({ ...active, connected: false }), 'idle');
  assert.equal(orb.resolveState({ ...active, micSilent: false }), 'listening');
  assert.equal(orb.styleForState('nomic').pulse, true);
});

test('legacy or invalid snapshots fall back safely to LiveKit agent and local speech state', () => {
  const connected = { connected: true };
  assert.equal(orb.resolveState({ ...connected, agentState: 'speaking' }), 'talking');
  assert.equal(orb.resolveState({ ...connected, agentState: 'thinking' }), 'thinking');
  assert.equal(orb.resolveState({ ...connected, agentState: 'listening' }), 'idle');
  assert.equal(orb.resolveState({ ...connected, agentState: 'listening', userSpeaking: true }), 'listening');
  assert.equal(orb.resolveState({ ...connected, agentOrbState: 'voiceprint', agentState: 'speaking' }), 'talking');
  assert.equal(orb.resolveState({ ...connected, agentOrbState: '__proto__' }), 'idle');
  assert.equal(orb.resolveState({ ...connected, agentOrbState: 'working', agentState: 'speaking' }), 'working');
});

test('Aura renders dim idle, a bright thinking pulse, and an animated working state', () => {
  const fs = require('node:fs');
  const vm = require('node:vm');
  const uniforms = {};
  const gl = new Proxy({}, {
    get(_target, key) {
      if (key === 'getShaderParameter' || key === 'getProgramParameter') return () => true;
      if (key === 'getUniformLocation') return (_program, name) => name;
      if (key === 'uniform1f') return (name, value) => { uniforms[name] = value; };
      if (key === 'uniform3fv' || key === 'uniform3f') return () => {};
      if (typeof key === 'string' && key === key.toUpperCase()) return 1;
      return () => ({});
    },
  });
  let now = 0;
  let frame;
  const sandbox = {
    window: { devicePixelRatio: 1 },
    performance: { now: () => now },
    requestAnimationFrame: (next) => { frame = next; },
    Float32Array,
  };
  vm.runInNewContext(fs.readFileSync(require.resolve('../../src-ui/vendor/aura.js'), 'utf8'), sandbox);
  const aura = sandbox.window.createAura({
    clientWidth: 200, clientHeight: 200, width: 200, height: 200,
    getContext: () => gl,
  });
  const step = (ms) => { now += ms; frame(now); };

  for (let i = 0; i < 30; i++) step(100);
  assert.ok(uniforms.uMix < 0.8, `idle should settle dim, got ${uniforms.uMix}`);
  aura.setState('thinking');
  step(1);
  const low = uniforms.uMix;
  step(350);
  const high = uniforms.uMix;
  assert.ok(low >= 1.2, `thinking should remain bright, got ${low}`);
  assert.ok(high > low + 0.5, `thinking should pulse, got ${low} and ${high}`);
  aura.setState('working');
  const before = uniforms.iTime;
  step(100);
  assert.ok(uniforms.iTime - before > 3, 'working should animate faster than idle');
  aura.destroy();
});

function makeUi(agent, options = {}) {
  const fs = require('node:fs');
  const vm = require('node:vm');
  const html = fs.readFileSync(require.resolve('../../src-ui/voicebot.html'), 'utf8');
  const script = html.match(/<script>\s*([\s\S]*?)<\/script>/)[1];
  const eventNames = [
    'TrackSubscribed', 'ParticipantAttributesChanged', 'DataReceived', 'Disconnected',
    'ParticipantConnected', 'ParticipantDisconnected', 'ActiveSpeakersChanged',
  ];
  const RoomEvent = Object.fromEntries(eventNames.map((name) => [name, name]));
  let activeRoom;
  let releaseFirstConnection;
  let connectionCount = 0;
  let disconnectCount = 0;
  if (agent && agent.isAgent === undefined) agent.isAgent = true;
  class FakeRoom {
    constructor() {
      this.handlers = {};
      this.methods = {};
      this.remoteParticipants = new Map(agent ? [[agent.identity, agent]] : []);
      this.localParticipant = { isSpeaking: false, setMicrophoneEnabled: async () => {} };
      activeRoom = this;
    }
    on(event, callback) { this.handlers[event] = callback; }
    emit(event, ...args) { if (this.handlers[event]) this.handlers[event](...args); }
    registerRpcMethod(name, callback) { this.methods[name] = callback; }
    async connect() {
      connectionCount++;
      if (options.deferFirstConnect && connectionCount === 1) {
        await new Promise((resolve) => { releaseFirstConnection = resolve; });
      }
    }
    async disconnect() {
      disconnectCount++;
      if (options.failFirstDisconnect) {
        options.failFirstDisconnect = false;
        throw new Error('disconnect failed');
      }
      if (!options.noDisconnectEvent) this.emit('Disconnected');
    }
  }
  const element = {
    title: '', attributes: {}, listeners: {},
    setAttribute(key, value) { this.attributes[key] = value; },
    addEventListener(name, callback) { this.listeners[name] = callback; },
  };
  const visual = {};
  const aura = {
    setState(state) { visual.state = state; },
    setColor(color) { visual.color = color; },
    setVolume(volume) { visual.volume = volume; },
  };
  const timers = new Map();
  let nextTimer = 0;
  let now = 0;
  let frame;
  const window = {
    VoicebotOrb: orb,
    LivekitClient: { Room: FakeRoom, RoomEvent, ParticipantKind },
    createAura: () => aura,
    __TAURI__: { core: { invoke: async (command) => command === 'voicebot_status'
      ? { stage: 'ready' } : { url: 'ws://localhost', token: 'test' } } },
  };
  const sandbox = {
    window,
    document: {
      body: { appendChild() {}, classList: { toggle() {} } },
      addEventListener() {}, getElementById: () => element,
    },
    navigator: { clipboard: { writeText: async () => {} } },
    requestAnimationFrame: (callback) => { frame = callback; },
    setTimeout: (callback, delay = 0) => {
      const id = ++nextTimer;
      callback.due = now + delay;
      timers.set(id, callback);
      return id;
    },
    clearTimeout: (id) => timers.delete(id),
    setInterval: () => 0, // the mic check's once-a-second meter; its logic is covered by resolveState
    TextDecoder,
    console,
  };
  vm.runInNewContext(script, sandbox, { filename: 'voicebot.html' });
  return {
    element, visual, timers,
    room: () => activeRoom,
    disconnectCount: () => disconnectCount,
    advance: (ms) => {
      now += ms;
      for (const [id, callback] of [...timers]) {
        if (callback.due <= now && timers.delete(id)) callback();
      }
    },
    releaseFirstConnection: () => releaseFirstConnection(),
    tick: () => frame(),
    flush: () => new Promise((resolve) => setImmediate(resolve)),
  };
}

test('agent arrival cancels the watchdog before delayed attributes, while humans do not', async () => {
  const ui = makeUi(null);
  await ui.flush();
  const room = ui.room();
  const human = participant('human', ParticipantKind.STANDARD, { 'lk.agent.state': 'speaking' });
  room.remoteParticipants.set(human.identity, human);
  room.emit('ParticipantConnected', human);
  room.emit('ParticipantAttributesChanged', human.attributes, human);
  assert.equal(ui.timers.size, 1, 'a human must not cancel the arrival watchdog');
  ui.advance(14999);
  const agent = participant('agent', ParticipantKind.AGENT);
  room.remoteParticipants.set(agent.identity, agent);
  room.emit('ParticipantConnected', agent);
  assert.equal(ui.timers.size, 0, 'agent arrival cancels the watchdog without state attributes');
  ui.advance(10000);
  await ui.flush();
  assert.equal(ui.room(), room);
  assert.equal(ui.disconnectCount(), 0);
  agent.attributes['zana.orb.state'] = 'working';
  room.emit('ParticipantAttributesChanged', agent.attributes, agent);
  ui.tick();
  assert.match(ui.element.title, /^Working/);
  assert.equal(ui.visual.state, 'working');
});

test('an agent with no attributes in the join snapshot is recognized', async () => {
  const agent = participant('agent', ParticipantKind.AGENT);
  const ui = makeUi(agent);
  await ui.flush();
  assert.equal(ui.timers.size, 0);
  ui.advance(15000);
  await ui.flush();
  assert.equal(ui.disconnectCount(), 0);
  agent.attributes['lk.agent.state'] = 'thinking';
  ui.room().emit('ParticipantAttributesChanged', agent.attributes, agent);
  assert.match(ui.element.title, /^Thinking/);
});

test('agent permission identifies an agent before its state attributes arrive', async () => {
  const ui = makeUi(null);
  await ui.flush();
  const room = ui.room();
  const agent = participant('permission-agent', ParticipantKind.STANDARD, {}, { agent: true });
  assert.equal(agent.isAgent, true);
  ui.advance(14999);
  room.remoteParticipants.set(agent.identity, agent);
  room.emit('ParticipantConnected', agent);
  assert.equal(ui.timers.size, 0);
  ui.advance(15000);
  await ui.flush();
  assert.equal(ui.disconnectCount(), 0);
  agent.attributes['zana.orb.state'] = 'talking';
  room.emit('ParticipantAttributesChanged', agent.attributes, agent);
  assert.match(ui.element.title, /^Talking/);
});

test('Voicebot reads retained state on join, reacts to changes, and resets on agent departure', async () => {
  const agent = {
    identity: 'agent', audioLevel: 0,
    attributes: { 'lk.agent.state': 'thinking', 'zana.orb.state': 'thinking' },
  };
  const ui = makeUi(agent);
  await ui.flush();
  assert.match(ui.element.title, /^Thinking/);
  assert.match(ui.element.attributes['aria-label'], /^Voicebot: Thinking/);
  const room = ui.room();
  for (const state of ['idle', 'listening', 'thinking', 'talking', 'working']) {
    agent.attributes['zana.orb.state'] = state;
    room.emit('ParticipantAttributesChanged', { 'zana.orb.state': state }, agent);
    ui.tick();
    const style = orb.styleForState(state);
    assert.match(ui.element.title, new RegExp(`^${style.label}`));
    assert.equal(ui.visual.state, style.auraState);
    assert.equal(ui.visual.color, style.color);
  }
  room.remoteParticipants.delete(agent.identity);
  room.emit('ParticipantDisconnected', agent);
  ui.tick();
  assert.equal(ui.visual.state, 'idle');
  assert.equal(ui.visual.color, orb.styleForState('idle').color);
  assert.doesNotMatch(ui.element.title, /^Working/);
  room.emit('ParticipantAttributesChanged', { 'zana.orb.state': 'working' }, agent);
  ui.tick();
  assert.equal(ui.visual.state, 'idle', 'late events from a departed agent must be ignored');
});

test('an older agent shows Listening only while the local user is speaking', async () => {
  const agent = { identity: 'agent', attributes: { 'lk.agent.state': 'listening' } };
  const ui = makeUi(agent);
  await ui.flush();
  const room = ui.room();
  ui.tick();
  assert.match(ui.element.title, /^Idle/);
  assert.equal(ui.visual.state, 'idle');
  room.localParticipant.isSpeaking = true;
  room.emit('ActiveSpeakersChanged', [room.localParticipant]);
  ui.tick();
  assert.match(ui.element.title, /^Listening/);
  assert.equal(ui.visual.state, 'listening');
  room.localParticipant.isSpeaking = false;
  room.emit('ActiveSpeakersChanged', []);
  ui.tick();
  assert.match(ui.element.title, /^Idle/);
  assert.equal(ui.visual.state, 'idle');
});

test('Voicebot preserves voiceprint, sleep, mute, and timed preview overlays', async () => {
  const agent = {
    identity: 'agent', attributes: { 'lk.agent.state': 'thinking', 'zana.orb.state': 'thinking' },
  };
  const ui = makeUi(agent);
  await ui.flush();
  const room = ui.room();
  const event = (value) => room.emit('DataReceived', new TextEncoder().encode(JSON.stringify(value)), null, null, 'events');
  event({ kind: 'voiceprint', action: 'recording', seconds: 2, needed: 8 });
  ui.tick();
  assert.match(ui.element.title, /^Voice Print: Keep talking: 2 of 8 seconds/);
  assert.equal(ui.visual.color, '#FF3B30');
  event({ kind: 'voiceprint', action: 'enrolled' });
  event({ kind: 'sleep', asleep: true, wake_word: 'Zana' });
  ui.tick();
  assert.match(ui.element.title, /^Sleeping: Say "Zana"/);
  assert.equal(ui.visual.color, '#636366');

  await room.methods['zana.preview_state']({ payload: 'working' });
  await room.methods['zana.mute']();
  ui.tick();
  assert.match(ui.element.title, /^Working/);
  assert.equal(ui.visual.color, orb.styleForState('working').color);
  await room.methods['zana.preview_state']({ payload: 'speaking' });
  ui.tick();
  assert.match(ui.element.title, /^Talking/);
  assert.equal(ui.timers.size, 1, 'a new preview replaces the previous timer');
  const timeout = [...ui.timers.values()][0];
  timeout();
  ui.tick();
  assert.match(ui.element.title, /^Muted/);
  assert.equal(ui.visual.state, 'idle');
});

test('starting a new conversation while a room is connecting is not lost', async () => {
  const agent = { identity: 'agent', attributes: { 'lk.agent.state': 'listening' } };
  const ui = makeUi(agent, { deferFirstConnect: true });
  await ui.flush();
  const firstRoom = ui.room();
  ui.element.listeners.dblclick();
  await ui.flush();
  ui.releaseFirstConnection();
  await ui.flush();
  await ui.flush();
  assert.notEqual(ui.room(), firstRoom, 'the new conversation must start after the old connect ends');
  assert.match(ui.element.title, /^Idle/);
});

test('room disconnect clears previews and ignores late events from the previous room', async () => {
  const agent = {
    identity: 'agent', attributes: { 'lk.agent.state': 'thinking', 'zana.orb.state': 'thinking' },
  };
  const ui = makeUi(agent);
  await ui.flush();
  const room = ui.room();
  await room.methods['zana.preview_state']({ payload: 'working' });
  room.emit('Disconnected');
  ui.tick();
  assert.match(ui.element.title, /^Reconnecting/);
  assert.equal(ui.visual.state, 'idle');
  assert.equal(ui.visual.color, orb.styleForState('idle').color);
  agent.attributes['zana.orb.state'] = 'talking';
  room.emit('ParticipantAttributesChanged', { 'zana.orb.state': 'talking' }, agent);
  ui.tick();
  assert.match(ui.element.title, /^Reconnecting/);
  assert.equal(ui.visual.state, 'idle');
});

test('a failed new-conversation disconnect is visible and allows a retry', async () => {
  const agent = { identity: 'agent', attributes: { 'lk.agent.state': 'listening' } };
  const ui = makeUi(agent, { failFirstDisconnect: true });
  await ui.flush();
  const firstRoom = ui.room();
  ui.element.listeners.dblclick();
  await ui.flush();
  assert.match(ui.element.title, /^Error: Could not start a new conversation: disconnect failed/);
  for (const timeout of ui.timers.values()) timeout();
  ui.element.listeners.dblclick();
  await ui.flush();
  assert.notEqual(ui.room(), firstRoom);
  assert.match(ui.element.title, /^Idle/);
});

test('a new conversation resets state even if room disconnect emits no event', async () => {
  const agent = {
    identity: 'agent', attributes: { 'lk.agent.state': 'thinking', 'zana.orb.state': 'working' },
  };
  const ui = makeUi(agent, { noDisconnectEvent: true });
  await ui.flush();
  const oldRoom = ui.room();
  ui.element.listeners.dblclick();
  await ui.flush();
  const newRoom = ui.room();
  assert.notEqual(newRoom, oldRoom);
  agent.attributes['zana.orb.state'] = 'talking';
  oldRoom.emit('ParticipantAttributesChanged', { 'zana.orb.state': 'talking' }, agent);
  ui.tick();
  assert.match(ui.element.title, /^Working/, 'late events from the previous room are ignored');
  assert.equal(ui.visual.state, 'working');
});

test('agent state received during join is not overwritten when connect completes', async () => {
  const agent = { identity: 'agent', attributes: { 'lk.agent.state': 'thinking' } };
  const ui = makeUi(agent, { deferFirstConnect: true });
  await ui.flush();
  const room = ui.room();
  agent.attributes['zana.orb.state'] = 'working';
  room.emit('ParticipantAttributesChanged', { 'zana.orb.state': 'working' }, agent);
  assert.match(ui.element.title, /^Working/);
  ui.releaseFirstConnection();
  await ui.flush();
  ui.tick();
  assert.match(ui.element.title, /^Working/);
  assert.equal(ui.visual.state, 'working');
});
