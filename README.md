<p align="center"><img src="src-tauri/icons/128x128.png" width="96" alt="Optimusic icon"></p>

# Optimusic

**An unofficial, lightweight YouTube Music desktop app for Windows and Linux.**

Optimusic loads music.youtube.com in your system's own webview (WebView2 on Windows, WebKitGTK
on Linux) instead of shipping a copy of Chromium like Electron apps do. It adds the desktop
features a browser tab is missing: media keys, a tray player, and background playback. It also
tunes the webview to use much less memory.

> Optimusic is an independent project, not affiliated with, endorsed by, or sponsored by Google
> or YouTube. YouTube Music is a trademark of Google LLC. You sign in to and use the real
> YouTube Music website, under its terms.

## Why Optimusic
Memory on Windows, measured with `bench.ps1` (private working set of the whole process tree,
the same number Task Manager shows):

| | Idle | Playing | Hidden to tray, playing |
|---|---|---|---|
| **Optimusic v0.1.0** | **~250 MB** | **~200 MB** | **~150 MB** |
| Optimusic before memory tuning | 296 MB | 297 MB | 380 MB |
| [YTMDesktop](https://github.com/ytmdesktop/ytmdesktop) (Electron) | 343 MB | 413 MB | 343 MB |
| Firefox with a YouTube Music tab | 766 MB | 758 MB | 802 MB |

Hidden to the tray with nothing playing, Optimusic drops to about 45 MB.

Your numbers will vary with your PC, your library and what's on screen.

## Features
- **Media keys and OS media controls:** the Windows media overlay (SMTC), MPRIS on Linux
  (GNOME/KDE media widget), Now Playing on macOS
- **Tray player:** now playing, play/pause, next, previous, show/hide, quit
- **Discord Rich Presence:** "Listening to Optimusic" with the song, artist, cover art, a
  progress bar, and "Listen on YouTube Music" and "Get Optimusic" buttons. Cleared when you pause, idle when Discord
  isn't running, and switchable from the tray ("Show on Discord")
- **Background playback:** closing the window hides it to the tray and the music keeps playing
- **Low-memory background mode:** while hidden or minimized, the webview stops rendering and trims its memory
- **Single instance:** launching it again just brings the window back
- Remembers window size and position
- External links open in your normal browser; Google sign-in stays in the app
- **Locked down:** the YouTube Music page can call exactly one app command (`track_update`), nothing else

## Install (Windows)
Download an installer from the [latest release](https://github.com/ImCYMBIOT/optimusic/releases/latest):
- `Optimusic_x.y.z_x64-setup.exe`: installs just for you, no admin needed
- `Optimusic_x.y.z_x64_en-US.msi`: standard MSI installer

WebView2 is built into Windows 10 and 11; the installer downloads it if it's missing. The
installers aren't code-signed yet, so SmartScreen may warn the first time (More info → Run anyway).

## Build from source

### Windows
Needs [Rust](https://rustup.rs) and the Tauri CLI (`cargo install tauri-cli --version "^2"`).

```powershell
cd src-tauri
cargo run                 # dev
cargo tauri build         # .msi + setup .exe in target/release/bundle/
```

### Ubuntu

```bash
# Tauri build deps
sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libayatana-appindicator3-dev \
  librsvg2-dev libdbus-1-dev build-essential pkg-config

# Codecs WebKitGTK needs for YouTube audio (important — without these playback fails)
sudo apt install gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav

# Rust, if you don't have it
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Tauri CLI (for bundling)
cargo install tauri-cli --version "^2"
```

```bash
cd src-tauri
cargo run                 # dev
cargo tauri build         # .deb + AppImage in target/release/bundle/
```

## How the memory savings work (Windows)
Almost all of the memory is WebView2, not the Rust app (which uses about 5 MB). Optimusic tunes
it in two ways (see `BROWSER_ARGS` in `main.rs`):
- **Lean WebView2 flags:** Chromium's low-end-device mode; the GPU, network and audio services
  run inside the browser process; Edge services (translate, SmartScreen, sync, component
  updates…) are off. That's 9 processes down to 5.
- **Background mode:** when hidden to the tray or minimized, the webview is marked invisible and
  WebView2's Low memory target is set, so it trims down while the music keeps playing.

To experiment, `OPTIMUSIC_BROWSER_ARGS` replaces the whole flag string. For example, adding
`--disable-gpu --disable-gpu-compositing` saves about 20 MB more while the window is open, but uses more CPU.

Benchmark any app's whole process tree (results go to `bench-results.csv`):

```powershell
powershell -ExecutionPolicy Bypass -File .\bench.ps1 -App tauri -Phase playing -Seconds 120
```

## Troubleshooting
- **Blank window or rendering glitches on Windows:** the flags are the first suspect; run with
  `OPTIMUSIC_BROWSER_ARGS="--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection"` (the Tauri defaults).
- **Blank / white window on Linux (often NVIDIA):** `WEBKIT_DISABLE_DMABUF_RENDERER=1 optimusic`
- **Google says "this browser may not be secure" at sign-in:** try a different user agent, e.g.
  `OPTIMUSIC_USER_AGENT="Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15" optimusic`
- **Nothing shows on Discord:** turn on Discord's User Settings → Activity Privacy → "Share
  your activity status", check "Show on Discord" in the tray menu, and make sure music is playing
  (presence is cleared while paused).
- **Tray icon missing on GNOME:** enable the "AppIndicator and KStatusNotifierItem Support" extension.

## Project layout
```
src-tauri/
  src/main.rs          window, tray, media controls, navigation policy, WebView2 tuning
  src/discord.rs       Discord Rich Presence, on its own thread
  src/bridge.js        injected into the page: player control + now-playing reports
  capabilities/        IPC allow-list for music.youtube.com
  tauri.conf.json
bench.ps1              Windows CPU/memory benchmark for a whole process tree
```

## Roadmap
- Global hotkeys for desktops without MPRIS
- Mini-player window, desktop notifications on track change
- Last.fm scrobbling

## License
MIT, see [LICENSE](LICENSE).
