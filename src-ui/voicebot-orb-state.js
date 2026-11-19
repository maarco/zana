// Shared presentation for the Voicebot orb's retained `zana.orb.state` attribute.
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.VoicebotOrb = api;
})(globalThis, () => {
  const styles = Object.freeze({
    idle:       { label: 'Idle',      color: '#527377', auraState: 'idle' },
    listening:  { label: 'Listening', color: '#30D158', auraState: 'listening' },
    thinking:   { label: 'Thinking',  color: '#00D9C3', auraState: 'thinking' },
    talking:    { label: 'Talking',   color: '#0A84FF', auraState: 'speaking' },
    working:    { label: 'Working',   color: '#FF9F0A', auraState: 'working' },
    voiceprint: { label: 'Voice Print', color: '#FF3B30', auraState: 'listening' },
    // UI-only overlay: no sound at all from the microphone for the mic-check minutes (pulses)
    nomic:      { label: 'No Microphone', color: '#FF3B30', auraState: 'idle', pulse: true },
  });

  const hasStyle = (state) => Object.prototype.hasOwnProperty.call(styles, state);

  function resolveState({ connected, muted, dictating, asleep, voicePrint, preview,
    agentOrbState, agentState, userSpeaking, micSilent }) {
    const previewState = preview === 'speaking' ? 'talking' : preview;
    if (hasStyle(previewState)) return previewState;
    // A dead microphone shows even while asleep (the wake word needs it); not when muted on purpose.
    if (micSilent && connected && !muted && !dictating) return 'nomic';
    if (!connected || muted || dictating || asleep) return 'idle';
    if (voicePrint) return 'voiceprint';
    if (agentOrbState !== 'voiceprint' && hasStyle(agentOrbState)) return agentOrbState;
    // Older agents have no orb attribute. LiveKit's "listening" means it is
    // ready for input, so only show Listening when the local user is speaking.
    if (agentState === 'speaking') return 'talking';
    if (agentState === 'thinking') return 'thinking';
    if (agentState === 'listening' && userSpeaking) return 'listening';
    return 'idle';
  }

  function styleForState(state) {
    return hasStyle(state) ? styles[state] : styles.idle;
  }

  return { resolveState, styleForState };
});
