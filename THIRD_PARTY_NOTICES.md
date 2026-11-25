# Third-Party Notices

This notice covers third-party files in the repository and the default
Voicebot assets that Zana fetches into the user's app-data directory. It is
not a legal opinion or a grant of rights beyond the upstream terms. The
complete license texts for the retained vendored code are in LICENSES/.

## Vendored JavaScript

### livekit-client 2.23.0

src-ui/vendor/livekit-client.umd.min.js is the LiveKit JavaScript client SDK,
copied unmodified from the npm package livekit-client@2.23.0.

Copyright 2021 LiveKit, Inc. The upstream package is licensed under the
Apache License, Version 2.0. The upstream NOTICE attribution is reproduced
here and the full text is in LICENSES/Apache-2.0.txt.

Upstream package: https://github.com/livekit/client-sdk-js/tree/v2.23.0
License: https://github.com/livekit/client-sdk-js/blob/v2.23.0/LICENSE
NOTICE: https://github.com/livekit/client-sdk-js/blob/v2.23.0/NOTICE

Copyright 2021 LiveKit, Inc.
Licensed under the Apache License, Version 2.0 (the License);
you may not use this file except in compliance with the License.
You may obtain a copy of the License at
https://www.apache.org/licenses/LICENSE-2.0

### three.js r186 (0.186.1)

src-ui/vendor/three.module.js and src-ui/vendor/three.core.js are the
three.js build files, copied unmodified from the npm package three@0.186.1.
Copyright 2010-2026 Three.js authors, MIT License. They render the Fur,
Nebula and Liquid orb designs. The applicable text is in
LICENSES/MIT-Third-Party.txt.

Upstream package: https://github.com/mrdoob/three.js/tree/r186
License: https://github.com/mrdoob/three.js/blob/r186/LICENSE

### Zana orb shader

src-ui/vendor/aura.js is original Zana WebGL code. It has no Unicorn Studio,
Polyform, LiveKit UI, or other third-party shader source. The existing
createAura API and Voicebot state behavior remain intact.

## Voicebot runtime assets

The model weights and voice prompt files below are not bundled in the Zana
release artifact. On first Voicebot start, Zana installs the pinned Python
packages and downloads the default model assets into the user's Voicebot
folder. Environment variables can replace the model identifiers; those
user-selected assets are outside this inventory.

### VibeVoice speech model

Default identifier: microsoft/VibeVoice-Realtime-0.5B, loaded by
voicebot/voice_server.py. The upstream model card declares MIT and separately
describes research-purpose intent, out-of-scope uses, voice-impersonation
limits, and a recommendation against commercial or real-world use without
further testing. This notice records those statements without interpreting
them as legal clearance for commercial redistribution.

Model card: https://huggingface.co/microsoft/VibeVoice-Realtime-0.5B
License field: https://huggingface.co/microsoft/VibeVoice-Realtime-0.5B/raw/main/README.md
Observed model revision: 6bce5f06044837fe6d2c5d7a71a84f0416bd57e4. The loader
follows the repository's default revision rather than pinning that snapshot.

### Parakeet speech-to-text model

Default identifier: mlx-community/parakeet-tdt-0.6b-v3, loaded by
voicebot/voice_server.py. The MLX repository declares CC BY 4.0 and says it
is a format conversion of nvidia/parakeet-tdt-0.6b-v3. Attribute NVIDIA for
the original model and MLX Community for the conversion. The converted
repository revision observed during this audit was
ed2b7e8c15f9aaa0b5772e2efb986255eaef7e15; the current loader follows the
repository's default revision rather than pinning that snapshot.

MLX model card: https://huggingface.co/mlx-community/parakeet-tdt-0.6b-v3
Original model card: https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3
License text: https://creativecommons.org/licenses/by/4.0/legalcode

### SpeechBrain ECAPA voice-print model

Default identifier: speechbrain/spkrec-ecapa-voxceleb, loaded by
voicebot/voiceprint.py. The upstream model card declares Apache-2.0 and says
the model was trained with VoxCeleb 1 and VoxCeleb 2. Zana uses the model
locally and saves only the resulting 192-number embedding; it does not
redistribute the training data. The upstream revision observed during this
audit was 0f99f2d0ebe89ac095bcc5903c4dd8f72b367286; the loader follows the
repository's default revision rather than pinning that snapshot.

Model card: https://huggingface.co/speechbrain/spkrec-ecapa-voxceleb
License: https://www.apache.org/licenses/LICENSE-2.0

### VibeVoice fork and stock voice prompts

voicebot/requirements-nodeps.txt installs the VibeVoice community fork at
commit 07cb79feadd2d3fd7f47530d4c964a12857936a0. The upstream commit's
LICENSE is MIT, copyright 2025 Microsoft, and its README states that the
source code and models are MIT. The seven stock .pt prompts downloaded from
that same commit are pre-computed KV-cache voice embeddings:

- en-Carter_man
- en-Davis_man
- en-Emma_woman
- en-Frank_man
- en-Grace_woman
- en-Mike_man
- in-Samuel_man

Fork license: https://github.com/vibevoice-community/VibeVoice/blob/07cb79feadd2d3fd7f47530d4c964a12857936a0/LICENSE
Fork README: https://github.com/vibevoice-community/VibeVoice/blob/07cb79feadd2d3fd7f47530d4c964a12857936a0/README.md
Voice prompt path: demo/voices/streaming_model/

The fork gives a repository-level MIT statement but does not put a separate
license header on each prompt file. Zana does not bundle these .pt files;
file-level redistribution beyond the runtime download remains an unresolved
upstream question.

### Silero VAD model

livekit-plugins-silero==1.8.8 embeds silero_vad.onnx in its wheel and
voicebot/agent.py loads it through silero.VAD.load(). Zana does not fetch a
separate VAD file at runtime. The LiveKit plugin source is Apache-2.0; the
Silero model repository's license is MIT, copyright 2020-present Silero Team.
The exact Silero model revision embedded by the dependency is not identified
in the plugin source, so this notice does not claim a precise model-version
match beyond the pinned plugin version.

LiveKit source: https://github.com/livekit/agents/blob/livekit-agents@1.8.8/livekit-plugins/livekit-plugins-silero/livekit/plugins/silero/onnx_model.py
Silero license: https://github.com/snakers4/silero-vad/blob/v6.3/LICENSE

## Package pins in the runtime path

The relevant package pins are livekit-plugins-silero==1.8.8,
parakeet-mlx==0.5.3, speechbrain==1.1.2, and huggingface-hub==0.36.2 in
voicebot/requirements.txt, plus the VibeVoice fork commit above.

## Open redistribution questions

- The default Hugging Face model loaders follow mutable default revisions,
  and environment overrides can select arbitrary repositories. Re-verify
  the selected model terms before bundling weights or changing a release
  default.
- The VibeVoice model card declares MIT but also contains research-use
  guidance and a no-commercial-use recommendation. This repository does not
  make a legal determination about commercial redistribution.
- The Parakeet conversion points to an upstream model and training datasets.
  This notice records the stated CC BY 4.0 model terms but does not clear
  dataset rights.
- The VibeVoice prompt files have a fork-level MIT statement but no
  file-specific license notice. They remain runtime-only and are not released
  as bundled Zana assets.

Verification snapshot: 2026-11-24. Source references above are the upstream
primary sources checked for this change.
