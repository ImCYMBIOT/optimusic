fn main() {
    // Declare our IPC commands explicitly so each capability file grants only what it lists:
    // music.youtube.com gets track_update and open_settings; the local settings page gets the rest.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&[
                "track_update",
                "open_settings",
                "customize_state",
                "set_theme",
                "set_hide_promos",
                "set_extension",
                "open_customize_folder",
                "reload_music_page",
            ])),
    )
    .expect("failed to run tauri-build");
}
