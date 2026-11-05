//! Thin Tauri command handlers. Each delegates to `AppCore`; inputs are IDs,
//! enums and numbers only (invariant 3). Names follow the source command table.

use std::sync::Arc;

use tauri::State;

use crate::app_core::{AppCore, CmdResult, CommandError, FolderList, SuggestionFilter};
use crate::model::*;
use crate::settings::{Settings, UiSettingsPatch};

type Core<'a> = State<'a, Arc<AppCore>>;

// Commands that touch disks or tools run off the main thread (`async`).

#[tauri::command]
pub async fn get_status(core: Core<'_>) -> CmdResult<AppStatus> {
    core.get_status()
}

#[tauri::command]
pub async fn start_scan(core: Core<'_>, kind: ScanKind) -> CmdResult<String> {
    core.start_scan(kind)
}

#[tauri::command]
pub async fn get_suggestions(core: Core<'_>, filters: Option<SuggestionFilter>) -> CmdResult<Vec<Suggestion>> {
    core.get_suggestions(filters)
}

#[tauri::command]
pub async fn create_plan(core: Core<'_>, item_ids: Vec<String>) -> CmdResult<CleanupPlan> {
    core.create_plan(item_ids)
}

#[tauri::command]
pub async fn create_permanent_plan(core: Core<'_>, item_ids: Vec<String>) -> CmdResult<CleanupPlan> {
    core.create_permanent_plan(item_ids)
}

#[tauri::command]
pub async fn get_plan(core: Core<'_>, plan_id: String) -> CmdResult<CleanupPlan> {
    core.get_plan(&plan_id)
}

#[tauri::command]
pub async fn acknowledge_item(core: Core<'_>, plan_id: String, item_id: String) -> CmdResult<CleanupPlan> {
    core.acknowledge_item(&plan_id, &item_id)
}

#[tauri::command]
pub async fn run_plan(core: Core<'_>, plan_id: String) -> CmdResult<String> {
    core.run_plan(&plan_id)
}

#[tauri::command]
pub async fn cancel_run(core: Core<'_>, run_id: String) -> CmdResult<bool> {
    core.cancel_run(&run_id)
}

#[tauri::command]
pub async fn get_run(core: Core<'_>, run_id: String) -> CmdResult<RunDetail> {
    core.get_run(&run_id)
}

#[tauri::command]
pub async fn get_trash(core: Core<'_>) -> CmdResult<TrashSummary> {
    core.get_trash()
}

#[tauri::command]
pub async fn empty_freeboard_trash(core: Core<'_>, token: String) -> CmdResult<EmptyTrashResult> {
    core.empty_freeboard_trash(&token)
}

#[tauri::command]
pub async fn list_history(core: Core<'_>, page: u32) -> CmdResult<HistoryPage> {
    core.list_history(page)
}

#[tauri::command]
pub async fn get_settings(core: Core<'_>) -> CmdResult<Settings> {
    core.get_settings()
}

#[tauri::command]
pub async fn update_settings(core: Core<'_>, patch: UiSettingsPatch) -> CmdResult<Settings> {
    core.update_settings(patch)
}

/// Opens a native folder picker owned by the backend. The chosen path is
/// stored by the backend and never passes through the interface.
#[tauri::command]
pub async fn add_folder(app: tauri::AppHandle, window: tauri::WebviewWindow, core: Core<'_>, list: FolderList) -> CmdResult<Settings> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = app.dialog().file().set_title("Choose a folder for Freeboard to scan");
    // Windows: without an owner window the picker can open behind Freeboard
    // (found in the Windows 11 VM). macOS and Linux keep the verified
    // free-standing dialog.
    #[cfg(windows)]
    let dialog = dialog.set_parent(&window);
    #[cfg(not(windows))]
    let _ = &window;
    let picked = dialog.blocking_pick_folder();
    match picked.and_then(|p| p.into_path().ok()) {
        Some(path) => core.add_folder(list, &path),
        None => core.get_settings(),
    }
}

#[tauri::command]
pub async fn remove_folder(core: Core<'_>, list: FolderList, index: u32) -> CmdResult<Settings> {
    core.remove_folder(list, index)
}

#[tauri::command]
pub async fn set_notifications_paused(app: tauri::AppHandle, core: Core<'_>, paused: bool) -> CmdResult<bool> {
    let r = core.set_notifications_paused(paused)?;
    crate::tray::sync_pause_item(&app, paused);
    Ok(r)
}

pub fn not_available(msg: &str) -> CommandError {
    CommandError::new("unavailable", msg)
}
