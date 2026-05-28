# Zana interactive doc site summary

Generated for this repository.

## chosen structure

- primary: docs portal
- secondary: inventory explorer
- interaction: level 2, light notes and state markers

## what it covers

- live Tauri runtime and command surfaces
- local Whisper first, optional rewrite, raw transcript fallback
- frontend windows and orb surfaces under `src-ui`
- hooks and plugin runtime boundaries
- release workflow, scripts, bundle identity, and manual QA gates
- stale docs and source traps that can mislead future work

## highest-risk findings captured in the site

- `src-tauri` is the active workspace; root `src/` is not in the root workspace.
- `PRIVACY.md` was updated to disclose opt-in rewrite behavior.
- release metadata now points at the intended public repository.
- egui migration docs are labeled as historical.
- plugin docs overstate live dynamic/plugin marketplace support.
- build/test gates do not prove visible tray/orb runtime or macOS permissions.

## run

```bash
cd interactive-doc-site
npm run serve
```

Then open `http://127.0.0.1:8799`.
