// Injected into music.youtube.com before page scripts run.
// Three jobs: (1) expose window.__optimusic.control() so Rust can drive the player,
// (2) report now-playing info to Rust, event-driven and throttled — no polling loop
// beyond a cheap 2s check for the <video> element being replaced, (3) apply the theme and
// run extensions that Rust hands over (see customize.rs). Only on music.youtube.com, so
// none of it ever runs on Google's sign-in pages.
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

    /** Replaces the theme stylesheet; an empty string removes it. */
    setTheme(css) {
      const apply = () => {
        let el = document.getElementById("optimusic-theme");
        if (!el) {
          el = document.createElement("style");
          el.id = "optimusic-theme";
        }
        el.textContent = css;
        // Last in <head> so it wins over the page's own styles
        (document.head || document.documentElement).appendChild(el);
      };
      if (document.head) apply();
      else document.addEventListener("DOMContentLoaded", apply, { once: true });
    },

    /** Runs an extension once per page load; errors stay inside it. */
    runExtension(name, fn) {
      if (ran.has(name)) return;
      ran.add(name);
      try {
        fn();
      } catch (e) {
        console.error(`[Optimusic extension ${name}]`, e);
      }
    },
  };
  const ran = new Set();

  function snapshot() {
    const md = navigator.mediaSession && navigator.mediaSession.metadata;
    const v = video();
    if (!md || !v || !md.title) return null;
    const art = md.artwork && md.artwork.length ? md.artwork[md.artwork.length - 1].src : null;
    const p = player();
    const data = p && p.getVideoData ? p.getVideoData() : null;
    return {
      title: md.title || "",
      artist: md.artist || "",
      album: md.album || null,
      art,
      videoId: (data && data.video_id) || null,
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

  // A "Themes & extensions" button in YouTube Music's top bar (and Ctrl+, / Cmd+,). It can only
  // open the settings window; changing settings happens in that window, not in this page.
  const openSettings = () => invoke("open_settings");
  function addSettingsButton() {
    const right = document.querySelector("ytmusic-nav-bar #right-content");
    if (!right || right.querySelector("#optimusic-settings")) return;
    const btn = document.createElement("button");
    btn.id = "optimusic-settings";
    btn.title = "Themes & extensions (Ctrl+,)";
    btn.setAttribute("aria-label", "Themes & extensions");
    btn.style.cssText =
      "all:unset;cursor:pointer;width:40px;height:40px;display:inline-flex;align-items:center;" +
      "justify-content:center;border-radius:50%;color:inherit;margin-right:4px;flex-shrink:0";
    btn.onmouseenter = () => (btn.style.background = "rgba(255,255,255,.1)");
    btn.onmouseleave = () => (btn.style.background = "none");
    btn.onclick = openSettings;
    // A paint palette, drawn with DOM calls (the page blocks innerHTML via Trusted Types)
    const ns = "http://www.w3.org/2000/svg";
    const svg = document.createElementNS(ns, "svg");
    for (const [k, v] of Object.entries({ width: 22, height: 22, viewBox: "0 0 24 24", fill: "none",
      stroke: "currentColor", "stroke-width": 1.8, "stroke-linecap": "round", "stroke-linejoin": "round" }))
      svg.setAttribute(k, v);
    const path = document.createElementNS(ns, "path");
    path.setAttribute("d", "M12 3a9 9 0 1 0 0 18c1.1 0 1.8-.8 1.8-1.7 0-.5-.2-.9-.5-1.2-.3-.3-.5-.7-.5-1.2 " +
      "0-.9.8-1.7 1.8-1.7H17a4 4 0 0 0 4-4C21 6.6 17 3 12 3z");
    svg.append(path);
    for (const [cx, cy] of [[7.5, 10.5], [10.5, 7], [15, 7.5]]) {
      const dot = document.createElementNS(ns, "circle");
      dot.setAttribute("cx", cx); dot.setAttribute("cy", cy); dot.setAttribute("r", 1.2);
      dot.setAttribute("fill", "currentColor"); dot.setAttribute("stroke", "none");
      svg.append(dot);
    }
    btn.append(svg);
    right.prepend(btn);
  }
  document.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === ",") {
      e.preventDefault();
      openSettings();
    }
  });

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
  // The same cheap 2s check also (re)adds the button, as YouTube Music builds its top bar late
  setInterval(() => {
    bind();
    addSettingsButton();
  }, 2000);
})();
