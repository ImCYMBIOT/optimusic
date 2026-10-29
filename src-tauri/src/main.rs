#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod discord;
mod updates;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::time::Duration;

use serde::Deserialize;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};
use tauri_plugin_opener::OpenerExt;

const YTM_URL: &str = "https://music.youtube.com/";
const BRIDGE_JS: &str = include_str!("bridge.js");

/// WebView2 flags. Nearly all of the app's memory is WebView2 (browser, GPU, renderer and
/// utility processes), so this is where the savings are. The first group is wry's defaults,
/// which we must repeat because passing any args replaces them. `OPTIMUSIC_BROWSER_ARGS`
/// overrides the whole string, for experimenting with bench.ps1.
#[cfg(target_os = "windows")]
const BROWSER_ARGS: &str = concat!(
    // wry defaults + Edge/Chromium services a music player never uses
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,",
    "Translate,msEdgeTranslate,OptimizationHints,MediaRouter,DialMediaRouteProvider,",
    "AutofillServerCommunication,InterestFeedContentSuggestions,CertificateTransparencyComponentUpdater,",
    "SpareRendererForSitePerProcess,BackForwardCache,",
    // run the audio service inside the browser process...
    "AudioServiceOutOfProcess",
    // ...and the network service too (two fewer processes)
    " --enable-features=NetworkServiceInProcess2",
    // smaller caches, tile memory and V8 heaps (Chromium's "low-end device" preset)
    " --enable-low-end-device-mode",
    // one renderer per site instead of one per frame/origin (Google iframes)
    " --process-per-site",
    // fold the GPU process into the browser process
    " --in-process-gpu",
    " --disable-background-networking --disable-component-update --disable-sync --no-pings",
);

struct AppState {
    media: Mutex<Option<MediaControls>>,
    now_playing: Mutex<Option<MenuItem<Wry>>>,
    play_pause: Mutex<Option<MenuItem<Wry>>>,
    quitting: AtomicBool,
    backgrounded: AtomicBool,
    discord: Mutex<Option<Sender<discord::Msg>>>,
}

#[derive(Deserialize, Clone)]
struct TrackInfo {
    title: String,
    artist: String,
    album: Option<String>,
    art: Option<String>,
    #[serde(rename = "videoId")]
    video_id: Option<String>,
    duration: f64,
    position: f64,
    playing: bool,
}

/// "Show on Discord" is on unless this marker file exists in the app's config folder.
fn discord_off_marker(app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("discord-off"))
}

/// Run a player action inside the page (see bridge.js).
fn control(app: &AppHandle, action: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(format!(
            "window.__optimusic && window.__optimusic.control({:?})",
            action
        ));
    }
}

/// Hidden to tray or minimized: tell WebView2 nothing is on screen (it stops painting and
/// drops GPU tiles, like a background tab) and ask it to trim memory. Audio keeps playing.
fn set_backgrounded(w: &WebviewWindow, background: bool) {
    if w.state::<AppState>().backgrounded.swap(background, Ordering::SeqCst) == background {
        return;
    }
    #[cfg(target_os = "windows")]
    let _ = w.with_webview(move |wv| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::*;
        use windows_core::Interface;
        let controller = wv.controller();
        let _ = controller.SetIsVisible(!background);
        if let Ok(core) = controller.CoreWebView2().and_then(|c| c.cast::<ICoreWebView2_19>()) {
            let _ = core.SetMemoryUsageTargetLevel(if background {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            });
        }
    });
}

fn hide_window(w: &WebviewWindow) {
    set_backgrounded(w, true);
    let _ = w.hide();
}

fn toggle_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            hide_window(&w);
        } else {
            show_window(app);
        }
    }
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        set_backgrounded(&w, false);
    }
}

/// Called from bridge.js whenever the track or play state changes.
#[tauri::command]
fn track_update(info: TrackInfo, state: State<'_, AppState>) {
    if let Some(mc) = state.media.lock().unwrap().as_mut() {
        let _ = mc.set_metadata(MediaMetadata {
            title: Some(&info.title),
            artist: Some(&info.artist),
            album: info.album.as_deref(),
            cover_url: info.art.as_deref(),
            duration: (info.duration > 0.0).then(|| Duration::from_secs_f64(info.duration)),
        });
        let progress = Some(MediaPosition(Duration::from_secs_f64(info.position.max(0.0))));
        let _ = mc.set_playback(if info.playing {
            MediaPlayback::Playing { progress }
        } else {
            MediaPlayback::Paused { progress }
        });
    }
    if let Some(item) = state.now_playing.lock().unwrap().as_ref() {
        let mut label = format!("{} — {}", info.title, info.artist);
        if label.chars().count() > 60 {
            label = label.chars().take(57).collect::<String>() + "…";
        }
        let _ = item.set_text(label);
    }
    if let Some(item) = state.play_pause.lock().unwrap().as_ref() {
        let _ = item.set_text(if info.playing { "Pause" } else { "Play" });
    }
    if let Some(tx) = state.discord.lock().unwrap().as_ref() {
        let _ = tx.send(discord::Msg::Track(info));
    }
}

/// Hook into MPRIS (Linux) / SMTC (Windows) / Now Playing (macOS).
fn setup_media_controls(app: &AppHandle) -> Option<MediaControls> {
    #[cfg(target_os = "windows")]
    let hwnd = {
        let w = app.get_webview_window("main")?;
        Some(w.hwnd().ok()?.0 as *mut std::ffi::c_void)
    };
    #[cfg(not(target_os = "windows"))]
    let hwnd = None;

    let mut mc = MediaControls::new(PlatformConfig {
        dbus_name: "optimusic",
        display_name: "Optimusic",
        hwnd,
    })
    .ok()?;

    let handle = app.clone();
    mc.attach(move |event| match event {
        MediaControlEvent::Play => control(&handle, "play"),
        MediaControlEvent::Pause => control(&handle, "pause"),
        MediaControlEvent::Toggle => control(&handle, "toggle"),
        MediaControlEvent::Stop => control(&handle, "pause"),
        MediaControlEvent::Next => control(&handle, "next"),
        MediaControlEvent::Previous => control(&handle, "prev"),
        MediaControlEvent::Seek(SeekDirection::Forward) => control(&handle, "seekBy:10"),
        MediaControlEvent::Seek(SeekDirection::Backward) => control(&handle, "seekBy:-10"),
        MediaControlEvent::SeekBy(dir, d) => {
            let s = d.as_secs_f64();
            let s = if matches!(dir, SeekDirection::Backward) { -s } else { s };
            control(&handle, &format!("seekBy:{s}"));
        }
        MediaControlEvent::SetPosition(MediaPosition(d)) => {
            control(&handle, &format!("seekTo:{}", d.as_secs_f64()))
        }
        MediaControlEvent::Raise => show_window(&handle),
        MediaControlEvent::Quit => {
            handle.state::<AppState>().quitting.store(true, Ordering::SeqCst);
            handle.exit(0);
        }
        _ => {}
    })
    .ok()?;
    Some(mc)
}

/// Keep YouTube / Google sign-in inside the app; send every other link to the system browser.
fn allowed_in_app(url: &url::Url) -> bool {
    if url.scheme() != "https" {
        return url.scheme() == "about" || url.scheme() == "blob" || url.scheme() == "data";
    }
    let host = url.host_str().unwrap_or("");
    host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host.starts_with("accounts.google.")
        || host.ends_with(".google.com")
        || host == "google.com"
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_window(app);
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState {
            media: Mutex::new(None),
            now_playing: Mutex::new(None),
            play_pause: Mutex::new(None),
            quitting: AtomicBool::new(false),
            backgrounded: AtomicBool::new(false),
            discord: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![track_update])
        .setup(|app| {
            let handle = app.handle().clone();

            // ---- Main window: YouTube Music itself ----
            let nav_handle = handle.clone();
            let mut builder = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::External(YTM_URL.parse().unwrap()),
            )
            .title("Optimusic")
            .inner_size(1200.0, 800.0)
            .min_inner_size(400.0, 300.0)
            .initialization_script(BRIDGE_JS)
            .on_navigation(move |url| {
                if allowed_in_app(url) {
                    true
                } else {
                    let _ = nav_handle.opener().open_url(url.as_str(), None::<&str>);
                    false
                }
            });
            // Optional override if Google ever rejects the webview's default user agent.
            if let Ok(ua) = std::env::var("OPTIMUSIC_USER_AGENT") {
                builder = builder.user_agent(&ua);
            }
            // macOS 14+ suspends hidden web views after a few minutes; keep the music going.
            #[cfg(target_os = "macos")]
            {
                builder = builder
                    .background_throttling(tauri::utils::config::BackgroundThrottlingPolicy::Disabled);
            }
            #[cfg(target_os = "windows")]
            {
                let args = std::env::var("OPTIMUSIC_BROWSER_ARGS");
                builder = builder.additional_browser_args(args.as_deref().unwrap_or(BROWSER_ARGS));
            }
            builder.build()?;

            // ---- Tray ----
            let now_playing = MenuItem::with_id(app, "np", "Nothing playing", false, None::<&str>)?;
            let play_pause = MenuItem::with_id(app, "toggle", "Play", true, None::<&str>)?;
            let next = MenuItem::with_id(app, "next", "Next", true, None::<&str>)?;
            let prev = MenuItem::with_id(app, "prev", "Previous", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Show / Hide", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let discord_on = !discord_off_marker(&handle).is_some_and(|p| p.exists());
            let discord_item =
                CheckMenuItem::with_id(app, "discord", "Show on Discord", true, discord_on, None::<&str>)?;
            let mut items: Vec<&dyn IsMenuItem<Wry>> =
                vec![&now_playing, &sep1, &play_pause, &next, &prev, &sep2];
            if let Some(id) = discord::client_id() {
                *app.state::<AppState>().discord.lock().unwrap() = Some(discord::spawn(id, discord_on));
                items.push(&discord_item);
            }
            let (update_modes, check_now) = updates::menu_items(&handle)?;
            let sep3 = PredefinedMenuItem::separator(app)?;
            let mut update_items: Vec<&dyn IsMenuItem<Wry>> =
                update_modes.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
            update_items.extend([&sep3 as &dyn IsMenuItem<Wry>, &check_now]);
            let updates_menu = Submenu::with_items(app, "Updates", true, &update_items)?;
            items.extend([&updates_menu as &dyn IsMenuItem<Wry>, &show, &quit]);
            let menu = Menu::with_items(app, &items)?;
            updates::start(&handle, menu.clone(), update_modes.clone(), check_now.clone())?;

            let discord_toggle = discord_item.clone();
            // macOS menu bar icons are monochrome "templates" that follow light/dark mode, and
            // clicking one opens its menu; elsewhere left-click toggles the window instead.
            #[cfg(target_os = "macos")]
            let (icon, mac) = (tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?, true);
            #[cfg(not(target_os = "macos"))]
            let (icon, mac) = (app.default_window_icon().unwrap().clone(), false);
            TrayIconBuilder::with_id("main")
                .icon(icon)
                .icon_as_template(mac)
                .tooltip("Optimusic")
                .menu(&menu)
                .show_menu_on_left_click(mac)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "toggle" => control(app, "toggle"),
                    "next" => control(app, "next"),
                    "prev" => control(app, "prev"),
                    "show" => toggle_window(app),
                    "discord" => {
                        let on = discord_toggle.is_checked().unwrap_or(true);
                        if let Some(tx) = app.state::<AppState>().discord.lock().unwrap().as_ref() {
                            let _ = tx.send(discord::Msg::Enabled(on));
                        }
                        if let Some(marker) = discord_off_marker(app) {
                            let _ = if on {
                                std::fs::remove_file(&marker)
                            } else {
                                std::fs::create_dir_all(marker.parent().unwrap())
                                    .and_then(|_| std::fs::write(&marker, ""))
                            };
                        }
                    }
                    "quit" => {
                        app.state::<AppState>().quitting.store(true, Ordering::SeqCst);
                        app.exit(0);
                    }
                    "updates-check" => updates::check_now(app),
                    "update-install" => updates::install_clicked(app),
                    id => {
                        if let Some(mode) = updates::Mode::from_menu_id(id) {
                            updates::set_mode(app, mode);
                        }
                    }
                })
                // Left-click toggles the window (Windows; Linux trays only show the menu)
                .on_tray_icon_event(move |tray, event| {
                    if mac {
                        return;
                    }
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_window(tray.app_handle());
                    }
                })
                .build(app)?;

            let state = app.state::<AppState>();
            *state.now_playing.lock().unwrap() = Some(now_playing);
            *state.play_pause.lock().unwrap() = Some(play_pause);
            *state.media.lock().unwrap() = setup_media_controls(&handle);
            Ok(())
        })
        // Closing the window hides it to the tray; music keeps playing.
        .on_window_event(|window, event| {
            let Some(w) = window.app_handle().get_webview_window(window.label()) else {
                return;
            };
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    if !window.state::<AppState>().quitting.load(Ordering::SeqCst) {
                        api.prevent_close();
                        hide_window(&w);
                    }
                }
                // Minimize / restore from the taskbar
                tauri::WindowEvent::Resized(_) => {
                    let minimized = w.is_minimized().unwrap_or(false);
                    if minimized || w.is_visible().unwrap_or(false) {
                        set_backgrounded(&w, minimized);
                    }
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running Optimusic")
        .run(|app, event| match event {
            // macOS: clicking the Dock icon brings the hidden window back
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => show_window(app),
            // Quit from the app menu / Cmd+Q must not be swallowed by hide-on-close
            tauri::RunEvent::ExitRequested { .. } => {
                app.state::<AppState>().quitting.store(true, Ordering::SeqCst);
            }
            _ => {}
        });
}
