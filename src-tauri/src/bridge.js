// Injected into music.youtube.com before page scripts run.
// Two jobs: (1) expose window.__optimusic.control() so Rust can drive the player,
// (2) report now-playing info to Rust, event-driven and throttled — no polling loop
// beyond a cheap 2s check for the <video> element being replaced.
(() => {
  if (window.__optimusic || location.hostname !== "music.youtube.com") return;

  const invoke = (cmd, args) => {
    const t = window.__TAURI_INTERNALS__;
    if (t) t.invoke(cmd, args).catch(() => {});
  };
  const player = () => document.getElementById("movie_player");
  const video = () => document.querySelector("video");

  window.__optimusic = {
    control(action) {
      const p = player();
      const v = video();
      if (!p) return;
      const [cmd, arg] = String(action).split(":");
      switch (cmd) {
        case "play": p.playVideo(); break;
        case "pause": p.pauseVideo(); break;
        case "toggle": (v && !v.paused) ? p.pauseVideo() : p.playVideo(); break;
        case "next": p.nextVideo(); break;
        case "prev": p.previousVideo(); break;
        case "seekTo": p.seekTo(parseFloat(arg), true); break;
        case "seekBy": p.seekTo(p.getCurrentTime() + parseFloat(arg), true); break;
      }
      setTimeout(() => push(true), 300);
    },
  };

  function snapshot() {
    const md = navigator.mediaSession && navigator.mediaSession.metadata;
    const v = video();
    if (!md || !v || !md.title) return null;
    const art = md.artwork && md.artwork.length ? md.artwork[md.artwork.length - 1].src : null;
    return {
      title: md.title || "",
      artist: md.artist || "",
      album: md.album || null,
      art,
      duration: isFinite(v.duration) ? v.duration : 0,
      position: v.currentTime || 0,
      playing: !v.paused,
    };
  }

  // Send immediately on track/state change; otherwise at most every 5s
  // (OS media widgets extrapolate the progress bar themselves).
  let lastKey = "";
  let lastSent = 0;
  function push(force) {
    const s = snapshot();
    if (!s) return;
    const key = s.title + "|" + s.artist + "|" + s.playing;
    const now = Date.now();
    if (force || key !== lastKey || now - lastSent > 5000) {
      lastKey = key;
      lastSent = now;
      invoke("track_update", { info: s });
    }
  }

  let bound = null;
  function bind() {
    const v = video();
    if (!v || v === bound) return;
    bound = v;
    for (const e of ["play", "pause", "loadedmetadata", "seeked", "durationchange", "ended"]) {
      v.addEventListener(e, () => push(true));
    }
    v.addEventListener("timeupdate", () => push(false));
  }
  setInterval(bind, 2000);
})();
