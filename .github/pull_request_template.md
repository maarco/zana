## Summary

- 

## Checks

- [ ] `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [ ] `cargo check -p Zana-app --manifest-path src-tauri/Cargo.toml --locked`
- [ ] `cargo test -p Zana-app --manifest-path src-tauri/Cargo.toml --locked`
- [ ] `cargo clippy -p Zana-app --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`

## Manual QA

- [ ] Not needed
- [ ] Fn/Ctrl recording
- [ ] microphone permission
- [ ] accessibility permission
- [ ] paste-at-cursor
- [ ] signing/notarization
- [ ] visible UI

## Privacy Impact

- [ ] no privacy impact
- [ ] updates `PRIVACY.md`
