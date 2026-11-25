# macOS release evidence

This note records the entitlement and stapling decisions for issue #34. It is
evidence for the release workflow on this branch, not proof that a signed
release has been published or installed.

## Entitlement decisions

The app explicitly disables App Sandbox. The four App Sandbox-only keys are
removed because they do not belong in this unsandboxed hardened-runtime
profile:

- `com.apple.security.device.microphone`: removed. The app uses the hardened
  runtime microphone entitlement below; the App Sandbox microphone key is not
  applicable while `app-sandbox` is false. The focused contract test asserts its
  absence.
- `com.apple.security.files.user-selected.read-write`: removed. App Sandbox
  file access is not part of this unsandboxed app. The focused contract test
  asserts its absence.
- `com.apple.security.files.bookmarks.app-scope`: removed. App Sandbox
  security-scoped bookmarks are not part of this unsandboxed app. The focused
  contract test asserts its absence.
- `com.apple.security.network.client`: removed. App Sandbox network access is
  not part of this unsandboxed app. The focused contract test asserts its
  absence.

The following entitlements remain because their necessity is not proven by the
checks permitted for this PR:

- `com.apple.security.cs.allow-jit`: kept. No signed or notarized runtime
  experiment was run in this PR that proves it can be removed. The focused
  contract test asserts retention until that experiment exists.
- `com.apple.security.cs.allow-unsigned-executable-memory`: kept. No signed or
  notarized runtime experiment was run in this PR that proves it can be
  removed. The focused contract test asserts retention until that experiment
  exists.
- `com.apple.security.cs.allow-dyld-environment-variables`: kept. No signed or
  notarized runtime experiment was run in this PR that proves it can be
  removed. The focused contract test asserts retention until that experiment
  exists.
- `com.apple.security.device.audio-input`: kept. This is the hardened-runtime
  microphone entitlement required by the issue's release contract. The focused
  contract test asserts retention; live signed microphone verification remains
  open.
- `com.apple.security.app-sandbox`: kept as `false` to make the unsandboxed
  profile explicit. The focused contract test asserts the value.

## Stapling contract

For a Developer ID release, `.github/workflows/release.yml` now performs these
operations in order:

1. Sign and verify `Zana.app`.
2. Zip the signed app and submit that ZIP to `notarytool`.
3. Staple and validate `Zana.app`, then run `spctl` on the app.
4. Copy the already-stapled app into the DMG and create the DMG.
5. Sign, notarize, staple, and validate the DMG.
6. Mount the DMG and validate the copied `Zana.app` ticket and executable
   assessment.

The contract test `mac_release_staples_app_before_dmg_and_validates_both`
checks the source order and the presence of both app and DMG validation.

## Remaining acceptance checks

These checks were intentionally not run here because this task forbids reading
secrets, triggering signing or notarization, changing live workflow settings,
and starting a publishing run:

- Run a normal Developer ID release candidate with the entitlement file from
  this branch and confirm Apple returns `Accepted` for both notarization
  submissions.
- Confirm `xcrun stapler validate` passes for the standalone `Zana.app`, the
  `Zana.app` copied out of the DMG, and the DMG itself on a macOS runner.
- Install the notarized app from the DMG and exercise Voicebot microphone input
  plus its child-process lifecycle.
- Remove each retained hardened-runtime exception in an isolated release
  candidate, repeat the signed/notarized microphone and child-process test, and
  remove only exceptions that pass that experiment.

The local helper `scripts/sign-and-notarize.sh` remains outside this focused
workflow change. It should not be treated as evidence for the GitHub release
path until its separate local stapling path is brought into parity.
