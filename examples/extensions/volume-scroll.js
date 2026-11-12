// Optimusic extension: scroll the mouse wheel over the player bar to change the volume.
// To use it, copy this file into Optimusic's extensions folder (tray → Extensions →
// Open extensions folder), then tray → Extensions → Refresh list, and tick it.
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
