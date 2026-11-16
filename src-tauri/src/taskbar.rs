use std::cell::RefCell;
use std::io::Cursor;
use std::mem::size_of;
use std::time::Duration;

use image::{ImageReader, RgbaImage};
use tauri::{AppHandle, Manager};
use windows::core::{w, BOOL, HSTRING};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmInvalidateIconicBitmaps, DwmSetIconicLivePreviewBitmap, DwmSetIconicThumbnail,
    DwmSetWindowAttribute, DWMWA_FORCE_ICONIC_REPRESENTATION, DWMWA_HAS_ICONIC_BITMAP,
};
use windows::Win32::Graphics::Gdi::{
    CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Shell::{
    DefSubclassProc, ITaskbarList3, RemoveWindowSubclass, SetWindowSubclass, TaskbarList,
    THBF_ENABLED, THBN_CLICKED, THB_FLAGS, THB_ICON, THB_TOOLTIP, THUMBBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIcon, DestroyIcon, GetClientRect, IsWindowVisible, RegisterWindowMessageW, HICON,
    WM_COMMAND, WM_NCDESTROY,
};

const SUBCLASS_ID: usize = 0x4f50544d;
const PREVIOUS: u32 = 1;
const TOGGLE: u32 = 2;
const NEXT: u32 = 3;
const WM_DWMSENDICONICTHUMBNAIL: u32 = 0x0323;
const WM_DWMSENDICONICLIVEPREVIEWBITMAP: u32 = 0x0326;

thread_local! {
    static TOOLBAR: RefCell<Option<Toolbar>> = const { RefCell::new(None) };
}

struct Toolbar {
    app: AppHandle,
    hwnd: HWND,
    taskbar: ITaskbarList3,
    created_message: u32,
    icons: [HICON; 4],
    added: bool,
    playing: bool,
    track: String,
    thumbnail_tooltip_pending: bool,
    art_url: Option<String>,
    art: Option<RgbaImage>,
    custom_preview: bool,
}

impl Toolbar {
    fn set_custom_preview(&mut self, enabled: bool) -> windows::core::Result<()> {
        let value = BOOL::from(enabled);
        unsafe {
            DwmSetWindowAttribute(
                self.hwnd,
                DWMWA_HAS_ICONIC_BITMAP,
                &value as *const _ as _,
                size_of::<BOOL>() as u32,
            )?;
            DwmSetWindowAttribute(
                self.hwnd,
                DWMWA_FORCE_ICONIC_REPRESENTATION,
                &value as *const _ as _,
                size_of::<BOOL>() as u32,
            )?;
        }
        self.custom_preview = enabled;
        Ok(())
    }

    fn update_art(&mut self, art: Option<&str>, video_playing: Option<bool>) {
        let url = artwork_url(art, video_playing).map(|url| url.to_string());
        if self.art_url == url {
            return;
        }
        if let Err(error) = self.set_custom_preview(false) {
            eprintln!("Could not restore native taskbar preview: {error}");
            return;
        }
        self.art = None;
        self.art_url = url.clone();
        let Some(url) = url else { return };
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let result = load_art(&url).await;
            let _ = app.run_on_main_thread(move || {
                TOOLBAR.with(|slot| {
                    let mut slot = slot.borrow_mut();
                    let Some(toolbar) = slot.as_mut() else { return };
                    if toolbar.art_url.as_deref() != Some(&url) {
                        return;
                    }
                    match result {
                        Ok(art) => {
                            toolbar.art = Some(art);
                            if let Err(error) = toolbar.set_custom_preview(true) {
                                let _ = toolbar.set_custom_preview(false);
                                eprintln!("Could not enable album-art taskbar preview: {error}");
                            } else {
                                unsafe {
                                    let _ = DwmInvalidateIconicBitmaps(toolbar.hwnd);
                                }
                            }
                        }
                        Err(error) => eprintln!("Could not load taskbar album art: {error}"),
                    }
                });
            });
        });
    }

    fn render_preview(&self, message: u32, lparam: LPARAM) -> windows::core::Result<()> {
        let Some(art) = self.art.as_ref().filter(|_| self.custom_preview) else {
            return Ok(());
        };
        let (width, height) = if message == WM_DWMSENDICONICTHUMBNAIL {
            (
                ((lparam.0 >> 16) & 0xffff) as u32,
                (lparam.0 & 0xffff) as u32,
            )
        } else {
            let mut rect = RECT::default();
            unsafe { GetClientRect(self.hwnd, &mut rect)? };
            (
                (rect.right - rect.left).max(0) as u32,
                (rect.bottom - rect.top).max(0) as u32,
            )
        };
        if width == 0 || height == 0 {
            return Ok(());
        }
        let pixels = preview_pixels(art, width.min(4096), height.min(4096));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: pixels.width() as i32,
                biHeight: -(pixels.height() as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast(), pixels.len());
            let result = if message == WM_DWMSENDICONICTHUMBNAIL {
                DwmSetIconicThumbnail(self.hwnd, bitmap, 0)
            } else {
                DwmSetIconicLivePreviewBitmap(self.hwnd, bitmap, None, 0)
            };
            let _ = DeleteObject(bitmap.into());
            result
        }
    }

    fn add(&mut self) -> windows::core::Result<()> {
        // Explorer accepts buttons only after TaskbarButtonCreated, including after a restart.
        unsafe {
            self.taskbar
                .ThumbBarAddButtons(self.hwnd, &buttons(&self.icons, self.playing))?
        };
        self.added = true;
        self.thumbnail_tooltip_pending = true;
        self.set_thumbnail_tooltip();
        Ok(())
    }

    fn set_thumbnail_tooltip(&mut self) {
        if !unsafe { IsWindowVisible(self.hwnd) }.as_bool() {
            return;
        }
        let text = if self.track.is_empty() {
            "Optimusic"
        } else {
            &self.track
        };
        let result = unsafe {
            self.taskbar
                .SetThumbnailTooltip(self.hwnd, &HSTRING::from(text))
        };
        match result {
            Ok(()) => self.thumbnail_tooltip_pending = false,
            Err(error) => {
                self.thumbnail_tooltip_pending = true;
                eprintln!("Could not update taskbar thumbnail tooltip: {error}");
            }
        }
    }
}

fn buttons(icons: &[HICON; 4], playing: bool) -> [THUMBBUTTON; 3] {
    [
        button(PREVIOUS, icons[0], "Previous"),
        button(TOGGLE, icons[if playing { 2 } else { 1 }], "Play/Pause"),
        button(NEXT, icons[3], "Next"),
    ]
}

impl Drop for Toolbar {
    fn drop(&mut self) {
        for icon in self.icons {
            unsafe {
                let _ = DestroyIcon(icon);
            }
        }
    }
}

fn button(id: u32, icon: HICON, tooltip: &str) -> THUMBBUTTON {
    let mut button = THUMBBUTTON {
        dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
        iId: id,
        hIcon: icon,
        dwFlags: THBF_ENABLED,
        ..Default::default()
    };
    for (out, ch) in button.szTip.iter_mut().zip(tooltip.encode_utf16()) {
        *out = ch;
    }
    button
}

const ICON_SIZE: usize = 32;
const ICON_SCALE: f64 = 1.3;

fn rectangle_distance(x: f64, y: f64, left: f64, right: f64, top: f64, bottom: f64) -> f64 {
    (x - left).min(right - x).min(y - top).min(bottom - y)
}

fn triangle_distance(x: f64, y: f64, left: f64, right: f64) -> f64 {
    let width = right - left;
    (x - left).min(((right - x) * 11.0 - (y - 16.0).abs() * width) / width.hypot(11.0))
}

fn glyph_distance(kind: usize, x: f64, y: f64) -> f64 {
    match kind {
        1 => triangle_distance(x, y, 9.0, 26.0),
        2 => rectangle_distance(x, y, 8.0, 13.0, 5.0, 27.0)
            .max(rectangle_distance(x, y, 19.0, 24.0, 5.0, 27.0)),
        _ => {
            let x = if kind == 0 { ICON_SIZE as f64 - x } else { x };
            triangle_distance(x, y, 6.0, 22.0).max(rectangle_distance(x, y, 24.0, 27.0, 6.0, 26.0))
        }
    }
}

fn icon_pixels(kind: usize) -> [u8; ICON_SIZE * ICON_SIZE * 4] {
    let mut pixels = [0; ICON_SIZE * ICON_SIZE * 4];
    for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let x = index % ICON_SIZE;
        let y = index / ICON_SIZE;
        let mut color = 0;
        let mut alpha = 0;
        // Supersample the white fill and dark outline; Windows needs premultiplied BGRA.
        for sy in 0..4 {
            for sx in 0..4 {
                let distance = glyph_distance(
                    kind,
                    (x as f64 + (sx as f64 + 0.5) / 4.0 - 16.0) / ICON_SCALE + 16.0,
                    (y as f64 + (sy as f64 + 0.5) / 4.0 - 16.0) / ICON_SCALE + 16.0,
                );
                if distance >= -0.65 {
                    alpha += 255;
                    color += if distance >= 0.65 { 255 } else { 80 };
                }
            }
        }
        let color = ((color + 8) / 16) as u8;
        pixel.copy_from_slice(&[color, color, color, ((alpha + 8) / 16) as u8]);
    }
    pixels
}

fn icon(kind: usize) -> windows::core::Result<HICON> {
    let pixels = icon_pixels(kind);
    let mut and_mask = [0xff; ICON_SIZE * ICON_SIZE / 8];
    for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        if pixel[3] != 0 {
            and_mask[index / 8] &= !(0x80 >> (index % 8));
        }
    }
    unsafe {
        CreateIcon(
            None,
            ICON_SIZE as i32,
            ICON_SIZE as i32,
            1,
            32,
            and_mask.as_ptr(),
            pixels.as_ptr(),
        )
    }
}

pub fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let window = app
        .get_webview_window("main")
        .ok_or("main window is missing")?;
    let hwnd = HWND(window.hwnd()?.0);
    // Tauri initializes COM on the UI thread; the interface stays on that thread.
    let taskbar: ITaskbarList3 =
        unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)? };
    unsafe { taskbar.HrInit()? };
    let created_message = unsafe { RegisterWindowMessageW(w!("TaskbarButtonCreated")) };
    if created_message == 0 {
        return Err(windows::core::Error::from_thread().into());
    }
    let mut icons = [HICON::default(); 4];
    for i in 0..icons.len() {
        match icon(i) {
            Ok(icon) => icons[i] = icon,
            Err(error) => {
                for icon in &icons[..i] {
                    unsafe {
                        let _ = DestroyIcon(*icon);
                    }
                }
                return Err(error.into());
            }
        }
    }
    let toolbar = Toolbar {
        app: app.clone(),
        hwnd,
        taskbar,
        created_message,
        icons,
        added: false,
        playing: false,
        track: String::new(),
        thumbnail_tooltip_pending: true,
        art_url: None,
        art: None,
        custom_preview: false,
    };
    if !unsafe { SetWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID, 0) }.as_bool() {
        return Err(windows::core::Error::from_thread().into());
    }
    TOOLBAR.with(|slot| *slot.borrow_mut() = Some(toolbar));
    Ok(())
}

fn track_label(title: &str, artist: &str) -> String {
    format!("{} — {}", title.trim(), artist.trim()).replace(['\r', '\n', '\0'], " ")
}

fn thumbnail_title(title: &str) -> String {
    let title = title.replace(['\r', '\n', '\0'], " ");
    let title = title.trim();
    if title.is_empty() {
        "Optimusic".to_owned()
    } else {
        format!("{title} - Optimusic")
    }
}

fn tray_tooltip(track: &str) -> String {
    // NOTIFYICONDATAW allows 127 UTF-16 code units plus the terminator.
    let text = format!("Optimusic\n{track}");
    if text.encode_utf16().count() <= 127 {
        return text;
    }
    let mut tooltip = String::new();
    let mut units = 0;
    for ch in text.chars() {
        if units + ch.len_utf16() > 126 {
            break;
        }
        tooltip.push(ch);
        units += ch.len_utf16();
    }
    tooltip.push('…');
    tooltip
}

fn artwork_url(art: Option<&str>, video_playing: Option<bool>) -> Option<url::Url> {
    let value = art.filter(|_| video_playing == Some(false))?;
    let url = url::Url::parse(value).ok()?;
    let host = url.host_str()?;
    let allowed = ["googleusercontent.com", "ggpht.com", "ytimg.com"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")));
    (allowed
        && url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443))
    .then_some(url)
}

async fn load_art(url: &str) -> Result<RgbaImage, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()?;
    let mut response = client.get(url).send().await?.error_for_status()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err("album art exceeds download limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?.thumbnail(512, 512).to_rgba8())
}

fn preview_pixels(art: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let scale = (width as f64 / art.width() as f64).min(height as f64 / art.height() as f64);
    let resized = image::imageops::resize(
        art,
        ((art.width() as f64 * scale).round() as u32).clamp(1, width),
        ((art.height() as f64 * scale).round() as u32).clamp(1, height),
        image::imageops::FilterType::Triangle,
    );
    let mut pixels = RgbaImage::from_pixel(width, height, image::Rgba([24, 24, 24, 255]));
    image::imageops::overlay(
        &mut pixels,
        &resized,
        ((width - resized.width()) / 2) as i64,
        ((height - resized.height()) / 2) as i64,
    );
    // DWM expects premultiplied BGRA rather than image's straight RGBA.
    for pixel in pixels.pixels_mut() {
        let [red, green, blue, alpha] = pixel.0;
        let premultiply = |color: u8| ((color as u16 * alpha as u16 + 127) / 255) as u8;
        pixel.0 = [
            premultiply(blue),
            premultiply(green),
            premultiply(red),
            alpha,
        ];
    }
    pixels
}

pub fn update(
    app: &AppHandle,
    title: &str,
    artist: &str,
    playing: bool,
    art: Option<&str>,
    video_playing: Option<bool>,
) {
    let track = track_label(title, artist);
    let title = thumbnail_title(title);
    let art = art.map(str::to_owned);
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = app_handle.tray_by_id("main") {
            let _ = tray.set_tooltip(Some(tray_tooltip(&track)));
        }
        TOOLBAR.with(|slot| {
            let mut slot = slot.borrow_mut();
            let Some(toolbar) = slot.as_mut() else { return };
            toolbar.update_art(art.as_deref(), video_playing);
            let playing_changed = toolbar.playing != playing;
            let track_changed = toolbar.track != track;
            if !playing_changed && !track_changed && !toolbar.thumbnail_tooltip_pending {
                return;
            }
            toolbar.playing = playing;
            toolbar.track = track;
            if track_changed {
                if let Some(window) = app_handle.get_webview_window("main") {
                    if let Err(error) = window.set_title(&title) {
                        eprintln!("Could not update taskbar preview title: {error}");
                    }
                }
                toolbar.thumbnail_tooltip_pending = true;
            }
            if toolbar.added {
                if playing_changed {
                    unsafe {
                        let _ = toolbar.taskbar.ThumbBarUpdateButtons(
                            toolbar.hwnd,
                            &buttons(&toolbar.icons, playing)[1..2],
                        );
                    }
                }
                if toolbar.thumbnail_tooltip_pending {
                    toolbar.set_thumbnail_tooltip();
                }
            }
        });
    });
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        let _ = RemoveWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID);
        TOOLBAR.with(|slot| {
            slot.borrow_mut().take();
        });
    } else {
        let mut preview_handled = false;
        let action = TOOLBAR.with(|slot| {
            let Ok(mut slot) = slot.try_borrow_mut() else {
                return None;
            };
            let toolbar = slot.as_mut()?;
            if matches!(
                message,
                WM_DWMSENDICONICTHUMBNAIL | WM_DWMSENDICONICLIVEPREVIEWBITMAP
            ) && toolbar.custom_preview
            {
                preview_handled = true;
                if let Err(error) = toolbar.render_preview(message, lparam) {
                    let _ = toolbar.set_custom_preview(false);
                    eprintln!("Could not render album-art taskbar preview: {error}");
                }
            } else if message == toolbar.created_message {
                toolbar.added = false;
                if let Err(error) = toolbar.add() {
                    eprintln!("Could not add taskbar media controls: {error}");
                }
            } else if let Some(action) = clicked_action(message, wparam) {
                return Some((toolbar.app.clone(), action));
            }
            None
        });
        if preview_handled {
            return LRESULT(0);
        }
        if let Some((app, action)) = action {
            // Drop the toolbar borrow before calling back into the webview.
            crate::control(&app, action);
            return LRESULT(0);
        }
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

fn clicked_action(message: u32, wparam: WPARAM) -> Option<&'static str> {
    if message != WM_COMMAND || ((wparam.0 >> 16) & 0xffff) != THBN_CLICKED as usize {
        return None;
    }
    match (wparam.0 & 0xffff) as u32 {
        PREVIOUS => Some("prev"),
        TOGGLE => Some("toggle"),
        NEXT => Some("next"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnail_heading_uses_song_name_and_app_name() {
        assert_eq!(thumbnail_title("  Song Name  "), "Song Name - Optimusic");
        assert_eq!(thumbnail_title("Song\nTitle\0"), "Song Title - Optimusic");
        assert_eq!(thumbnail_title(" \r\n\0 "), "Optimusic");
    }

    #[test]
    fn artwork_requires_known_non_video_playback_and_a_trusted_https_url() {
        let art = Some("https://lh3.googleusercontent.com/cover=w512");
        assert!(artwork_url(art, Some(false)).is_some());
        assert!(artwork_url(art, Some(true)).is_none());
        assert!(artwork_url(art, None).is_none());
        assert!(artwork_url(None, Some(false)).is_none());
        for url in [
            "http://lh3.googleusercontent.com/cover",
            "https://googleusercontent.com.example.org/cover",
            "https://notgoogleusercontent.com/cover",
            "https://user:password@lh3.googleusercontent.com/cover",
            "https://lh3.googleusercontent.com:8443/cover",
            "file:///cover.png",
        ] {
            assert!(artwork_url(Some(url), Some(false)).is_none(), "{url}");
        }
    }

    #[test]
    fn artwork_is_centered_without_distortion_and_encoded_as_bgra() {
        let art = RgbaImage::from_pixel(2, 2, image::Rgba([255, 64, 32, 255]));
        let pixels = preview_pixels(&art, 6, 4);
        assert_eq!(pixels.dimensions(), (6, 4));
        assert_eq!(pixels.get_pixel(0, 2).0, [24, 24, 24, 255]);
        assert_eq!(pixels.get_pixel(1, 2).0, [32, 64, 255, 255]);
        assert_eq!(pixels.get_pixel(4, 2).0, [32, 64, 255, 255]);
        assert_eq!(pixels.get_pixel(5, 2).0, [24, 24, 24, 255]);
        assert_eq!(preview_pixels(&art, 1, 1).dimensions(), (1, 1));
    }

    fn button_tooltip(button: &THUMBBUTTON) -> String {
        let end = button.szTip.iter().position(|ch| *ch == 0).unwrap();
        String::from_utf16(&button.szTip[..end]).unwrap()
    }

    #[test]
    fn icons_have_smooth_edges_and_contrast_on_both_themes() {
        for kind in 0..4 {
            let pixels = icon_pixels(kind);
            let pixels = pixels.as_chunks::<4>().0;
            assert_eq!(pixels[0], [0, 0, 0, 0]);
            assert!(pixels.iter().any(|pixel| pixel[3] > 0 && pixel[3] < 255));
            assert!(pixels.contains(&[255, 255, 255, 255]));
            assert!(pixels
                .iter()
                .any(|pixel| pixel[3] == 255 && pixel[0] <= 180));
            assert!(pixels.iter().all(|pixel| pixel[0] <= pixel[3]));
        }
    }

    #[test]
    fn previous_and_next_icons_are_mirrored() {
        let previous = icon_pixels(0);
        let next = icon_pixels(3);
        for y in 0..ICON_SIZE {
            for x in 0..ICON_SIZE {
                let previous_index = (y * ICON_SIZE + x) * 4;
                let next_index = (y * ICON_SIZE + ICON_SIZE - 1 - x) * 4;
                assert_eq!(
                    previous[previous_index..previous_index + 4],
                    next[next_index..next_index + 4]
                );
            }
        }
    }

    #[test]
    fn track_metadata_forms_a_single_tray_line() {
        let track = track_label("  Song\nTitle  ", " Bj\u{f6}rk\0Band ");
        assert_eq!(track, "Song Title — Bj\u{f6}rk Band");
        assert_eq!(
            tray_tooltip(&track),
            "Optimusic\nSong Title — Bj\u{f6}rk Band"
        );
    }

    #[test]
    fn long_tray_tooltips_preserve_unicode_within_windows_limit() {
        let track = track_label(&"\u{1f3b5}".repeat(100), "Artist");
        let tooltip = tray_tooltip(&track);
        assert!(tooltip.starts_with("Optimusic\n"));
        assert!(tooltip.ends_with('…'));
        assert!(tooltip.encode_utf16().count() <= 127);
        assert!(!tooltip.contains('\u{fffd}'));
    }

    #[test]
    fn button_tooltips_are_action_only_in_both_playback_states() {
        let icons = [HICON::default(); 4];
        for playing in [false, true] {
            let buttons = buttons(&icons, playing);
            for (index, action) in ["Previous", "Play/Pause", "Next"].iter().enumerate() {
                assert_eq!(button_tooltip(&buttons[index]), *action);
            }
        }
    }

    #[test]
    fn thumbnail_clicks_route_to_player_actions() {
        for (id, action) in [(PREVIOUS, "prev"), (TOGGLE, "toggle"), (NEXT, "next")] {
            let click = WPARAM(((THBN_CLICKED as usize) << 16) | id as usize);
            assert_eq!(clicked_action(WM_COMMAND, click), Some(action));
            assert_eq!(clicked_action(WM_COMMAND, WPARAM(id as usize)), None);
            assert_eq!(clicked_action(WM_NCDESTROY, click), None);
        }
        assert_eq!(
            clicked_action(WM_COMMAND, WPARAM(((THBN_CLICKED as usize) << 16) | 99)),
            None
        );
    }

    #[test]
    fn playback_changes_only_the_middle_button() {
        let icons = [
            icon(0).unwrap(),
            icon(1).unwrap(),
            icon(2).unwrap(),
            icon(3).unwrap(),
        ];
        let paused = buttons(&icons, false);
        let playing = buttons(&icons, true);
        for i in [0, 2] {
            assert_eq!(paused[i].hIcon, playing[i].hIcon);
            assert_eq!(paused[i].szTip, playing[i].szTip);
        }
        assert_eq!(paused[1].hIcon, icons[1]);
        assert_eq!(playing[1].hIcon, icons[2]);
        assert_eq!(button_tooltip(&paused[1]), "Play/Pause");
        assert_eq!(button_tooltip(&playing[1]), "Play/Pause");
        for icon in icons {
            unsafe {
                DestroyIcon(icon).unwrap();
            }
        }
    }
}
