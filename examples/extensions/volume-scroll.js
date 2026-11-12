// Optimusic extension: scroll the mouse wheel over the player bar to change the volume.
// To use it, open Themes & extensions (Ctrl+,) → Extensions → Open extensions folder, copy this
// file there, and switch it on.
const STEP = 5;

document.addEventListener(
  "wheel",
  (e) => {
    const bar = document.querySelector("ytmusic-player-bar");
    const player = document.getElementById("movie_player");
    if (!bar || !bar.contains(e.target) || !player || !player.setVolume) return;
    e.preventDefault();
    const volume = Math.max(0, Math.min(100, player.getVolume() + (e.deltaY < 0 ? STEP : -STEP)));
    player.setVolume(volume);
    // Keep YouTube Music's own volume slider in sync
    const slider = document.querySelector("ytmusic-player-bar #volume-slider");
    if (slider) slider.value = volume;
  },
  { passive: false }
);
