//! Tray icon: status ring, free space, and the menu
//! Open Freeboard / Scan Now / Pause Notifications / Quit.

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::model::{DiskStatus, GB};

pub const TRAY_ID: &str = "freeboard";
const SCALE_GB: f64 = 120.0;

pub struct TrayItems {
    pub status: MenuItem<Wry>,
    pub pause: CheckMenuItem<Wry>,
    /// Renamed when the language changes: (item, string key).
    pub labels: Vec<(MenuItem<Wry>, &'static str)>,
}

/// Ring colors follow the prototype's tray ring: neutral for Healthy, amber
/// for Low, red for Critical. Pixels are RGBA.
fn ring_color(status: DiskStatus) -> [u8; 3] {
    match status {
        DiskStatus::Healthy => [0x8C, 0x9B, 0xA1],
        DiskStatus::Low => [0xE3, 0xAE, 0x55],
        DiskStatus::Critical => [0xF0, 0x83, 0x78],
    }
}

/// Render the status ring (prototype `ringSVG`): a faint full track plus an
/// arc for the used share of the top 120 GB, starting at 12 o'clock.
pub fn ring_rgba(status: DiskStatus, free_bytes: u64, size: u32) -> Vec<u8> {
    let used = 1.0 - (free_bytes as f64 / GB as f64).min(SCALE_GB) / SCALE_GB;
    let s = size as f64;
    let c = s / 2.0;
    let r = s * 7.0 / 18.0;
    let w = s * 3.0 / 18.0;
    let col = ring_color(status);
    let track = [0x8C, 0x9B, 0xA1];
    let mut px = vec![0u8; (size * size * 4) as usize];
    let ss = 4; // supersampling
    for y in 0..size {
        for x in 0..size {
            let (mut a_arc, mut a_track) = (0.0, 0.0);
            for sy in 0..ss {
                for sx in 0..ss {
                    let fx = x as f64 + (sx as f64 + 0.5) / ss as f64 - c;
                    let fy = y as f64 + (sy as f64 + 0.5) / ss as f64 - c;
                    let d = (fx * fx + fy * fy).sqrt();
                    if (d - r).abs() <= w / 2.0 {
                        // Angle clockwise from 12 o'clock in [0, 1).
                        let ang = (fx.atan2(-fy) / (2.0 * std::f64::consts::PI)).rem_euclid(1.0);
                        if ang <= used {
                            a_arc += 1.0;
                        } else {
                            a_track += 1.0;
                        }
                    }
                }
            }
            let n = (ss * ss) as f64;
            let (aa, at) = (a_arc / n, a_track / n * 0.35);
            let alpha = aa + at;
            if alpha > 0.0 {
                let i = ((y * size + x) * 4) as usize;
                for k in 0..3 {
                    px[i + k] = ((col[k] as f64 * aa + track[k] as f64 * at) / alpha) as u8;
                }
                px[i + 3] = (alpha.min(1.0) * 255.0) as u8;
            }
        }
    }
    px
}

/// "Low · 12.4 GB free", "Niedrig · 12,4 GB frei" (the status pill and
/// the prototype's free-space line, in the language in use).
pub fn status_text(status: DiskStatus, free_bytes: u64, lang: &str) -> String {
    use crate::i18n::{gb, tr};
    let key = match status {
        DiskStatus::Healthy => "st.good",
        DiskStatus::Low => "st.warn",
        DiskStatus::Critical => "st.crit",
    };
    let free = gb(lang, free_bytes);
    // "12,4 GB" + " frei": the unit is part of `gb.free`, so swap it in.
    let unit = if lang == "fr" { "Go" } else { "GB" };
    let tail = tr(lang, "gb.free").replacen("GB", unit, 1);
    format!("{} · {} {}", tr(lang, key), free.trim_end_matches(unit).trim_end(), tail)
}

pub fn build(app: &AppHandle, paused: bool) -> tauri::Result<()> {
    let status = MenuItem::with_id(app, "status", "Checking free space…", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Freeboard", true, None::<&str>)?;
    // Linux desktops do not report tray clicks, so the menu opens the tray view.
    let summary = MenuItem::with_id(app, "summary", "Show Summary", true, None::<&str>)?;
    let scan = MenuItem::with_id(app, "scan", "Scan Now", true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pause Notifications", true, paused, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = if cfg!(target_os = "linux") {
        Menu::with_items(app, &[&status, &sep1, &summary, &open, &scan, &pause, &sep2, &quit])?
    } else {
        Menu::with_items(app, &[&status, &sep1, &open, &scan, &pause, &sep2, &quit])?
    };
    let icon = Image::new_owned(ring_rgba(DiskStatus::Healthy, 120 * GB, 44), 44, 44);
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Freeboard")
        .menu(&menu)
        // A left click opens the tray view; the menu is on the right click.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "open" => crate::open_window(app),
            "summary" => crate::toggle_tray_view(app, None),
            "scan" => {
                if let Some(core) = app.try_state::<std::sync::Arc<crate::app_core::AppCore>>() {
                    let _ = core.start_scan(crate::model::ScanKind::Quick);
                }
            }
            "pause" => {
                if let Some(core) = app.try_state::<std::sync::Arc<crate::app_core::AppCore>>() {
                    let now = !core.notifications_paused();
                    let _ = core.set_notifications_paused(now);
                    sync_pause_item(app, now);
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, position, .. } = ev {
                crate::toggle_tray_view(tray.app_handle(), Some(position));
            }
        })
        .build(app)?;
    let labels = vec![(open.clone(), "m.open"), (summary.clone(), "m.summary"), (scan.clone(), "m.scan"), (quit.clone(), "m.quit")];
    app.manage(TrayItems { status, pause, labels });
    Ok(())
}

pub fn update(app: &AppHandle, status: DiskStatus, free_bytes: u64, show_free: bool, lang: &str) {
    let text = status_text(status, free_bytes, lang);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(Image::new_owned(ring_rgba(status, free_bytes, 44), 44, 44)));
        let _ = tray.set_tooltip(Some(format!("Freeboard: {text}")));
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let _ = tray.set_title(if show_free { Some(crate::i18n::gb(lang, free_bytes)) } else { None });
    }
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.status.set_text(text);
        let _ = items.pause.set_text(crate::i18n::tr(lang, "m.pause"));
        for (item, key) in &items.labels {
            let _ = item.set_text(crate::i18n::tr(lang, key));
        }
    }
}

pub fn sync_pause_item(app: &AppHandle, paused: bool) {
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.pause.set_checked(paused);
    }
}

#[cfg(test)]
mod lang_tests {
    use super::*;

    #[test]
    fn status_text_follows_the_language() {
        assert_eq!(status_text(DiskStatus::Low, 12_400_000_000, "en"), "Low · 12.4 GB free");
        assert_eq!(status_text(DiskStatus::Low, 12_400_000_000, "de"), format!("{} · 12,4 GB frei", crate::i18n::tr("de", "st.warn")));
        assert_eq!(status_text(DiskStatus::Healthy, 12_400_000_000, "fr"), format!("{} · 12,4 Go libres", crate::i18n::tr("fr", "st.good")));
    }
}
