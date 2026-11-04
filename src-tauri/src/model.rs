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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Docker,
    Worktrees,
    Builds,
    Caches,
    Files,
}

impl Category {
    pub const ORDER: [Category; 5] = [
        Category::Docker,
        Category::Worktrees,
        Category::Builds,
        Category::Caches,
        Category::Files,
    ];
    pub fn rank(self) -> usize {
        Self::ORDER.iter().position(|c| *c == self).unwrap_or(usize::MAX)
    }
}

/// The common shape returned by every detector (source contract, no fields dropped).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: String,
    pub detector: String,
    pub category: Category,
    pub title: String,
    pub location: String,
    pub size_bytes: u64,
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
    pub fingerprint: String,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPlan {
    pub id: String,
    pub created_at: String,
    pub expires_at: String,
    pub item_ids: Vec<String>,
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
    pub trash_bytes: u64,
    pub trend: Vec<TrendPoint>,
    pub scan: ScanState,
    pub notifications_paused: bool,
    pub platform: String,
    pub first_run: bool,
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
    pub item_id: String,
    pub title: String,
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
    pub state: String,
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
