fn main() {
    // Declare our IPC commands explicitly so the capability file can grant
    // music.youtube.com access to *only* these, and nothing else.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["track_update"])),
    )
    .expect("failed to run tauri-build");
}
