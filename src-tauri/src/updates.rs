//! Updates (spec "Menu bar window", revision v21): the version chip in the
//! menu bar window's header checks for a newer Freeboard, downloads it and
//! restarts into it, through the Tauri updater plugin and signed update files.
//!
//! Network use (docs/decisions.md, "Updates only when asked"): this is the only network
//! request Freeboard makes of its own, and it is made only when the person
//! clicks the chip (check) or Update (download). Nothing checks on a timer or
//! at start-up. The request goes to the endpoint in tauri.conf.json (the
//! project's GitHub releases), carries no data about the computer beyond what
//! any download does, and the file is installed only if its signature
//! matches the public key built into the app.
//!
//! The state lives here rather than in the webview, because the menu bar
//! window is destroyed whenever it loses focus: a download keeps going, and
//! reopening the window shows where it got to.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

pub const EV_UPDATE_CHANGED: &str = "update_changed";

/// Command-line flag of the copy started by Restart to update, so it shows
/// "up to date ✓" without checking again.
pub const UPDATED_FLAG: &str = "--updated";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    /// Not checked since Freeboard started: the chip shows "v1.4.0".
    Idle,
    Checking,
    /// "v1.4.0 · up to date ✓"
    Latest,
    /// "1.4.1 available" and the banner.
    Available,
    Downloading,
    /// Downloaded and verified: "Restart to update".
    Ready,
    /// The check could not reach the server, or the download failed or did
    /// not verify. Clicking the chip checks again.
    CheckFailed,
    InstallFailed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub stage: Stage,
    /// The installed version.
    pub current: String,
    /// The newer version, once one was found.
    pub version: Option<String>,
    /// Its one-line release note ("New: …"), if the release has one.
    pub notes: Option<String>,
}

struct Inner {
    stage: Stage,
    update: Option<Update>,
    bytes: Option<Vec<u8>>,
}

pub struct Updates(Mutex<Inner>);

impl Updates {
    pub fn new() -> Self {
        let stage = if std::env::args().any(|a| a == UPDATED_FLAG) { Stage::Latest } else { Stage::Idle };
        Updates(Mutex::new(Inner { stage, update: None, bytes: None }))
    }
}

impl Default for Updates {
    fn default() -> Self {
        Self::new()
    }
}

fn status_of(app: &AppHandle, i: &Inner) -> UpdateStatus {
    UpdateStatus {
        stage: i.stage,
        current: app.package_info().version.to_string(),
        version: i.update.as_ref().map(|u| u.version.clone()),
        notes: i.update.as_ref().and_then(|u| u.body.as_deref()).and_then(first_line),
    }
}

/// The banner has one row: keep the note's first line.
fn first_line(body: &str) -> Option<String> {
    body.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| l.chars().take(140).collect())
}

pub fn status(app: &AppHandle) -> UpdateStatus {
    let u = app.state::<Updates>();
    let i = u.0.lock().unwrap();
    status_of(app, &i)
}

fn set(app: &AppHandle, f: impl FnOnce(&mut Inner)) -> UpdateStatus {
    let s = {
        let u = app.state::<Updates>();
        let mut i = u.0.lock().unwrap();
        f(&mut i);
        status_of(app, &i)
    };
    let _ = app.emit(EV_UPDATE_CHANGED, &s);
    s
}

/// The chip was clicked: ask the release server once.
pub fn check(app: &AppHandle) -> UpdateStatus {
    {
        let u = app.state::<Updates>();
        let i = u.0.lock().unwrap();
        if matches!(i.stage, Stage::Checking | Stage::Downloading) {
            return status_of(app, &i);
        }
    }
    let s = set(app, |i| i.stage = Stage::Checking);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let found = match app.updater() {
            Ok(up) => up.check().await,
            Err(e) => Err(e),
        };
        match found {
            Ok(Some(update)) => {
                tracing::info!("update {} available", update.version);
                set(&app, |i| {
                    i.stage = Stage::Available;
                    i.update = Some(update);
                    i.bytes = None;
                });
            }
            Ok(None) => {
                set(&app, |i| {
                    i.stage = Stage::Latest;
                    i.update = None;
                    i.bytes = None;
                });
            }
            Err(e) => {
                tracing::warn!("update check failed: {e}");
                set(&app, |i| i.stage = Stage::CheckFailed);
            }
        }
    });
    s
}

/// Update was clicked: download the new version and verify its signature.
/// Nothing is replaced yet; that happens on Restart to update.
pub fn download(app: &AppHandle) -> UpdateStatus {
    let update = {
        let u = app.state::<Updates>();
        let i = u.0.lock().unwrap();
        match (&i.stage, &i.update) {
            (Stage::Available | Stage::InstallFailed, Some(up)) => up.clone(),
            _ => return status_of(app, &i),
        }
    };
    let s = set(app, |i| i.stage = Stage::Downloading);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // `download` verifies the signature against the built-in public key
        // and fails on a mismatch.
        match update.download(|_, _| {}, || {}).await {
            Ok(bytes) => {
                tracing::info!("update {} downloaded ({} bytes)", update.version, bytes.len());
                set(&app, |i| {
                    i.stage = Stage::Ready;
                    i.bytes = Some(bytes);
                });
            }
            Err(e) => {
                tracing::warn!("update download failed: {e}");
                set(&app, |i| i.stage = Stage::InstallFailed);
            }
        }
    });
    s
}

/// Restart to update: install the downloaded files and start the new
/// version. On Windows the plugin starts the installer (passive mode), which
/// closes Freeboard and opens the new version itself.
pub fn install_and_restart(app: &AppHandle) -> UpdateStatus {
    let (update, bytes) = {
        let u = app.state::<Updates>();
        let mut i = u.0.lock().unwrap();
        match (i.stage, i.update.clone(), i.bytes.take()) {
            (Stage::Ready, Some(up), Some(b)) => (up, b),
            (_, _, b) => {
                i.bytes = b;
                return status_of(app, &i);
            }
        }
    };
    if let Err(e) = update.install(&bytes) {
        tracing::warn!("update install failed: {e}");
        return set(app, |i| i.stage = Stage::InstallFailed);
    }
    tracing::info!("update {} installed; restarting", update.version);
    restart(app);
    status(app)
}

/// Start the installed copy in tray mode once this one has exited (the
/// single-instance check would otherwise hand over to this process).
fn restart(app: &AppHandle) {
    let me = std::process::id().to_string();
    match std::env::current_exe() {
        Ok(exe) => {
            let started = std::process::Command::new(exe).args([crate::shell::TRAY_FLAG, crate::shell::AFTER_PID_FLAG, &me, UPDATED_FLAG]).spawn();
            match started {
                Ok(_) => app.exit(0),
                Err(e) => {
                    tracing::warn!("could not start the new version: {e}");
                    app.restart();
                }
            }
        }
        Err(_) => app.restart(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_banner_shows_the_first_line_of_the_note() {
        assert_eq!(first_line("\n  Faster worktree scans, removal queue.\n\nMore text").as_deref(), Some("Faster worktree scans, removal queue."));
        assert_eq!(first_line("   \n"), None);
        assert_eq!(first_line(&"x".repeat(300)).map(|s| s.len()), Some(140));
    }
}
