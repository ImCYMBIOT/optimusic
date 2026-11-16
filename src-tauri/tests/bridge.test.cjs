const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");
const { test } = require("node:test");
const vm = require("node:vm");

const source = readFileSync(join(__dirname, "..", "src", "bridge.js"), "utf8");

function bridge() {
  const messages = [];
  const listeners = {};
  const parent = { parentElement: null, style: {} };
  const video = {
    isConnected: true,
    paused: false,
    ended: false,
    readyState: 4,
    videoWidth: 1920,
    videoHeight: 1080,
    duration: 180,
    currentTime: 10,
    parentElement: parent,
    style: {},
    rect: { width: 640, height: 360 },
    getBoundingClientRect() { return this.rect; },
    addEventListener(name, callback) { listeners[name] = callback; },
  };
  const metadata = {
    title: "Track",
    artist: "Artist",
    artwork: [{ src: "https://lh3.googleusercontent.com/cover" }],
  };
  let tick;
  vm.runInNewContext(source, {
    location: { hostname: "music.youtube.com" },
    window: {
      __TAURI_INTERNALS__: {
        invoke(command, args) {
          messages.push({ command, info: args.info });
          return Promise.resolve();
        },
      },
    },
    document: {
      querySelector: selector => selector === "video" ? video : null,
      getElementById: () => null,
      addEventListener() {},
    },
    navigator: { mediaSession: { metadata } },
    getComputedStyle: el => ({
      display: "block", visibility: "visible", opacity: "1", ...el.style,
    }),
    setInterval: callback => { tick = callback; },
    Date: { now: () => 1000 },
  });
  return { video, parent, metadata, listeners, messages, tick: () => tick() };
}

test("visible, active video uses the native preview", () => {
  const app = bridge();
  app.tick();
  assert.equal(app.messages.at(-1).info.video_playing, true);
});

test("paused, audio-only, or hidden video uses artwork", () => {
  for (const change of [
    app => { app.video.paused = true; },
    app => { app.video.ended = true; },
    app => { app.video.videoWidth = 0; },
    app => { app.video.rect.width = 0; },
    app => { app.parent.style.display = "none"; },
    app => { app.parent.style.visibility = "hidden"; },
    app => { app.parent.style.opacity = "0"; },
  ]) {
    const app = bridge();
    change(app);
    app.tick();
    assert.equal(app.messages.at(-1).info.video_playing, false);
  }
});

test("unavailable video state remains unknown", () => {
  for (const change of [
    app => { app.video.readyState = 0; },
    app => { app.video.isConnected = false; },
  ]) {
    const app = bridge();
    change(app);
    app.tick();
    assert.equal(app.messages.at(-1).info.video_playing, null);
  }
});

test("mode and artwork changes bypass playback update throttling", () => {
  const app = bridge();
  app.tick();
  app.parent.style.display = "none";
  app.tick();
  assert.equal(app.messages.length, 2);
  assert.equal(app.messages.at(-1).info.video_playing, false);
  app.metadata.artwork[0].src = "https://lh3.googleusercontent.com/other-cover";
  app.tick();
  assert.equal(app.messages.length, 3);
  assert.equal(app.messages.at(-1).info.art, app.metadata.artwork[0].src);
  app.parent.style.display = "block";
  app.tick();
  assert.equal(app.messages.at(-1).info.video_playing, true);
  app.tick();
  assert.equal(app.messages.length, 4);
});

test("video metadata changes immediately report a switch to video", () => {
  const app = bridge();
  app.video.videoWidth = 0;
  app.tick();
  app.video.videoWidth = 1920;
  app.listeners.resize();
  assert.equal(app.messages.length, 2);
  assert.equal(app.messages.at(-1).info.video_playing, true);
});

test("progress ticks reuse the last video check instead of re-walking the page", () => {
  const app = bridge();
  app.tick();
  assert.equal(app.messages.at(-1).info.video_playing, true);
  let styleReads = 0;
  const parentStyle = app.parent.style;
  app.parent.style = new Proxy(parentStyle, { get(t, k) { styleReads++; return t[k]; } });
  app.parent.style.display = "none";
  styleReads = 0;
  app.listeners.timeupdate();
  assert.equal(styleReads, 0, "timeupdate must not recompute styles");
  app.tick();
  assert.equal(app.messages.at(-1).info.video_playing, false, "the 2s check picks up the change");
});
