//! Thin Tauri command handlers. Each delegates to `AppCore`; inputs are IDs,
//! enums and numbers only (invariant 3). Names follow the source command table.

use std::sync::Arc;

use tauri::State;

use crate::app_core::{AppCore, BlindSpotResult, CmdResult, CommandError, FolderList, SuggestionFilter, VolumeId};
use crate::diskmap::{AreaGrowth, BlindSpot, BlindSpotAction, DiskMap, PlaceChecked};
use crate::model::*;
use crate::settings::{Goal, Settings, UiSettingsPatch};

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
pub async fn list_logbook(core: Core<'_>, page: u32) -> CmdResult<HistoryPage> {
    core.list_logbook(page)
}

#[tauri::command]
pub async fn get_pile(core: Core<'_>) -> CmdResult<crate::selection::PileState> {
    core.get_pile()
}

#[tauri::command]
pub async fn set_pile_goal(core: Core<'_>, goal_bytes: Goal) -> CmdResult<crate::selection::PileState> {
    core.set_pile_goal(goal_bytes)
}

#[tauri::command]
pub async fn set_pile_areas(core: Core<'_>, areas: Vec<Group>) -> CmdResult<crate::selection::PileState> {
    core.set_pile_areas(areas)
}

#[tauri::command]
pub async fn add_to_pile(core: Core<'_>, item_ids: Vec<String>) -> CmdResult<crate::selection::PileState> {
    core.add_to_pile(item_ids)
}

#[tauri::command]
pub async fn take_out_of_pile(core: Core<'_>, item_ids: Vec<String>) -> CmdResult<crate::selection::PileState> {
    core.take_out_of_pile(item_ids)
}

#[tauri::command]
pub async fn set_item_decision(core: Core<'_>, item_id: String, decision: Option<ItemDecision>) -> CmdResult<Suggestion> {
    core.set_item_decision(item_id, decision)
}

#[tauri::command]
pub async fn set_item_parts(core: Core<'_>, item_id: String, part_ids: Vec<String>) -> CmdResult<Suggestion> {
    core.set_item_parts(item_id, part_ids)
}

// ----- disk map (spec "Disk accounting", "Blind spots", "Places checked") -----

#[tauri::command]
pub async fn get_disk_map(core: Core<'_>, volume: VolumeId) -> CmdResult<DiskMap> {
    core.get_disk_map(volume)
}

#[tauri::command]
pub async fn get_blind_spots(core: Core<'_>, volume: VolumeId) -> CmdResult<Vec<BlindSpot>> {
    core.get_blind_spots(volume)
}

/// Fix a blind spot by its ID. The path, if any, comes from the backend's
/// own measurement; the interface never sends one.
#[tauri::command]
pub async fn resolve_blind_spot(core: Core<'_>, id: String, action: BlindSpotAction) -> CmdResult<BlindSpotResult> {
    core.resolve_blind_spot(&id, action)
}

#[tauri::command]
pub async fn get_places_checked(core: Core<'_>) -> CmdResult<Vec<PlaceChecked>> {
    core.get_places_checked()
}

#[tauri::command]
pub async fn get_growth(core: Core<'_>, days: u32) -> CmdResult<Vec<AreaGrowth>> {
    core.get_growth(days)
}

#[tauri::command]
pub async fn create_disk_report(core: Core<'_>) -> CmdResult<serde_json::Value> {
    core.create_disk_report()
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
pub async fn add_folder(app: tauri::AppHandle, core: Core<'_>, list: FolderList) -> CmdResult<Settings> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app.dialog().file().set_title("Choose a folder for Freeboard to scan").blocking_pick_folder();
    match picked.and_then(|p| p.into_path().ok()) {
        Some(path) => core.add_folder(list, &path),
        None => core.get_settings(),
    }
}

#[tauri::command]
pub async fn folder_suggestions(core: Core<'_>) -> CmdResult<Vec<crate::app_core::FolderSuggestion>> {
    core.folder_suggestions()
}

#[tauri::command]
pub async fn add_suggested_folder(core: Core<'_>, index: u32) -> CmdResult<Settings> {
    core.add_suggested_folder(index)
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

/// Close the tray view (its close button and Esc).
#[tauri::command]
pub fn close_tray_view(app: tauri::AppHandle) {
    crate::close_tray_view(&app);
}

/// The tray view's button: open the main window on the pile.
#[tauri::command]
pub fn review_pile(app: tauri::AppHandle) {
    crate::review_pile(&app);
}

pub fn not_available(msg: &str) -> CommandError {
    CommandError::new("unavailable", msg)
}
