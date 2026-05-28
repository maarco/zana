# Security Policy

## Supported Versions

Security fixes target the latest `main` branch and the newest published release.
Older prerelease builds are not guaranteed to receive patches.

## Reporting a Vulnerability

Please do not open a public issue for vulnerabilities.

Report privately through GitHub Security Advisories after the repository is
published. If advisories are not available yet, contact the maintainer directly
through the GitHub profile listed on the repository.

Include:

- affected version or commit
- reproduction steps
- impact
- whether the issue involves microphone, accessibility, clipboard, screenshots,
  rewrite provider calls, updater, signing, or model downloads

## Privacy-Sensitive Areas

Treat these as high risk:

- microphone capture
- accessibility permissions and global hotkeys
- paste-at-cursor automation
- clipboard and screenshot context
- optional rewrite provider requests
- update checks, signing, and notarization
