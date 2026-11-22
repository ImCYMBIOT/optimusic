// Optimusic's built-in Synced lyrics (Themes & extensions → Extensions). Runs on music.youtube.com.
//
// When the Lyrics tab is open, asks the app (lyrics.rs) for the song's timed lyrics from LRCLIB.
// If there are some, they replace YouTube Music's plain text: the current line is highlighted,
// the list follows the song, and clicking a line jumps there. Otherwise YouTube Music's own
// lyrics stay, just easier to read. No polling loop of its own beyond a cheap 1.5s check; line
// changes are timed from the player's clock.
//
// Turned on again after being off: start() again. Turned off: stop() undoes everything.
(() => {
  if (window.__optimusicLyrics) {
    window.__optimusicLyrics.start();
    return;
  }

  const CSS = `
    .optimusic-synced > :not(#optimusic-lyrics) { display: none !important; }
    #optimusic-lyrics { padding: 12px 4px 45vh; }
    #optimusic-lyrics .ol-line {
      font-family: inherit; font-size: 26px; font-weight: 700; line-height: 1.35;
      margin: 0 0 20px; color: rgba(255, 255, 255, 0.36); cursor: pointer;
      transition: color 0.25s, opacity 0.25s; overflow-wrap: anywhere;
    }
    #optimusic-lyrics .ol-line.ol-past { color: rgba(255, 255, 255, 0.55); }
    #optimusic-lyrics .ol-line:hover { color: rgba(255, 255, 255, 0.8); }
    #optimusic-lyrics .ol-line.ol-on { color: #fff; }
    #optimusic-lyrics .ol-line.ol-gap { font-size: 20px; letter-spacing: 6px; }
    #optimusic-lyrics .ol-source { font-size: 12px; color: rgba(255, 255, 255, 0.45); margin-top: 28px; }
    .optimusic-plain ytmusic-description-shelf-renderer .description {
      font-size: 20px !important; line-height: 1.7 !important; font-weight: 500 !important;
      color: rgba(255, 255, 255, 0.88) !important;
    }
  `;

  const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
  const video = () => document.querySelector("video");
  const player = () => document.getElementById("movie_player");

  let on = false;
  let interval = null;
  let style = null;
  let box = null; // our lyrics list
  let tab = null; // YouTube Music's lyrics tab while it's showing ours
  let bound = null; // the <video> we listen to
  let song = ""; // the song `lines` belongs to
  let lines = null; // [{t, text}] or null (none found / not loaded)
  let request = 0; // ignores answers for a song that's no longer playing
  let retryAt = 0;
  let active = -1;
  let timer = null;
  let userScrolledAt = 0;

  /** YouTube Music's Lyrics tab, if it's open and visible. */
  function lyricsTab() {
    const t = document.querySelector('ytmusic-player-page ytmusic-tab-renderer[page-type="MUSIC_PAGE_TYPE_TRACK_LYRICS"]');
    return t && t.getClientRects().length ? t : null;
  }

  function current() {
    const md = navigator.mediaSession && navigator.mediaSession.metadata;
    const v = video();
    if (!md || !md.title || !v || !isFinite(v.duration) || v.duration <= 0) return null;
    return { title: md.title, artist: md.artist || "", album: md.album || null, duration: v.duration };
  }

  function check() {
    if (!on) return;
    const t = lyricsTab();
    const info = current();
    const key = info ? `${info.title}\u0001${info.artist}\u0001${Math.round(info.duration)}` : "";
    if (key !== song) {
      song = key;
      lines = null;
      retryAt = 0;
      request++;
      render();
    }
    if (!t) {
      detach();
      return;
    }
    if (t !== tab) {
      detach();
      tab = t;
    }
    if (info && !lines && Date.now() >= retryAt) load(info, key);
    render();
  }

  async function load(info, key) {
    const id = ++request;
    retryAt = Infinity; // one request at a time
    let found = null;
    try {
      found = await invoke("synced_lyrics", info);
      retryAt = Infinity; // LRCLIB answered; don't ask again for this song
    } catch {
      retryAt = Date.now() + 30000; // offline or LRCLIB down: try again later
    }
    if (id !== request || key !== song) return;
    lines = found && found.length ? found : null;
    active = -1;
    render();
  }

  /** Shows our list in the Lyrics tab, or styles YouTube Music's own lyrics if we have none. */
  function render() {
    if (!tab) return;
    if (!lines) {
      if (box) box.remove();
      tab.classList.remove("optimusic-synced");
      tab.classList.add("optimusic-plain");
      return;
    }
    if (!box || box.dataset.song !== song) {
      box = document.createElement("div");
      box.id = "optimusic-lyrics";
      box.dataset.song = song;
      lines.forEach((l, i) => {
        const el = document.createElement("div");
        el.className = l.text ? "ol-line" : "ol-line ol-gap";
        el.textContent = l.text || "♪ ♪ ♪";
        el.onclick = () => seek(i);
        box.append(el);
      });
      const src = document.createElement("div");
      src.className = "ol-source";
      src.textContent = "Synced lyrics from LRCLIB";
      box.append(src);
      box.addEventListener("wheel", () => (userScrolledAt = Date.now()), { passive: true });
      box.addEventListener("touchmove", () => (userScrolledAt = Date.now()), { passive: true });
      active = -1;
    }
    if (box.parentNode !== tab) tab.prepend(box);
    tab.classList.remove("optimusic-plain");
    tab.classList.add("optimusic-synced");
    bindVideo();
    sync(true);
  }

  function detach() {
    clearTimeout(timer);
    if (box) box.remove();
    if (tab) tab.classList.remove("optimusic-synced", "optimusic-plain");
    tab = null;
    active = -1;
  }

  function seek(i) {
    const p = player();
    if (!p || !lines) return;
    userScrolledAt = 0;
    p.seekTo(lines[i].t, true);
    const v = video();
    if (v && v.paused) p.playVideo();
  }

  function bindVideo() {
    const v = video();
    if (!v || v === bound) return;
    if (bound) for (const e of EVENTS) bound.removeEventListener(e, onVideo);
    bound = v;
    for (const e of EVENTS) v.addEventListener(e, onVideo);
  }
  const EVENTS = ["timeupdate", "seeked", "play", "pause", "ratechange", "loadedmetadata", "durationchange"];
  function onVideo(e) {
    if (e.type === "loadedmetadata" || e.type === "durationchange") check();
    else sync(e.type === "seeked");
  }

  /** Highlights the line being sung and sets a timer for the next one. */
  function sync(jump) {
    clearTimeout(timer);
    const v = video();
    if (!on || !box || !lines || !box.isConnected || !v) return;
    const now = v.currentTime + 0.1; // a hair early feels in time
    let lo = 0, hi = lines.length - 1, i = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (lines[mid].t <= now) { i = mid; lo = mid + 1; } else hi = mid - 1;
    }
    if (i !== active) {
      const els = box.children;
      for (let k = 0; k < lines.length; k++) {
        els[k].classList.toggle("ol-on", k === i);
        els[k].classList.toggle("ol-past", k < i);
      }
      active = i;
      if (Date.now() - userScrolledAt > 4000) {
        const target = els[Math.max(i, 0)];
        target.scrollIntoView({ block: "center", behavior: jump ? "auto" : "smooth" });
      }
    }
    const next = lines[i + 1];
    if (next && !v.paused) {
      const ms = ((next.t - now) / (v.playbackRate || 1)) * 1000;
      timer = setTimeout(() => sync(false), Math.max(ms, 30));
    }
  }

  // Tab clicks open the Lyrics tab; react straight away instead of at the next check
  const onClick = () => setTimeout(check, 150);

  window.__optimusicLyrics = {
    start() {
      if (on) return;
      on = true;
      style = document.createElement("style");
      style.id = "optimusic-lyrics-style";
      style.textContent = CSS;
      (document.head || document.documentElement).append(style);
      document.addEventListener("click", onClick, true);
      interval = setInterval(check, 1500);
      check();
    },
    stop() {
      if (!on) return;
      on = false;
      clearInterval(interval);
      document.removeEventListener("click", onClick, true);
      if (bound) for (const e of EVENTS) bound.removeEventListener(e, onVideo);
      bound = null;
      detach();
      box = null;
      if (style) style.remove();
      song = "";
      lines = null;
      request++;
    },
  };
  window.__optimusicLyrics.start();
})();
