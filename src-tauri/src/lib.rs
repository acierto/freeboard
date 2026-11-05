//! Freeboard core library. The Tauri shell (`desktop` feature) is a thin
//! layer over `app_core::AppCore`, which keeps running in the tray after the
//! webview is destroyed.

pub mod app_core;
pub mod cleanup;
#[cfg(test)]
mod contract_tests;
pub mod detectors;
pub mod guard;
pub mod model;
pub mod monitor;
pub mod platform;
pub mod runner;
pub mod scan;
pub mod settings;
pub mod store;

#[cfg(feature = "desktop")]
pub mod commands;
#[cfg(feature = "desktop")]
pub mod tray;

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
                    .action("default", "Open Suggestions")
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
        fn update_tray(&self, status: DiskStatus, free_bytes: u64) {
            let app = self.0.clone();
            let _ = self.0.run_on_main_thread(move || crate::tray::update(&app, status, free_bytes));
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

    pub fn open_window(app: &AppHandle) {
        app.state::<HideGeneration>().0.fetch_add(1, Ordering::SeqCst);
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        } else {
            // Recreate the interface; it reloads everything from backend state.
            let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Freeboard")
                .inner_size(1120.0, 900.0)
                .min_inner_size(360.0, 480.0)
                .build();
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
            if app.state::<HideGeneration>().0.load(Ordering::SeqCst) == gen {
                if let Some(w) = app.get_webview_window("main") {
                    if !w.is_visible().unwrap_or(true) {
                        tracing::info!("destroying hidden webview");
                        let _ = w.destroy();
                    }
                }
            }
        });
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
        let app = tauri::Builder::default()
            .plugin(tauri_plugin_notification::init())
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
            .manage(HideGeneration(AtomicU64::new(0)))
            .setup(|app| {
                let handle = app.handle().clone();
                let data_dir = app.path().app_data_dir()?;
                std::fs::create_dir_all(&data_dir)?;
                init_logging(&data_dir.join("logs"));
                let store = Arc::new(crate::store::Store::open(&data_dir.join("freeboard.db")).map_err(|e| e.to_string())?);
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
                WindowEvent::Focused(true) => deliver_pending_view(window.app_handle()),
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
                crate::commands::list_history,
                crate::commands::get_settings,
                crate::commands::update_settings,
                crate::commands::add_folder,
                crate::commands::remove_folder,
                crate::commands::set_notifications_paused,
            ])
            .build(tauri::generate_context!())
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
pub use shell::{open_window, run};

#[cfg(not(feature = "desktop"))]
pub fn run() {
    eprintln!("Freeboard was built without the desktop feature.");
}
