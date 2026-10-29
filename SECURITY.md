# Security policy

## Reporting a vulnerability
Please **don't open a public issue** for security problems. Report them privately instead:
[Security → Report a vulnerability](https://github.com/ImCYMBIOT/optimusic/security/advisories/new).

Include what you found, how to reproduce it, and which version and platform you're on.
I'll reply as soon as I can, and credit you in the fix unless you'd rather stay anonymous.

## Supported versions
Only the [latest release](https://github.com/ImCYMBIOT/optimusic/releases/latest) gets fixes.

## How releases are protected
- Release builds run on GitHub Actions, never on a personal machine.
- Updates are signed; installed copies refuse any update that isn't signed with the project's key.
- The signing key is only available to release builds of `v*` tags, and only after the
  maintainer approves each one. Builds from pull requests never see it.
- Third-party build actions are pinned to exact commits.
- Optimusic talks to music.youtube.com and Google sign-in, GitHub (to check for updates, unless
  updates are set to Off), and, if it's running, the local Discord app. Nothing else, and no
  analytics.
