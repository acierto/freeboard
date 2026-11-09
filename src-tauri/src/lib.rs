//! Freeboard core library. The Tauri shell (`desktop` feature) is a thin
//! layer over `app_core::AppCore`, which keeps running in the tray after the
//! webview is destroyed.

pub mod app_core;
pub mod cleanup;
#[cfg(test)]
mod contract_tests;
pub mod detectors;
pub mod diskmap;
pub mod guard;
pub mod i18n;
pub mod model;
pub mod risk_notes;
pub mod rules;
pub mod monitor;
pub mod platform;
pub mod runner;
pub mod scan;
pub mod selection;
pub mod settings;
pub mod store;

#[cfg(feature = "desktop")]
pub mod commands;
#[cfg(feature = "desktop")]
pub mod tray;
#[cfg(feature = "desktop")]
pub mod updates;

#[cfg(feature = "desktop")]
mod shell {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
    use tauri_plugin_autostart::ManagerExt as _;
    use tauri_plugin_notification::NotificationExt;

    use crate::app_core::{system_clock, AppCore, CoreConfig, Shell};
    use crate::model::{DiskStatus, EventSink};

    /// The webview is destroyed after the window has been hidden this long.
    pub const DESTROY_AFTER: Duration = Duration::from_secs(60);

    /// Start-up flag: run only the tray and monitoring, without a window.
    /// Used by login startup and by the Linux tray-mode restart.
    pub const TRAY_FLAG: &str = "--tray";

    /// `--after-pid <pid>`: wait for that process to exit before starting, so
    /// the single-instance check does not hand over to the exiting process.
    pub const AFTER_PID_FLAG: &str = "--after-pid";

    fn wait_for_previous_instance() {
        let args: Vec<String> = std::env::args().collect();
        let Some(pid) = args.iter().position(|a| a == AFTER_PID_FLAG).and_then(|i| args.get(i + 1)).and_then(|p| p.parse::<i32>().ok())
        else {
            return;
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while std::time::Instant::now() < deadline {
            // Signal 0 only checks whether the process still exists.
            #[cfg(unix)]
            if unsafe { libc::kill(pid, 0) } != 0 {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Incremented on every show/hide so stale destroy timers do nothing.
    struct HideGeneration(AtomicU64);

    struct TauriSink(AppHandle);
    impl EventSink for TauriSink {
        fn emit(&self, event: &str, payload: serde_json::Value) {
            let _ = self.0.emit(event, payload);
        }
    }

    struct TauriShell(AppHandle);
    impl Shell for TauriShell {
        fn notify(&self, title: &str, body: &str) {
            #[cfg(target_os = "linux")]
            {
                // Linux: show through the desktop notification service directly
                // so a click (the "default" action) opens Suggestions.
                let shown = notify_rust::Notification::new()
                    .appname("Freeboard")
                    .summary(title)
                    .body(body)
                    .icon("freeboard")
                    .action("default", &crate::i18n::tr(crate::i18n::system_language(), "menu.open"))
                    .show();
                if let Ok(handle) = shown {
                    let app = self.0.clone();
                    std::thread::spawn(move || {
                        handle.wait_for_action(|action| {
                            if action == "default" {
                                let a2 = app.clone();
                                let _ = app.run_on_main_thread(move || open_window(&a2));
                            }
                        });
                    });
                    return;
                }
            }
            let _ = self.0.notification().builder().title(title).body(body).show();
        }
        fn update_tray(&self, status: DiskStatus, free_bytes: u64, show_free: bool, lang: &'static str) {
            let app = self.0.clone();
            let _ = self.0.run_on_main_thread(move || crate::tray::update(&app, status, free_bytes, show_free, lang));
        }
        fn set_autostart(&self, enabled: bool) {
            // Only release builds register themselves; a dev binary never does.
            if cfg!(debug_assertions) {
                return;
            }
            let al = self.0.autolaunch();
            let _ = if enabled { al.enable() } else { al.disable() };
        }
    }

    /// The tray view's window label and logical size (prototype `#popover`
    /// plus room for its shadow).
    pub const TRAY_VIEW: &str = "tray";
    // The menu bar window (spec "Menu bar window", v21): a 340 px card, at
    // most 640 px tall, in the transparent window's padding (12 top, 16 on
    // the sides, 24 below for the shadow). The interface fits the height to
    // the card once drawn (`fit_tray_view`).
    const TRAY_VIEW_SIZE: (f64, f64) = (372.0, 676.0);
    const TRAY_VIEW_PAD_Y: f64 = 36.0;
    const TRAY_CARD_MAX_H: f64 = 640.0;

    /// Open the tray view next to the tray icon, or close it if it is open.
    /// `anchor` is the click position in physical pixels (None on Linux, where
    /// tray clicks are not reported and the menu opens it instead).
    pub fn toggle_tray_view(app: &AppHandle, anchor: Option<tauri::PhysicalPosition<f64>>) {
        if let Some(w) = app.get_webview_window(TRAY_VIEW) {
            let _ = w.destroy();
            return;
        }
        let (w, h) = TRAY_VIEW_SIZE;
        let built = WebviewWindowBuilder::new(app, TRAY_VIEW, WebviewUrl::default())
            .title("Freeboard")
            .inner_size(w, h)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            // Only the rounded card and its shadow show, as in the prototype
            // (macOS needs the macos-private-api feature for this).
            .transparent(true)
            .shadow(false)
            .visible(false)
            .build();
        let win = match built {
            Ok(win) => win,
            Err(e) => {
                tracing::error!("could not open the tray view: {e}");
                return;
            }
        };
        let monitor = anchor.and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten()).or_else(|| app.primary_monitor().ok().flatten());
        if let Some(m) = monitor {
            let scale = m.scale_factor();
            let (pw, ph) = (w * scale, h * scale);
            let (mx, my) = (m.position().x as f64, m.position().y as f64);
            let (mw, mh) = (m.size().width as f64, m.size().height as f64);
            let p = anchor.unwrap_or(tauri::PhysicalPosition::new(mx + mw - pw / 2.0 - 16.0 * scale, my));
            // Under the icon when the tray is at the top of the screen (macOS,
            // most Linux desktops), above it when at the bottom (Windows).
            let y = if p.y - my < mh / 2.0 { p.y + 12.0 * scale } else { p.y - ph - 12.0 * scale };
            let x = (p.x - pw / 2.0).clamp(mx + 8.0 * scale, mx + mw - pw - 8.0 * scale);
            let _ = win.set_position(tauri::PhysicalPosition::new(x, y.max(my)));
        }
        let _ = win.show();
        let _ = win.set_focus();
    }

    /// Fit the tray view's height to its card (the interface measures it).
    /// When the window sits above the icon (a taskbar at the bottom of the
    /// screen) its bottom edge stays put, so the card stays next to the icon.
    pub fn fit_tray_view(app: &AppHandle, card_height: f64) {
        let Some(win) = app.get_webview_window(TRAY_VIEW) else { return };
        let h = card_height.clamp(120.0, TRAY_CARD_MAX_H) + TRAY_VIEW_PAD_Y;
        let (Ok(pos), Ok(size), Ok(scale)) = (win.outer_position(), win.outer_size(), win.scale_factor()) else { return };
        let new_h = (h * scale).round();
        if (new_h - size.height as f64).abs() < 1.0 {
            return;
        }
        let above_icon = win
            .current_monitor()
            .ok()
            .flatten()
            .map(|m| (pos.y as f64 - m.position().y as f64) > m.size().height as f64 / 2.0)
            .unwrap_or(false);
        let _ = win.set_size(tauri::LogicalSize::new(TRAY_VIEW_SIZE.0, h));
        if above_icon {
            let y = pos.y as f64 + size.height as f64 - new_h;
            let _ = win.set_position(tauri::PhysicalPosition::new(pos.x as f64, y));
        }
    }

    pub fn close_tray_view(app: &AppHandle) {
        if let Some(w) = app.get_webview_window(TRAY_VIEW) {
            let _ = w.destroy();
        }
    }

    /// The tray view's button: close it and open the main window on the pile.
    pub fn review_pile(app: &AppHandle) {
        open_main_view(app, "pile");
    }

    /// Close the tray view and open the main window on a view.
    pub fn open_main_view(app: &AppHandle, view: &str) {
        close_tray_view(app);
        if let Some(core) = app.try_state::<Arc<AppCore>>() {
            core.set_pending_view(view);
        }
        open_window(app);
        deliver_pending_view(app);
    }

    pub fn open_window(app: &AppHandle) {
        app.state::<HideGeneration>().0.fetch_add(1, Ordering::SeqCst);
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        } else {
            // Recreate the interface; it reloads everything from backend state.
            let built = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Freeboard")
                .inner_size(1120.0, 900.0)
                .min_inner_size(360.0, 480.0)
                .visible(true)
                .build();
            match built {
                // GTK may leave a newly built window unmapped; show it explicitly.
                Ok(w) => {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
                Err(e) => tracing::error!("could not create the window: {e}"),
            }
        }
        deliver_pending_view(app);
    }

    fn deliver_pending_view(app: &AppHandle) {
        if let Some(core) = app.try_state::<Arc<AppCore>>() {
            if let Some(view) = core.take_pending_view() {
                let app2 = app.clone();
                // Give a recreated webview time to attach its listeners.
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(600));
                    let _ = app2.emit("navigate", serde_json::json!({ "view": view }));
                });
            }
        }
    }

    fn hide_window(app: &AppHandle) {
        let Some(w) = app.get_webview_window("main") else { return };
        let _ = w.hide();
        let gen = app.state::<HideGeneration>().0.fetch_add(1, Ordering::SeqCst) + 1;
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(DESTROY_AFTER);
            loop {
                if app.state::<HideGeneration>().0.load(Ordering::SeqCst) != gen {
                    return; // reopened since
                }
                let hidden = app.get_webview_window("main").map(|w| !w.is_visible().unwrap_or(true)).unwrap_or(true);
                if !hidden {
                    return;
                }
                if cfg!(target_os = "linux") {
                    // WebKitGTK and its libraries stay resident in a process
                    // that has shown a window, so destroying the webview frees
                    // little. Start a fresh tray-only copy and exit instead,
                    // but never while a scan or cleanup is running.
                    let busy = app.try_state::<Arc<AppCore>>().map(|c| c.is_busy()).unwrap_or(true);
                    if busy {
                        std::thread::sleep(Duration::from_secs(30));
                        continue;
                    }
                    if restart_in_tray_mode(&app) {
                        return;
                    }
                }
                if let Some(w) = app.get_webview_window("main") {
                    tracing::info!("destroying hidden webview");
                    let _ = w.destroy();
                }
                return;
            }
        });
    }

    /// Linux: replace this process with a tray-only one. Every piece of state
    /// the tray mode needs (settings, last status, pause, findings, plans,
    /// history, Trash records) is already in SQLite.
    fn restart_in_tray_mode(app: &AppHandle) -> bool {
        let Ok(exe) = std::env::current_exe() else { return false };
        let me = std::process::id().to_string();
        match std::process::Command::new(exe).args([TRAY_FLAG, AFTER_PID_FLAG, &me]).spawn() {
            Ok(child) => {
                tracing::info!("restarting in tray mode as pid {}", child.id());
                app.exit(0);
                true
            }
            Err(e) => {
                tracing::warn!("could not restart in tray mode: {e}");
                false
            }
        }
    }

    fn init_logging(dir: &std::path::Path) {
        let appender = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("freeboard")
            .filename_suffix("log")
            .max_log_files(7)
            .build(dir);
        if let Ok(appender) = appender {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
                .with_writer(appender)
                .with_ansi(false)
                .try_init();
        }
    }

    pub fn run() {
        wait_for_previous_instance();
        let mut context = tauri::generate_context!();
        if std::env::args().any(|a| a == TRAY_FLAG) {
            // Tray mode: create no window at start-up, so WebKit is not loaded
            // until the person opens Freeboard from the tray.
            context.config_mut().app.windows.clear();
        }
        let app = tauri::Builder::default()
            // Must be registered first. A second launch hands its arguments to
            // the running copy and exits; the running copy opens its window
            // (unless the second launch was itself a tray-mode start).
            .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
                if !args.iter().any(|a| a == TRAY_FLAG) {
                    open_window(app);
                }
            }))
            .plugin(tauri_plugin_notification::init())
            .plugin(tauri_plugin_dialog::init())
            // Updates: checked only when the version chip is clicked (docs/decisions.md, "Updates only when asked").
            .plugin(tauri_plugin_updater::Builder::new().build())
            .manage(crate::updates::Updates::new())
            // Login startup goes straight to the tray, without opening a window.
            .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec![TRAY_FLAG])))
            .manage(HideGeneration(AtomicU64::new(0)))
            .setup(|app| {
                let handle = app.handle().clone();
                let data_dir = app.path().app_data_dir()?;
                std::fs::create_dir_all(&data_dir)?;
                init_logging(&data_dir.join("logs"));
                let store = match crate::store::Store::open(&data_dir.join("freeboard.db")) {
                    Ok(s) => Arc::new(s),
                    // An older Freeboard meeting data a newer one upgraded:
                    // say so and quit, instead of crashing at launch (found
                    // on a real Mac, 2026-11-07: an old copy in Applications).
                    Err(crate::store::StoreError::TooNew(_)) => {
                        use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
                        // The settings are unreadable here, so the system's language.
                        let lang = crate::i18n::system_language();
                        app.dialog()
                            .message(crate::i18n::tr(lang, "nat.old.p"))
                            .title(crate::i18n::tr(lang, "nat.old.t"))
                            .kind(MessageDialogKind::Warning)
                            .blocking_show();
                        std::process::exit(0);
                    }
                    Err(e) => return Err(e.to_string().into()),
                };
                let home = crate::monitor::home_or_root();
                let core = AppCore::new(CoreConfig {
                    store,
                    runner: crate::runner::Runner::default(),
                    home,
                    volumes: Arc::new(crate::monitor::SysVolumes),
                    events: Arc::new(TauriSink(handle.clone())),
                    detectors: crate::scan::default_detectors(),
                    clock: system_clock(),
                    platform: crate::platform::platform_name(),
                    extra_protected: vec![data_dir.clone()],
                    measure_outside_home: true,
                });
                core.set_shell(Arc::new(TauriShell(handle.clone())));
                crate::tray::build(&handle, core.notifications_paused())?;
                TauriShell(handle.clone()).set_autostart(core.settings().start_at_login);
                core.spawn_background();
                app.manage(core);
                Ok(())
            })
            .on_window_event(|window, event| match event {
                WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                    // Closing hides to the tray; monitoring continues in Rust.
                    api.prevent_close();
                    hide_window(window.app_handle());
                }
                // A click anywhere outside the tray view closes it.
                WindowEvent::Focused(false) if window.label() == TRAY_VIEW => close_tray_view(window.app_handle()),
                WindowEvent::Focused(true) if window.label() == "main" => deliver_pending_view(window.app_handle()),
                _ => {}
            })
            .invoke_handler(tauri::generate_handler![
                crate::commands::get_status,
                crate::commands::start_scan,
                crate::commands::get_suggestions,
                crate::commands::create_plan,
                crate::commands::create_permanent_plan,
                crate::commands::get_plan,
                crate::commands::acknowledge_item,
                crate::commands::run_plan,
                crate::commands::cancel_run,
                crate::commands::get_run,
                crate::commands::get_trash,
                crate::commands::empty_freeboard_trash,
                crate::commands::list_logbook,
                crate::commands::get_pile,
                crate::commands::set_pile_goal,
                crate::commands::set_pile_areas,
                crate::commands::add_to_pile,
                crate::commands::take_out_of_pile,
                crate::commands::set_item_decision,
                crate::commands::set_item_parts,
                crate::commands::get_disk_map,
                crate::commands::get_blind_spots,
                crate::commands::resolve_blind_spot,
                crate::commands::resolve_blind_spots,
                crate::commands::get_places_checked,
                crate::commands::get_growth,
                crate::commands::create_disk_report,
                crate::commands::get_tidy_rules,
                crate::commands::get_volumes,
                crate::commands::set_volume_watched,
                crate::commands::set_tidy_rule,
                crate::commands::get_settings,
                crate::commands::update_settings,
                crate::commands::add_folder,
                crate::commands::remove_folder,
                crate::commands::folder_suggestions,
                crate::commands::get_folder_details,
                crate::commands::get_tools,
                crate::commands::get_worktrees,
                crate::commands::open_main_view,
                crate::commands::undo_remove_folder,
                crate::commands::add_suggested_folder,
                crate::commands::set_notifications_paused,
                crate::commands::close_tray_view,
                crate::commands::review_pile,
                crate::commands::fit_tray_view,
                crate::commands::get_update,
                crate::commands::check_for_update,
                crate::commands::download_update,
                crate::commands::restart_to_update,
            ])
            .build(context)
            .expect("error while building Freeboard");

        app.run(|app, event| match event {
            // Destroying the last webview must not quit: the core keeps
            // monitoring in the tray. Only the Quit menu item exits (code Some).
            RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => open_window(app),
            _ => {}
        });
    }
}

#[cfg(feature = "desktop")]
pub use shell::{close_tray_view, fit_tray_view, open_main_view, open_window, review_pile, run, toggle_tray_view};

#[cfg(not(feature = "desktop"))]
pub fn run() {
    eprintln!("Freeboard was built without the desktop feature.");
}
