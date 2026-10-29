> **Coming from v0.2.x or earlier?** Install this version by hand one last time (downloads below). From v0.3.0 on, Optimusic can update itself.

## New: updates your way
Choose in the tray (menu bar on macOS) → **Updates**:
- **Off**: never checks, no update requests at all
- **Notify me** (default): checks at start-up and every 6 hours. An **Update to vX** item appears in the tray menu, and nothing downloads until you click it
- **Install automatically**: installs new versions the next time you start Optimusic, before any music plays

Every update is signed, and Optimusic refuses any update that isn't.

## New: macOS (untested, help wanted)
There's now a Mac version for Apple silicon and Intel Macs (macOS 10.15+). **I don't have a Mac yet, so it hasn't been tested on one.** It builds cleanly and should work; I'll test it as soon as I can. If you try it, please [open an issue](https://github.com/ImCYMBIOT/optimusic/issues) saying whether it worked.

The app isn't signed by Apple yet, so macOS blocks it the first time: open **System Settings → Privacy & Security** and click **Open Anyway**. If it says the app "is damaged", run `xattr -dr com.apple.quarantine /Applications/Optimusic.app` in Terminal. Full steps are in the [README](https://github.com/ImCYMBIOT/optimusic#install-macos--untested-help-wanted).

## Also
- Builds for every platform now come from GitHub Actions, never a personal machine
- Quitting from the app menu (Cmd+Q on Mac) now really quits instead of hiding to the tray

## Downloads
| Platform | File |
|---|---|
| Windows | `Optimusic_0.3.0_x64-setup.exe` (recommended, no admin needed) or `Optimusic_0.3.0_x64_en-US.msi` |
| macOS | `Optimusic_0.3.0_universal.dmg` |
| Ubuntu / Debian | `Optimusic_0.3.0_amd64.deb`: `sudo apt install ./Optimusic_0.3.0_amd64.deb` |
| Other Linux | `Optimusic_0.3.0_amd64.AppImage`: `chmod +x` it and run |

Files ending in `.sig` and `latest.json` are for the updater; you don't need them.

Windows installers aren't code-signed yet, so SmartScreen may warn the first time: click **More info → Run anyway**.

Not affiliated with, endorsed by, or sponsored by Google, YouTube, Apple or Discord.
