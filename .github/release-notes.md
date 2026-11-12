## New: themes and extensions
Restyle YouTube Music and add small features, a bit like Spicetify for Spotify. Open **Themes & extensions** from the new palette button in YouTube Music's top bar, with **Ctrl+,** (Cmd+, on Mac), or from the tray.

- **Built-in themes:** AMOLED Black (true black) and Optimusic Violet (violet instead of red accents). They apply instantly, and the window stays open so you can try a few.
- **Hide upgrade prompts:** a separate switch that hides the "Upgrade" entry and "Try Premium" pop-ups. It doesn't touch ads.
- **Your own themes:** drop a `.css` file into the themes folder.
- **Extensions:** drop a `.js` file into the extensions folder. Each one is off until you turn it on, and Optimusic asks you to confirm first, because extensions run while you're signed in to Google.

Themes and extensions only ever run on music.youtube.com, never on Google's sign-in pages, and the YouTube Music page can only *open* the settings window, never change your settings. There's a sample theme and extension in the [`examples`](https://github.com/ImCYMBIOT/optimusic/tree/main/examples) folder.

## This release is Windows only
GitHub's build servers are unavailable on this account for now, so v0.4.0 was built for Windows only. **Linux and macOS stay on [v0.3.0](https://github.com/ImCYMBIOT/optimusic/releases/tag/v0.3.0)** until the next full release.

## Download (Windows)
- **`Optimusic-Windows-Setup.exe`**: installs just for you, no admin needed (recommended)
- **`Optimusic-Windows.msi`**: standard MSI installer

Already on v0.3.0? With updates set to Notify me, **Update to v0.4.0** appears in your tray menu.

The installers aren't code-signed yet, so SmartScreen may warn the first time: click **More info → Run anyway**.

Not affiliated with, endorsed by, or sponsored by Google, YouTube or Discord.
