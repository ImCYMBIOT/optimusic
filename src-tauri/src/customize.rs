//! Themes (CSS) and extensions (JavaScript) for the YouTube Music page, Spicetify-style.
//!
//! - One theme at a time: a built-in one or any `.css` file in the themes folder.
//! - "Hide upgrade prompts" is its own toggle, so it works with any theme.
//! - Extensions are `.js` files in the extensions folder. Each is off by default and switched
//!   on separately; they run inside the signed-in page, so the settings page asks first.
//!
//! The settings window (dist/settings.html, its own capability) is the only page that can call
//! the commands below; music.youtube.com can only open it. bridge.js does the in-page work, and
//! only on music.youtube.com.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;

/// (id, name, description, preview colours: background, surface, accent, CSS)
const BUILTIN: [(&str, &str, &str, [&str; 3], &str); 2] = [
    (
        "amoled",
        "AMOLED Black",
        "True black everywhere. Saves power on OLED screens.",
        ["#000000", "#0a0a0a", "#ff4e45"],
        include_str!("../themes/amoled.css"),
    ),
    (
        "violet",
        "Optimusic Violet",
        "Violet instead of red for the progress bar, links and highlights.",
        ["#030303", "#212121", "#a78bfa"],
        include_str!("../themes/violet.css"),
    ),
];
const DEFAULT_PREVIEW: [&str; 3] = ["#030303", "#212121", "#ff0000"];
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

#[derive(Default)]
pub struct Customize {
    settings: Mutex<Settings>,
    /// An extension was turned off; it's only gone once YouTube Music reloads.
    needs_reload: Mutex<bool>,
}

pub fn manage(app: &AppHandle) {
    app.manage(Customize { settings: Mutex::new(load(app)), ..Default::default() });
}

/// The script injected before YouTube Music's own: bridge.js plus the saved theme, so the
/// page never flashes unthemed. Extensions wait for the page to load (`on_page_load`).
pub fn init_script(app: &AppHandle, bridge: &str) -> String {
    let css = theme_css(app, &load(app));
    format!("{bridge}\nwindow.__optimusic && window.__optimusic.setTheme({});", json(&css))
}

/// After each page load: re-apply the theme (it may have changed since start-up) and run the
/// enabled extensions.
pub fn on_page_load(w: &WebviewWindow) {
    let app = w.app_handle();
    let settings = app.state::<Customize>().settings.lock().unwrap().clone();
    *app.state::<Customize>().needs_reload.lock().unwrap() = false;
    apply_theme(app, &settings);
    for file in &settings.extensions {
        run_extension(app, w, file);
    }
}

/// Opens (or focuses) the Themes & extensions window.
pub fn open_window(app: &AppHandle, browser_args: Option<&str>) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    let mut builder = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Optimusic: Themes & extensions")
        .inner_size(760.0, 600.0)
        .min_inner_size(520.0, 420.0);
    #[cfg(target_os = "windows")]
    if let Some(args) = browser_args {
        // Must match the main window's, as both share one WebView2 profile.
        builder = builder.additional_browser_args(args);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = browser_args;
    let _ = builder.build();
}

// ---- What the settings page sees ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    id: String,
    name: String,
    description: String,
    builtin: bool,
    preview: Option<[String; 3]>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    file: String,
    name: String,
    description: String,
    enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    themes: Vec<Theme>,
    theme: String,
    hide_promos: bool,
    extensions: Vec<Extension>,
    needs_reload: bool,
    themes_folder: String,
    extensions_folder: String,
}

fn view(app: &AppHandle) -> View {
    let c = app.state::<Customize>();
    let s = c.settings.lock().unwrap().clone();
    let needs_reload = *c.needs_reload.lock().unwrap();
    let colours = |c: [&str; 3]| Some(c.map(String::from));

    let mut themes = vec![Theme {
        id: String::new(),
        name: "Default".into(),
        description: "YouTube Music as it comes.".into(),
        builtin: true,
        preview: colours(DEFAULT_PREVIEW),
    }];
    themes.extend(BUILTIN.iter().map(|(id, name, description, preview, _)| Theme {
        id: format!("builtin:{id}"),
        name: name.to_string(),
        description: description.to_string(),
        builtin: true,
        preview: colours(*preview),
    }));
    let themes_dir = folder(app, "themes");
    themes.extend(files(&themes_dir, "css").into_iter().map(|f| Theme {
        description: read(&themes_dir.join(&f)).map(|t| describe(&t)).unwrap_or_default(),
        name: stem(&f),
        id: format!("file:{f}"),
        builtin: false,
        preview: None,
    }));

    let ext_dir = folder(app, "extensions");
    let extensions = files(&ext_dir, "js")
        .into_iter()
        .map(|f| Extension {
            description: read(&ext_dir.join(&f)).map(|t| describe(&t)).unwrap_or_default(),
            name: stem(&f),
            enabled: s.extensions.contains(&f),
            file: f,
        })
        .collect();

    View {
        themes,
        theme: s.theme,
        hide_promos: s.hide_promos,
        extensions,
        needs_reload,
        themes_folder: themes_dir.to_string_lossy().into_owned(),
        extensions_folder: ext_dir.to_string_lossy().into_owned(),
    }
}

// ---- Commands for the settings page (see capabilities/settings.json) ----

#[tauri::command]
pub fn customize_state(app: AppHandle) -> View {
    view(&app)
}

#[tauri::command]
pub fn set_theme(app: AppHandle, id: String) -> View {
    update(&app, |s| s.theme = id);
    view(&app)
}

#[tauri::command]
pub fn set_hide_promos(app: AppHandle, hide: bool) -> View {
    update(&app, |s| s.hide_promos = hide);
    view(&app)
}

/// The page shows its own warning before calling this with `enabled: true`.
#[tauri::command]
pub fn set_extension(app: AppHandle, file: String, enabled: bool) -> View {
    let c = app.state::<Customize>();
    let was = c.settings.lock().unwrap().extensions.contains(&file);
    if enabled && !was && files(&folder(&app, "extensions"), "js").contains(&file) {
        c.settings.lock().unwrap().extensions.push(file.clone());
        save(&app);
        if let Some(w) = app.get_webview_window("main") {
            run_extension(&app, &w, &file);
        }
    } else if !enabled && was {
        // A script that already ran can't be unloaded; it's gone after a reload.
        c.settings.lock().unwrap().extensions.retain(|f| *f != file);
        save(&app);
        *c.needs_reload.lock().unwrap() = true;
    }
    view(&app)
}

#[tauri::command]
pub fn open_customize_folder(app: AppHandle, kind: String) {
    let dir = folder(&app, if kind == "themes" { "themes" } else { "extensions" });
    let _ = std::fs::create_dir_all(&dir);
    let _ = app.opener().open_path(dir.to_string_lossy(), None::<&str>);
}

#[tauri::command]
pub fn reload_music_page(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval("location.reload()");
    }
}

// ---- Internals ----

fn update(app: &AppHandle, f: impl FnOnce(&mut Settings)) {
    let settings = {
        let c = app.state::<Customize>();
        let mut s = c.settings.lock().unwrap();
        f(&mut s);
        s.clone()
    };
    save(app);
    apply_theme(app, &settings);
}

fn apply_theme(app: &AppHandle, settings: &Settings) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(format!(
            "window.__optimusic && window.__optimusic.setTheme({});",
            json(&theme_css(app, settings))
        ));
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

fn theme_css(app: &AppHandle, s: &Settings) -> String {
    let mut css = if let Some(id) = s.theme.strip_prefix("builtin:") {
        BUILTIN.iter().find(|b| b.0 == id).map(|b| b.4.to_string()).unwrap_or_default()
    } else if let Some(file) = s.theme.strip_prefix("file:") {
        // Only files actually in the themes folder (no "../" paths)
        let dir = folder(app, "themes");
        files(&dir, "css").iter().find(|f| *f == file).and_then(|f| read(&dir.join(f))).unwrap_or_default()
    } else {
        String::new()
    };
    if s.hide_promos {
        css.push('\n');
        css.push_str(NO_PROMOS);
    }
    css
}

/// The file's first comment, minus an "Optimusic theme:" / "Optimusic extension:" prefix.
fn describe(text: &str) -> String {
    let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let comment = first
        .strip_prefix("//")
        .or_else(|| first.strip_prefix("/*"))
        .map(|c| c.trim_end_matches("*/").trim())
        .unwrap_or("");
    let comment = ["Optimusic theme:", "Optimusic extension:"]
        .iter()
        .find_map(|p| comment.strip_prefix(p))
        .unwrap_or(comment);
    let mut chars = comment.trim().chars().take(160);
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn stem(file: &str) -> String {
    Path::new(file).file_stem().unwrap_or_default().to_string_lossy().into_owned()
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
