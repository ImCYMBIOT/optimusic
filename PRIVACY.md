# Privacy policy

Optimusic has no analytics, no telemetry and no accounts of its own. It never sends anything to
its developer. This page lists every connection the app makes, and how to turn off the optional
ones.

## What Optimusic connects to

| Connection | When | What is sent | How to turn it off |
|---|---|---|---|
| **YouTube Music and Google** (music.youtube.com, Google sign-in, Google's image servers) | Whenever the app is open | The same things a browser sends when you use YouTube Music. This is Google's service and is covered by [Google's privacy policy](https://policies.google.com/privacy). | It's what the app is for |
| **GitHub** (update check) | At start-up and every 6 hours | A request for one small file listing the newest version. GitHub sees your IP address, as for any download ([GitHub's privacy statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement)). Nothing about you or your listening is sent. | Tray → Updates → **Off** |
| **GitHub** (update download) | Only when you click "Update to vX", or at start-up if you chose "Install automatically" | A download request for the new version | Tray → Updates → **Off** |
| **Album art** (Google's image servers) | When a song plays, on Windows | A request for the cover image, for the taskbar preview and the system media overlay | Not optional on Windows; it's the same image the page already loads |
| **Discord** (the Discord app on your own computer) | While music plays and Discord is running | The song title, artist, album, cover image link, progress and a link to the song, which Discord then shows on your profile ([Discord's privacy policy](https://discord.com/privacy)) | Tray → **Show on Discord** |

Optimusic doesn't contact anything else.

## What stays on your computer
- **Your sign-in:** you sign in on Google's own page. Optimusic doesn't read or store your
  password. The sign-in cookies are kept in the app's own browser profile on your computer, the
  way a browser keeps them.
- **Your settings:** theme, extensions, update and Discord choices, and window position are
  files in the app's settings folder. They are never uploaded.

## Themes and extensions
Themes are plain CSS and can't send data anywhere. **Extensions are scripts written by other
people** and run inside YouTube Music while you're signed in, so an extension could read or send
data. They are off by default, and Optimusic asks before turning one on. Only use extensions you
trust.

## The website
[imcymbiot.github.io/optimusic](https://imcymbiot.github.io/optimusic/) is hosted on GitHub
Pages. It uses no cookies, analytics or third-party scripts, and its fonts are served from the
site itself. GitHub, as the host, sees visitors' IP addresses.

## Changes and contact
Changes to this policy are made in this file, so its
[history](https://github.com/ImCYMBIOT/optimusic/commits/main/PRIVACY.md) shows every edit.
Questions: [open an issue](https://github.com/ImCYMBIOT/optimusic/issues).
