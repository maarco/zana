# Third-Party Notices

## livekit-client 2.23.0

`src-ui/vendor/livekit-client.umd.min.js` is the LiveKit JavaScript client SDK,
copied unmodified from the npm package `livekit-client@2.23.0`.

Copyright LiveKit, Inc. Licensed under the Apache License, Version 2.0:
https://www.apache.org/licenses/LICENSE-2.0

## Aura visualizer shader

`src-ui/vendor/aura.js` contains the fragment shader of LiveKit's Agents UI
"agent-audio-visualizer-aura" component, copied unmodified with its header:
originally developed for Unicorn Studio (https://unicorn.studio), © 2026 UNCRN
LLC, licensed under the "Polyform Non-Resale License 1.0.0" as the header
states. The license link in that header
(https://polyformproject.org/licenses/non-resale/1.0.0/) did not resolve when
this was added, so its exact terms are unverified. The WebGL runner and state
animation around it are a port of LiveKit's Apache-2.0 component code.

## Voicebot Python packages

`voicebot/requirements.txt` and `voicebot/requirements-nodeps.txt` list the
packages Zana downloads into its Voicebot folder when Voicebot Mode is turned
on (including LiveKit Agents, parakeet-mlx and VibeVoice). They are not bundled
with Zana; each keeps its own license.
