//! Themes (CSS) and extensions (JavaScript) for the YouTube Music page, Spicetify-style.
//!
//! - One theme at a time: a built-in one or any `.css` file in the themes folder.
//! - "Hide upgrade prompts" is its own toggle, so it works with any theme.
//! - Extensions are `.js` files in the extensions folder. Each is off by default and switched
//!   on separately; they run inside the signed-in page, so turning one on asks first.
//!
//! Both folders live in the app's config folder, and the tray has items to open them.
//! bridge.js does the in-page work, and only on music.youtube.com.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::menu::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, WebviewWindow, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

const BUILTIN: [(&str, &str, &str); 2] = [
    ("amoled", "AMOLED Black", include_str!("../themes/amoled.css")),
    ("violet", "Optimusic Violet", include_str!("../themes/violet.css")),
];
const NO_PROMOS: &str = include_str!("../themes/no-upgrade-prompts.css");
/// Bigger files are ignored; no theme or extension needs a megabyte.
const MAX_FILE: u64 = 1 << 20;

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
struct Settings {
    /// "" for none, "builtin:<id>" or "file:<name>.css"
    theme: String,
    hide_promos: bool,
    /// File names of the enabled extensions
    extensions: Vec<String>,
}

pub struct Customize {
    settings: Mutex<Settings>,
    themes_menu: Submenu<Wry>,
    extensions_menu: Submenu<Wry>,
    theme_items: Mutex<Vec<(String, CheckMenuItem<Wry>)>>,
    extension_items: Mutex<Vec<(String, CheckMenuItem<Wry>)>>,
    promos_item: CheckMenuItem<Wry>,
    reload_item: MenuItem<Wry>,
}

/// The script injected before YouTube Music's own: bridge.js plus the saved theme, so the
/// page never flashes unthemed. Extensions wait for the page to load (`on_page_load`).
pub fn init_script(app: &AppHandle, bridge: &str) -> String {
    let css = theme_css(app, &load(app));
    format!("{bridge}\nwindow.__optimusic && window.__optimusic.setTheme({});", json(&css))
}

/// Creates the Themes and Extensions submenus for the tray.
pub fn menus(app: &AppHandle) -> tauri::Result<(Submenu<Wry>, Submenu<Wry>)> {
    let settings = load(app);
    let themes_menu = Submenu::new(app, "Themes", true)?;
    let extensions_menu = Submenu::new(app, "Extensions", true)?;
    let promos_item = CheckMenuItem::with_id(
        app,
        "promos-toggle",
        "Hide upgrade prompts",
        true,
        settings.hide_promos,
        None::<&str>,
    )?;
    let reload_item = MenuItem::with_id(app, "page-reload", "Reload page", true, None::<&str>)?;
    app.manage(Customize {
        settings: Mutex::new(settings),
        themes_menu: themes_menu.clone(),
        extensions_menu: extensions_menu.clone(),
        theme_items: Mutex::new(Vec::new()),
        extension_items: Mutex::new(Vec::new()),
        promos_item,
        reload_item,
    });
    fill(app)?;
    Ok((themes_menu, extensions_menu))
}

/// (Re)builds both submenus from the built-ins and whatever is in the folders now.
fn fill(app: &AppHandle) -> tauri::Result<()> {
    let c = app.state::<Customize>();
    let settings = c.settings.lock().unwrap().clone();
    for menu in [&c.themes_menu, &c.extensions_menu] {
        for item in menu.items()? {
            menu.remove(&item)?;
        }
    }

    let mut themes = vec![("".to_string(), "Default".to_string())];
    themes.extend(BUILTIN.iter().map(|(id, name, _)| (format!("builtin:{id}"), name.to_string())));
    themes.extend(files(&folder(app, "themes"), "css").into_iter().map(|f| {
        let name = Path::new(&f).file_stem().unwrap().to_string_lossy().into_owned();
        (format!("file:{f}"), name)
    }));
    let mut theme_items = Vec::new();
    for (id, name) in themes {
        let item = CheckMenuItem::with_id(app, format!("theme:{id}"), name, true, id == settings.theme, None::<&str>)?;
        c.themes_menu.append(&item)?;
        theme_items.push((id, item));
    }
    c.themes_menu.append(&PredefinedMenuItem::separator(app)?)?;
    c.themes_menu.append(&c.promos_item)?;
    c.themes_menu.append(&PredefinedMenuItem::separator(app)?)?;
    c.themes_menu.append(&MenuItem::with_id(app, "themes-open", "Open themes folder", true, None::<&str>)?)?;
    c.themes_menu.append(&MenuItem::with_id(app, "themes-rescan", "Refresh list", true, None::<&str>)?)?;
    *c.theme_items.lock().unwrap() = theme_items;

    let mut extension_items = Vec::new();
    let found = files(&folder(app, "extensions"), "js");
    if found.is_empty() {
        c.extensions_menu.append(&MenuItem::with_id(app, "ext-none", "No extensions yet", false, None::<&str>)?)?;
    }
    for f in found {
        let name = Path::new(&f).file_stem().unwrap().to_string_lossy().into_owned();
        let on = settings.extensions.contains(&f);
        let item = CheckMenuItem::with_id(app, format!("ext:{f}"), name, true, on, None::<&str>)?;
        c.extensions_menu.append(&item)?;
        extension_items.push((f, item));
    }
    c.extensions_menu.append(&PredefinedMenuItem::separator(app)?)?;
    c.extensions_menu.append(&MenuItem::with_id(app, "ext-open", "Open extensions folder", true, None::<&str>)?)?;
    c.extensions_menu.append(&MenuItem::with_id(app, "ext-rescan", "Refresh list", true, None::<&str>)?)?;
    c.extensions_menu.append(&c.reload_item)?;
    *c.extension_items.lock().unwrap() = extension_items;
    Ok(())
}

/// Handles a tray click if it belongs to the Themes or Extensions menus.
pub fn handle_menu(app: &AppHandle, id: &str) -> bool {
    let c = app.state::<Customize>();
    if let Some(theme) = id.strip_prefix("theme:") {
        c.settings.lock().unwrap().theme = theme.to_string();
        for (item_id, item) in c.theme_items.lock().unwrap().iter() {
            let _ = item.set_checked(item_id == theme);
        }
        save_and_apply_theme(app);
    } else if id == "promos-toggle" {
        c.settings.lock().unwrap().hide_promos = c.promos_item.is_checked().unwrap_or(false);
        save_and_apply_theme(app);
    } else if let Some(file) = id.strip_prefix("ext:") {
        toggle_extension(app, file);
    } else if id == "themes-open" || id == "ext-open" {
        let dir = folder(app, if id == "themes-open" { "themes" } else { "extensions" });
        let _ = std::fs::create_dir_all(&dir);
        let _ = app.opener().open_path(dir.to_string_lossy(), None::<&str>);
    } else if id == "themes-rescan" || id == "ext-rescan" {
        let _ = fill(app);
    } else if id == "page-reload" {
        let _ = c.reload_item.set_text("Reload page");
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.eval("location.reload()");
        }
    } else {
        return false;
    }
    true
}

fn toggle_extension(app: &AppHandle, file: &str) {
    let c = app.state::<Customize>();
    let item = c.extension_items.lock().unwrap().iter().find(|(f, _)| f == file).map(|(_, i)| i.clone());
    let Some(item) = item else { return };
    let enabled = c.settings.lock().unwrap().extensions.iter().any(|f| f == file);

    if enabled {
        // Turning off: a script that already ran can't be unloaded, so it's gone after a reload.
        c.settings.lock().unwrap().extensions.retain(|f| f != file);
        save(app);
        let _ = item.set_checked(false);
        let _ = c.reload_item.set_text("Reload page to finish turning it off");
        return;
    }

    // Turning on: undo the menu's own tick until the user confirms.
    let _ = item.set_checked(false);
    let app = app.clone();
    let file = file.to_string();
    app.dialog()
        .message(format!(
            "\"{file}\" will run inside YouTube Music while you're signed in to your Google account. \
             A malicious extension could act on your account.\n\n\
             Only turn on extensions you wrote yourself or got from someone you trust."
        ))
        .title("Turn on this extension?")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("Turn on".into(), "Cancel".into()))
        .show(move |ok| {
            if !ok {
                return;
            }
            let c = app.state::<Customize>();
            c.settings.lock().unwrap().extensions.push(file.clone());
            save(&app);
            let _ = item.set_checked(true);
            if let Some(w) = app.get_webview_window("main") {
                run_extension(&app, &w, &file);
            }
        });
}

/// After each page load: re-apply the theme (it may have changed since start-up) and run the
/// enabled extensions.
pub fn on_page_load(w: &WebviewWindow) {
    let app = w.app_handle();
    let settings = load(app);
    let _ = w.eval(format!(
        "window.__optimusic && window.__optimusic.setTheme({});",
        json(&theme_css(app, &settings))
    ));
    for file in &settings.extensions {
        run_extension(app, w, file);
    }
}

fn run_extension(app: &AppHandle, w: &WebviewWindow, file: &str) {
    let Some(code) = read(&folder(app, "extensions").join(file)) else { return };
    // Each extension gets its own eval, so a syntax error in one can't break the others.
    let _ = w.eval(format!(
        "window.__optimusic && window.__optimusic.runExtension({}, function () {{\n{code}\n}});",
        json(file)
    ));
}

fn save_and_apply_theme(app: &AppHandle) {
    save(app);
    let settings = app.state::<Customize>().settings.lock().unwrap().clone();
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(format!(
            "window.__optimusic && window.__optimusic.setTheme({});",
            json(&theme_css(app, &settings))
        ));
    }
}

fn theme_css(app: &AppHandle, s: &Settings) -> String {
    let mut css = if let Some(id) = s.theme.strip_prefix("builtin:") {
        BUILTIN.iter().find(|(b, ..)| *b == id).map(|(.., css)| css.to_string()).unwrap_or_default()
    } else if let Some(file) = s.theme.strip_prefix("file:") {
        read(&folder(app, "themes").join(file)).unwrap_or_default()
    } else {
        String::new()
    };
    if s.hide_promos {
        css.push('\n');
        css.push_str(NO_PROMOS);
    }
    css
}

fn folder(app: &AppHandle, name: &str) -> PathBuf {
    app.path().app_config_dir().unwrap_or_default().join(name)
}

/// Sorted file names in `dir` with the given extension.
fn files(dir: &Path, ext: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case(ext)))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

fn read(path: &Path) -> Option<String> {
    if std::fs::metadata(path).ok()?.len() > MAX_FILE {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// A JSON string literal, which is also a valid, safely escaped JavaScript string.
fn json(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn settings_file(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("customize.json"))
}

fn load(app: &AppHandle) -> Settings {
    settings_file(app)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(app: &AppHandle) {
    let settings = app.state::<Customize>().settings.lock().unwrap().clone();
    if let (Some(path), Ok(data)) = (settings_file(app), serde_json::to_vec_pretty(&settings)) {
        let _ = std::fs::create_dir_all(path.parent().unwrap()).and_then(|_| std::fs::write(path, data));
    }
}
