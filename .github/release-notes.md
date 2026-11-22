## New: taskbar controls on Windows
Hover over Optimusic in the taskbar and you get **Previous, Play/Pause and Next** buttons right under the preview. While a song is playing (not a music video), the preview shows its **album art**, and the preview title and tray tooltip show what's playing.

Thanks to [@geeknoid](https://github.com/geeknoid) for contributing this ([#6](https://github.com/ImCYMBIOT/optimusic/pull/6)).

This feature first shipped in v0.4.2, which was held back after Microsoft Defender flagged it by mistake. v0.4.3 is a fresh build with the same feature plus the security fixes below, tested before release.

## Security hardening
- **Safer link handling:** only real Google and YouTube pages open inside the app. Look-alike addresses such as `accounts.google.evil.com` now open in your normal browser instead.
- **SmartScreen stays on** inside the app, along with the browser engine's security updates.
- **The Themes & extensions window has a content security policy**, and extensions are re-checked against the extensions folder before they run.

## Downloads
| Platform | File |
|---|---|
| Windows | `Optimusic-Windows-Setup.exe` (recommended, no admin needed) or `Optimusic-Windows.msi` |
| macOS *(untested)* | `Optimusic-macOS.dmg`, for Apple silicon and Intel |
| Ubuntu / Debian | `Optimusic-Linux.deb`: `sudo apt install ./Optimusic-Linux.deb` |
| Other Linux | `Optimusic-Linux.AppImage`: `chmod +x` it and run |

The same files are also here with the version in their names. Files ending in `.sig`, `.tar.gz` and `latest.json` are for the updater; you don't need them.

Already on v0.3.0 or later? With updates set to Notify me, **Update to v0.4.3** appears in your tray menu.

The installers aren't code-signed yet: on Windows, SmartScreen may warn the first time (More info → Run anyway); on macOS, open System Settings → Privacy & Security and click Open Anyway.

[Code signing policy](https://github.com/ImCYMBIOT/optimusic#code-signing-policy) · [Privacy policy](https://github.com/ImCYMBIOT/optimusic/blob/main/PRIVACY.md)

Not affiliated with, endorsed by, or sponsored by Google, YouTube, Apple, Microsoft or Discord.
