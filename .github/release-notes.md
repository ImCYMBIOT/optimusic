## New: synced lyrics
YouTube Music on PC only shows plain lyrics. Turn on **Synced lyrics** (Themes & extensions → Extensions, under "Built in") and the Lyrics tab:

- **highlights the line being sung** and scrolls along with the song (scroll yourself and it waits a few seconds before following again)
- **jumps to any line you click**
- keeps YouTube Music's own lyrics, in bigger, easier-to-read text, for songs that don't have timed lyrics

Timed lyrics come from [LRCLIB](https://lrclib.net), a free, open lyrics database. It's **off by default**, because it sends the song's title, artist and length to lrclib.net. The lookup is done by the app itself: YouTube Music's page can only ask for lyrics while the switch is on, and can't turn it on.

Also in this release: everything from v0.4.3 (taskbar controls on Windows and the security hardening).

## Downloads
| Platform | File |
|---|---|
| Windows | `Optimusic-Windows-Setup.exe` (recommended, no admin needed) or `Optimusic-Windows.msi` |
| macOS *(untested)* | `Optimusic-macOS.dmg`, for Apple silicon and Intel |
| Ubuntu / Debian | `Optimusic-Linux.deb`: `sudo apt install ./Optimusic-Linux.deb` |
| Other Linux | `Optimusic-Linux.AppImage`: `chmod +x` it and run |

The same files are also here with the version in their names. Files ending in `.sig`, `.tar.gz` and `latest.json` are for the updater; you don't need them.

Already on v0.3.0 or later? With updates set to Notify me, **Update to v0.4.4** appears in your tray menu.

The installers aren't code-signed yet: on Windows, SmartScreen may warn the first time (More info → Run anyway); on macOS, open System Settings → Privacy & Security and click Open Anyway.

[Code signing policy](https://github.com/ImCYMBIOT/optimusic#code-signing-policy) · [Privacy policy](https://github.com/ImCYMBIOT/optimusic/blob/main/PRIVACY.md)

Not affiliated with, endorsed by, or sponsored by Google, YouTube, Apple, Microsoft or Discord.
