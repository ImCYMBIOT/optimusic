## Themes and extensions, now on every platform
v0.4.0 came out for Windows only. **v0.4.1 brings the same features to Linux and macOS**, and all three platforms are back in step.

If you're on Windows and already have v0.4.0, there's nothing new for you in this one; it just matches the version number.

### What's in it
- **Themes & extensions window:** open it from the palette button in YouTube Music's top bar, with **Ctrl+,** (Cmd+, on Mac), or from the tray. Changes apply instantly.
- **Built-in themes:** AMOLED Black and Optimusic Violet, plus a **Hide upgrade prompts** switch (it doesn't touch ads).
- **Your own themes and extensions:** drop `.css` or `.js` files into their folders. Extensions are off until you turn them on, and Optimusic asks you to confirm first. Samples are in the [`examples`](https://github.com/ImCYMBIOT/optimusic/tree/main/examples) folder.

Themes and extensions only ever run on music.youtube.com, never on Google's sign-in pages.

## Downloads
| Platform | File |
|---|---|
| Windows | `Optimusic-Windows-Setup.exe` (recommended, no admin needed) or `Optimusic-Windows.msi` |
| macOS *(untested)* | `Optimusic-macOS.dmg`, for Apple silicon and Intel |
| Ubuntu / Debian | `Optimusic-Linux.deb`: `sudo apt install ./Optimusic-Linux.deb` |
| Other Linux | `Optimusic-Linux.AppImage`: `chmod +x` it and run |

The same files are also here with the version in their names. Files ending in `.sig`, `.tar.gz` and `latest.json` are for the updater; you don't need them.

Already on v0.3.0 or later? With updates set to Notify me, **Update to v0.4.1** appears in your tray menu.

The installers aren't code-signed yet: on Windows, SmartScreen may warn the first time (More info → Run anyway); on macOS, open System Settings → Privacy & Security and click Open Anyway.

Not affiliated with, endorsed by, or sponsored by Google, YouTube, Apple or Discord.
