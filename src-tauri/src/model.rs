//! Shared data contract between the Rust core and the React interface.
//!
//! Field names follow `docs/reference/FREEBOARD_SPEC.md` ("Data model and risk
//! rules") and are serialized in camelCase so the TypeScript types in
//! `src/lib/types.ts` mirror them one to one.

use serde::{Deserialize, Serialize};

pub const GB: u64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Safe,
    Review,
    Risky,
}

impl Risk {
    /// Invariant 11: unknown means less safe. Raise the risk one level.
    pub fn raised(self) -> Risk {
        match self {
            Risk::Safe => Risk::Review,
            Risk::Review | Risk::Risky => Risk::Risky,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Risk::Safe => "safe",
            Risk::Review => "review",
            Risk::Risky => "risky",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    NativeCommand,
    Delete,
    Trash,
    None,
}

/// Suggestion groups (spec "Data model"): the areas the user chooses
/// Where from, and the groups in Everything I found, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Docker,
    Projects,
    Caches,
    AiModels,
    PhonesEmulators,
    OldToolsApps,
    YourFiles,
    System,
}

impl Group {
    /// The disk-map area a group's items fall in, by default (prototype TILES ids).
    pub fn default_area(self) -> &'static str {
        match self {
            Group::Docker => "docker",
            Group::Projects => "projects",
            Group::Caches => "caches",
            Group::AiModels => "ai",
            Group::PhonesEmulators => "mobile",
            Group::OldToolsApps => "apps",
            Group::YourFiles => "files",
            Group::System => "system",
        }
    }
    pub fn rank(self) -> usize {
        Self::ALL.iter().position(|g| *g == self).unwrap_or(usize::MAX)
    }
    pub const ALL: [Group; 8] = [
        Group::Docker,
        Group::Projects,
        Group::Caches,
        Group::AiModels,
        Group::PhonesEmulators,
        Group::OldToolsApps,
        Group::YourFiles,
        Group::System,
    ];
}

/// A user's standing decision about an item (spec `item_decisions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemDecision {
    /// Keep, don't ask again: out of the pile and all automatic choices.
    Keep,
    /// Ask me later: skipped until the finished state or the next scan.
    Later,
}

/// One splittable unit of a suggestion: a project, model, emulator, backup,
/// app or version (spec `SuggestionPart`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionPart {
    pub id: String,
    pub label: String,
    pub location: String,
    pub size_bytes: u64,
    pub selected: bool,
}

/// The common shape returned by every detector (source contract, no fields dropped).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: String,
    pub detector: String,
    pub group: Group,
    /// The disk-map area this item's bytes belong to.
    pub map_area: String,
    pub title: String,
    pub location: String,
    /// On-disk size of the selected parts (all of it when there are no parts).
    pub size_bytes: u64,
    /// Size of every part, selected or not.
    #[serde(default)]
    pub full_size_bytes: u64,
    pub last_used_at: Option<String>,
    pub last_used_label: String,
    pub risk: Risk,
    pub action: Action,
    pub action_label: String,
    pub reason: String,
    pub recovery: String,
    pub recoverable: bool,
    pub blocked_reason: Option<String>,
    pub needs_confirmation: bool,
    /// Empty when the item can't be split.
    #[serde(default)]
    pub parts: Vec<SuggestionPart>,
    pub fingerprint: String,
    /// The user's Keep or Ask me later decision, if any.
    #[serde(default)]
    pub decision: Option<ItemDecision>,
    // Additions beyond the source contract (never replacing a source field).
    /// True when the size is an estimate; the UI prefixes it with "about".
    #[serde(default)]
    pub size_is_estimate: bool,
    /// Set when a previous cleanup skipped this item, with the reason.
    #[serde(default)]
    pub skip_reason: Option<String>,
    /// Name shown in the per-item acknowledgment ("billing_pgdata volume").
    #[serde(default)]
    pub ack_name: Option<String>,
    /// Why this item is Review or Risky (risk_notes.rs). Empty for Safe.
    #[serde(default)]
    pub risk_why: Vec<String>,
    /// Commands the user can run first to make removing it safer. Only shown;
    /// Freeboard never runs them.
    #[serde(default)]
    pub safer: Vec<SaferStep>,
    /// Git worktrees: what the Worktrees view shows (spec "Worktrees view").
    #[serde(default)]
    pub worktree: Option<WorktreeInfo>,
}

/// Where a worktree stands (spec "Git worktrees"): removing one deletes its
/// folder only; the branch and its commits stay, so only uncommitted files
/// can be lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WtStatus {
    /// Clean, idle 7+ days, not locked: removable.
    Ready,
    /// Used in the last 7 days, or locked: selectable by hand only.
    Look,
    /// Uncommitted files, a merge in progress, open in a program, commits on
    /// no branch, or a state that can't be read: not removed.
    Work,
}

/// How the worktree's branch relates to the default branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WtMerged {
    /// An ancestor of the default branch.
    Yes,
    /// Not an ancestor, but merging it changes nothing (`git merge-tree`).
    Squash,
    No,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    /// The repository's folder name.
    pub repo: String,
    /// The main checkout, as shown ("~/proj/app").
    pub root: String,
    pub branch: Option<String>,
    /// The worktree's folder, relative to the main checkout when inside it.
    pub rel_path: String,
    pub status: WtStatus,
    /// "agent", "claude" or "codex" for branches coding agents make.
    pub agent: Option<String>,
    pub locked: bool,
    /// Days since the last commit, checkout or change at its top level.
    pub idle_days: Option<u32>,
    /// Uncommitted and untracked files, and up to eight of their names.
    pub dirty: u32,
    pub dirty_files: Vec<String>,
    pub merged: WtMerged,
    /// Commits on the branch that the default branch does not have.
    pub ahead: u32,
    /// The branch has an upstream.
    pub pushed: bool,
    /// Why it is Work when not for uncommitted files: "merging", "in_use",
    /// "no_branch" or "unknown".
    pub hold: Option<String>,
    /// The exact commands removal runs, in order.
    pub commands: Vec<String>,
    /// How to get the worktree back when the branch stays.
    pub reopen: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaferStep {
    pub label: String,
    pub command: String,
}

impl Suggestion {
    /// Only eligible Safe items start selected (invariants 9 and 10).
    pub fn preselected(&self) -> bool {
        self.risk == Risk::Safe && self.blocked_reason.is_none() && !self.needs_confirmation
    }
    pub fn selectable(&self) -> bool {
        self.blocked_reason.is_none() && self.action != Action::None
    }
}

/// Backend-only description of how to act on a suggestion. Never sent to the
/// frontend as an executable request; the frontend only ever sends IDs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    /// One or more filesystem paths (permanent delete or Trash).
    Paths { paths: Vec<String> },
    /// A native tool command with a bounded resource set.
    Native {
        program: String,
        args: Vec<String>,
        /// The exact resource identities the command is allowed to touch
        /// (image IDs, container IDs, volume names, cache entries).
        resources: Vec<String>,
        /// Working directory, when the tool needs one (git).
        cwd: Option<String>,
    },
    /// Nothing can be done (blocked items).
    None,
}

/// A detector result: the public suggestion plus backend-owned target data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub suggestion: Suggestion,
    pub target: Target,
    /// For an item split into parts: one complete finding per part, in the
    /// order of `suggestion.parts`, each with its own target and fingerprint,
    /// so every part goes through the normal pre-cleanup checks on its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Finding>,
}

impl Finding {
    pub fn new(suggestion: Suggestion, target: Target) -> Finding {
        Finding { suggestion, target, parts: Vec::new() }
    }
    /// The filesystem paths this finding acts on (empty for tool commands).
    pub fn paths(&self) -> &[String] {
        match &self.target {
            Target::Paths { paths } => paths,
            _ => &[],
        }
    }
}

/// One item of a plan: the suggestion and the parts it covers (spec
/// `CleanupPlan` items `{ suggestion_id, part_ids }`). `part_ids` is empty for
/// an item that is not split into parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub suggestion_id: String,
    pub part_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlan {
    pub id: String,
    pub created_at: String,
    pub expires_at: String,
    pub item_ids: Vec<String>,
    /// What the plan covers, item by item, with the selected parts.
    #[serde(default)]
    pub plan_items: Vec<PlanItem>,
    pub bytes_freed_now: u64,
    pub bytes_held_in_trash: u64,
    pub unrecoverable_ids: Vec<String>,
    // Additions for the interface (source fields above are unchanged).
    pub items: Vec<Suggestion>,
    pub needs_ack_ids: Vec<String>,
    pub acknowledged_ids: Vec<String>,
    pub unrecoverable_bytes: u64,
    pub free_bytes_now: u64,
    pub ready: bool,
    pub state: PlanState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanState {
    Open,
    Running,
    Finished,
    Expired,
    Invalidated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiskStatus {
    Healthy,
    Low,
    Critical,
}

impl DiskStatus {
    pub fn label(self) -> &'static str {
        match self {
            DiskStatus::Healthy => "Healthy",
            DiskStatus::Low => "Low",
            DiskStatus::Critical => "Critical",
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            DiskStatus::Healthy => "healthy",
            DiskStatus::Low => "low",
            DiskStatus::Critical => "critical",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "healthy" => Some(Self::Healthy),
            "low" => Some(Self::Low),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeStatus {
    pub id: String,
    pub name: String,
    pub mount_point: String,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub status: DiskStatus,
    /// macOS: APFS purgeable space reported separately (may differ from Finder).
    pub purgeable_bytes: Option<u64>,
    pub snapshot_count: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Forecast {
    pub bytes_per_day: f64,
    pub days_left: f64,
}

/// This week's pace for the menu bar window (`monitor::forecast::pace`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pace {
    /// Free space used over the last 7 days, cleanups left out. Negative
    /// when more was freed by hand than used.
    pub week_bytes: i64,
    pub bytes_per_day: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrendPoint {
    pub at: String,
    pub free_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanState {
    pub running: Option<String>,
    pub phase: Option<String>,
    pub last_finished_at: Option<String>,
    pub detector_errors: Vec<DetectorError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectorError {
    pub detector: String,
    /// "unavailable" | "timeout" | "permission" | "malformed" | "error"
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub volumes: Vec<VolumeStatus>,
    pub status: DiskStatus,
    pub forecast: Option<Forecast>,
    /// This week's pace (menu bar window).
    pub pace: Option<Pace>,
    pub trash_bytes: u64,
    pub trend: Vec<TrendPoint>,
    pub scan: ScanState,
    pub notifications_paused: bool,
    pub platform: String,
    pub first_run: bool,
    /// The first preferred system language Freeboard supports (en, nl, de,
    /// fr, es), for Settings → Language "Same as my Mac".
    pub system_language: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanKind {
    Quick,
    Deep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    Waiting,
    Checking,
    Running,
    Done,
    MovedToTrash,
    Skipped,
    Failed,
    Canceled,
    Interrupted,
}

impl ItemState {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemState::Waiting => "waiting",
            ItemState::Checking => "checking",
            ItemState::Running => "running",
            ItemState::Done => "done",
            ItemState::MovedToTrash => "moved_to_trash",
            ItemState::Skipped => "skipped",
            ItemState::Failed => "failed",
            ItemState::Canceled => "canceled",
            ItemState::Interrupted => "interrupted",
        }
    }
    pub fn parse(s: &str) -> ItemState {
        match s {
            "waiting" => ItemState::Waiting,
            "checking" => ItemState::Checking,
            "running" => ItemState::Running,
            "done" => ItemState::Done,
            "moved_to_trash" => ItemState::MovedToTrash,
            "skipped" => ItemState::Skipped,
            "failed" => ItemState::Failed,
            "canceled" => ItemState::Canceled,
            _ => ItemState::Interrupted,
        }
    }
    pub fn is_final(self) -> bool {
        !matches!(self, ItemState::Waiting | ItemState::Checking | ItemState::Running)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunItem {
    /// The item, or for a split item the part, this row records.
    pub item_id: String,
    /// For a part: the item it belongs to.
    #[serde(default)]
    pub parent_id: Option<String>,
    pub title: String,
    /// The action kind as recorded ("native_command", "delete", "trash").
    #[serde(default)]
    pub action: String,
    pub action_label: String,
    pub expected_bytes: u64,
    pub state: ItemState,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDetail {
    pub id: String,
    pub plan_id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    /// "running" | "finished" | "canceled" | "interrupted"
    pub state: String,
    pub free_before: u64,
    pub free_after: Option<u64>,
    pub expected_bytes: u64,
    pub measured_bytes_freed: Option<i64>,
    pub trash_bytes: u64,
    pub skipped: u32,
    pub failed: u32,
    pub involves_docker: bool,
    pub items: Vec<RunItem>,
    /// Free space measured when this was read (the progress sheet's boat).
    #[serde(default)]
    pub free_bytes_now: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: String,
    pub started_at: String,
    pub items: u32,
    pub measured_bytes_freed: Option<i64>,
    pub trash_bytes: u64,
    pub skipped: u32,
    /// Crates (plan items) removed or moved to the Trash, at least in part.
    #[serde(default)]
    pub removed: u32,
    pub state: String,
    /// The Keep me tidy rule that ran it; None for a cleanup the user confirmed.
    #[serde(default)]
    pub rule: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub runs: Vec<RunSummary>,
    pub page: u32,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashSummary {
    pub bytes: u64,
    pub items: u32,
    /// Opaque token that must be echoed back to `empty_freeboard_trash`.
    pub confirmation_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyTrashResult {
    pub emptied: u32,
    pub failed: u32,
    pub bytes: u64,
    pub free_bytes_after: u64,
}

// ---------------------------------------------------------------------------
// Events (names from the source "Events" table).

pub const EV_STATUS_CHANGED: &str = "status_changed";
pub const EV_SCAN_PROGRESS: &str = "scan_progress";
pub const EV_SCAN_FINISHED: &str = "scan_finished";
pub const EV_CLEANUP_ITEM_UPDATED: &str = "cleanup_item_updated";
pub const EV_CLEANUP_FINISHED: &str = "cleanup_finished";
pub const EV_PILE_CHANGED: &str = "pile_changed";
pub const EV_DISK_MAP_CHANGED: &str = "disk_map_changed";
pub const EV_BLIND_SPOTS_CHANGED: &str = "blind_spots_changed";
pub const EV_TIDY_RULE_RAN: &str = "tidy_rule_ran";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusChanged {
    pub volume: String,
    pub status: DiskStatus,
    pub free_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scan_id: String,
    pub phase: String,
    pub detector: Option<String>,
    pub current_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanFinished {
    pub scan_id: String,
    pub detector_errors: Vec<DetectorError>,
    pub totals: ScanTotals,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanTotals {
    pub count: u32,
    pub bytes: u64,
    pub safe_bytes: u64,
    pub review_bytes: u64,
    pub risky_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItemUpdated {
    pub run_id: String,
    pub item_id: String,
    pub state: ItemState,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupFinished {
    pub run_id: String,
    pub measured_bytes_freed: i64,
    pub skipped_count: u32,
}

/// Sink for backend events. The Tauri shell forwards these to the webview;
/// tests collect them in memory.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
}

#[derive(Default)]
pub struct NullSink;
impl EventSink for NullSink {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}
}

#[derive(Default)]
pub struct MemorySink(pub std::sync::Mutex<Vec<(String, serde_json::Value)>>);
impl EventSink for MemorySink {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        self.0.lock().unwrap().push((event.to_string(), payload));
    }
}
impl MemorySink {
    pub fn events(&self, name: &str) -> Vec<serde_json::Value> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
            .collect()
    }
}
