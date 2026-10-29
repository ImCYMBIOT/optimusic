//! Update checks against GitHub releases (endpoint and public key: `plugins.updater` in
//! tauri.conf.json). Every update is signature-checked by the plugin before it's installed.
//!
//! Three modes, picked from the tray's Updates submenu and saved across restarts:
//! - Off: no update requests at all.
//! - Notify (default): an "Update to vX" tray item appears; nothing downloads until it's clicked.
//! - Auto: installs at the next start-up, before any music plays, so a song is never cut off.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

const EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const RELEASES: &str = "https://github.com/ImCYMBIOT/optimusic/releases/latest";

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Off,
    Notify,
    Auto,
}

const MODES: [(Mode, &str, &str); 3] = [
    (Mode::Off, "updates-off", "off"),
    (Mode::Notify, "updates-notify", "notify"),
    (Mode::Auto, "updates-auto", "auto"),
];

impl Mode {
    /// The tray menu id for a mode, or `None` if `id` isn't one of them.
    pub fn from_menu_id(id: &str) -> Option<Self> {
        MODES.iter().find(|(_, m, _)| *m == id).map(|(mode, ..)| *mode)
    }
}

pub struct Updates {
    mode: Mutex<Mode>,
    pending: Mutex<Option<Update>>,
    /// The last install attempt failed; the tray item now opens the download page instead.
    failed: AtomicBool,
    shown: AtomicBool,
    menu: Menu<Wry>,
    modes: Vec<CheckMenuItem<Wry>>,
    check_now: MenuItem<Wry>,
    install: MenuItem<Wry>,
}

/// Tray items for the Updates submenu: the three modes, then "Check now".
pub fn menu_items(app: &AppHandle) -> tauri::Result<(Vec<CheckMenuItem<Wry>>, MenuItem<Wry>)> {
    let mode = load_mode(app);
    let labels = ["Off", "Notify me", "Install automatically"];
    let modes = MODES
        .iter()
        .zip(labels)
        .map(|((m, id, _), label)| CheckMenuItem::with_id(app, *id, label, true, *m == mode, None::<&str>))
        .collect::<tauri::Result<_>>()?;
    let check_now = MenuItem::with_id(app, "updates-check", "Check now", true, None::<&str>)?;
    Ok((modes, check_now))
}

/// Starts the periodic checks. `menu` is the tray menu, where the "Update to vX" item goes.
pub fn start(
    app: &AppHandle,
    menu: Menu<Wry>,
    modes: Vec<CheckMenuItem<Wry>>,
    check_now: MenuItem<Wry>,
) -> tauri::Result<()> {
    let install = MenuItem::with_id(app, "update-install", "Update", true, None::<&str>)?;
    app.manage(Updates {
        mode: Mutex::new(load_mode(app)),
        pending: Mutex::new(None),
        failed: AtomicBool::new(false),
        shown: AtomicBool::new(false),
        menu,
        modes,
        check_now,
        install,
    });
    let app = app.clone();
    std::thread::spawn(move || {
        let mut startup = true;
        loop {
            let mode = *app.state::<Updates>().mode.lock().unwrap();
            if mode != Mode::Off {
                // Auto installs only at start-up; a mid-session find waits in the tray.
                tauri::async_runtime::block_on(check(&app, startup && mode == Mode::Auto, false));
            }
            startup = false;
            std::thread::sleep(EVERY);
        }
    });
    Ok(())
}

pub fn set_mode(app: &AppHandle, mode: Mode) {
    let u = app.state::<Updates>();
    *u.mode.lock().unwrap() = mode;
    for (item, (m, ..)) in u.modes.iter().zip(MODES) {
        let _ = item.set_checked(m == mode);
    }
    if let Some(path) = settings_file(app) {
        let name = MODES.iter().find(|(m, ..)| *m == mode).unwrap().2;
        let _ = std::fs::create_dir_all(path.parent().unwrap()).and_then(|_| std::fs::write(&path, name));
    }
    if mode != Mode::Off {
        check_now(app);
    }
}

pub fn check_now(app: &AppHandle) {
    let app = app.clone();
    let _ = app.state::<Updates>().check_now.set_text("Checking…");
    tauri::async_runtime::spawn(async move { check(&app, false, true).await });
}

async fn check(app: &AppHandle, install_now: bool, manual: bool) {
    let u = app.state::<Updates>();
    let result = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    match result {
        Ok(Some(update)) if install_now => install(app, update),
        Ok(Some(update)) => offer(app, update),
        Ok(None) if manual => {
            let _ = u.check_now.set_text("Check now (up to date)");
        }
        Err(_) if manual => {
            let _ = u.check_now.set_text("Check now (couldn't reach GitHub)");
        }
        _ => {}
    }
    if manual {
        // Leave the result up for a bit, then go back to the plain label.
        let item = u.check_now.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(60));
            let _ = item.set_text("Check now");
        });
    }
}

/// Shows "Update to vX" at the top of the tray menu.
fn offer(app: &AppHandle, update: Update) {
    let u = app.state::<Updates>();
    let _ = u.install.set_text(format!("Update to v{}", update.version));
    let _ = u.install.set_enabled(true);
    u.failed.store(false, Ordering::SeqCst);
    *u.pending.lock().unwrap() = Some(update);
    if !u.shown.swap(true, Ordering::SeqCst) {
        let _ = u.menu.insert(&u.install, 0);
    }
}

/// The "Update to vX" tray item was clicked.
pub fn install_clicked(app: &AppHandle) {
    let u = app.state::<Updates>();
    if u.failed.load(Ordering::SeqCst) {
        let _ = app.opener().open_url(RELEASES, None::<&str>);
        return;
    }
    let pending = u.pending.lock().unwrap().take();
    if let Some(update) = pending {
        install(app, update);
    }
}

/// Downloads, verifies and installs, then restarts into the new version. On Windows the
/// installer closes the app itself; on Linux .deb installs ask for the admin password.
fn install(app: &AppHandle, update: Update) {
    let u = app.state::<Updates>();
    let _ = u.install.set_text(format!("Downloading v{}…", update.version));
    let _ = u.install.set_enabled(false);
    if !u.shown.swap(true, Ordering::SeqCst) {
        let _ = u.menu.insert(&u.install, 0);
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match update.download_and_install(|_, _| {}, || {}).await {
            Ok(()) => app.restart(),
            Err(_) => {
                let u = app.state::<Updates>();
                u.failed.store(true, Ordering::SeqCst);
                let _ = u.install.set_text("Update failed: open download page");
                let _ = u.install.set_enabled(true);
            }
        }
    });
}

fn settings_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("updates"))
}

fn load_mode(app: &AppHandle) -> Mode {
    let saved = settings_file(app).and_then(|p| std::fs::read_to_string(p).ok());
    MODES
        .iter()
        .find(|(.., name)| Some(*name) == saved.as_deref().map(str::trim))
        .map_or(Mode::Notify, |(mode, ..)| *mode)
}
