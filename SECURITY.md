# Security Policy

## Supported Versions

Security fixes target the latest `main` branch and the newest published release.
Older prerelease builds are not guaranteed to receive patches.

## Reporting a Vulnerability

Please do not open a public issue for vulnerabilities.

GitHub private vulnerability reporting is not currently enabled for this
repository. Contact the maintainer through the [repository owner's GitHub
profile](https://github.com/maarco) to arrange a private reporting channel. Do
not include sensitive details in a public issue or comment.

Include:

- affected version or commit
- reproduction steps
- impact
- whether the issue involves microphone, accessibility, clipboard, screenshots,
  paste-at-cursor, terminal commands, AppleScript, key presses, MCP servers,
  updater, signing, or model downloads

## Privacy-Sensitive Areas

Treat these as high risk:

- microphone capture
- accessibility permissions
- paste-at-cursor automation
- clipboard and screenshot context
- optional rewrite provider requests
- Terminal Tool shell commands
- AppleScript app automation
- key presses sent to the active app
- configured MCP servers and their tools
- update checks, signing, and notarization
