//! Cleanup engine: saved plans, acknowledgments, revalidation, guarded
//! execution, Trash scope and history.
//!
//! Interface contract used by `core.rs`:
//! `CleanupEngine::{new, create_plan, get_plan, acknowledge_item, run_plan,
//! cancel_run, get_run, list_history, trash_summary, empty_freeboard_trash,
//! recover_interrupted}`.
//!
//! Flow (source "Cleanup and monitoring"): the backend saves an immutable plan
//! snapshot; the user confirms that exact plan (`run_plan`); each item is
//! checked again immediately before its action; changed or unknown items are
//! skipped with a reason; one item failing never stops the rest; free space is
//! measured before and after.

pub mod persist;
pub mod trash;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::detectors::common::{hash_hex, rfc3339};
use crate::detectors::{self, docker, git, Probe};
use crate::guard::Guard;
use crate::model::*;
use crate::runner::{Cmd, Confirmed, Runner};
use crate::scan::size::{measure, MeasureOpts};
use crate::scan::ScanContext;
use crate::settings::Settings;
use crate::store::{now_secs, Store};

use self::persist::PlanRow;
use self::trash::{NativeTrash, PurgeOutcome, TrashBackend, TrashRecord};

pub const PLAN_TTL_SECS: i64 = 30 * 60;
pub const HISTORY_PAGE_SIZE: u32 = 20;
pub const TRASH_UNAVAILABLE: &str = "Trash unavailable";

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CleanupError {
    #[error("unknown item: {0}")]
    UnknownItem(String),
    #[error("unknown plan")]
    UnknownPlan,
    #[error("unknown run")]
    UnknownRun,
    #[error("this plan has expired; create a new one")]
    Expired,
    #[error("a newer scan replaced these suggestions; create a new plan")]
    Invalidated,
    #[error("the plan changed after it was saved")]
    Altered,
    #[error("acknowledge every item that needs it first")]
    NeedsAcknowledgment,
    #[error("item cannot be selected: {0}")]
    Blocked(String),
    #[error("another cleanup is running")]
    Busy,
    #[error("the plan is empty")]
    Empty,
    #[error("invalid confirmation")]
    BadToken,
    #[error("this plan was already used; create a new one")]
    AlreadyUsed,
    #[error("permanent deletion is only offered for items whose move to the Trash failed")]
    NotEligible(String),
    #[error("{0}")]
    Internal(String),
}

impl From<rusqlite::Error> for CleanupError {
    fn from(e: rusqlite::Error) -> Self {
        CleanupError::Internal(e.to_string())
    }
}

/// Returns current free bytes of the volume holding the user's home.
pub type FreeSpaceFn = Arc<dyn Fn() -> u64 + Send + Sync>;
pub type SettingsFn = Arc<dyn Fn() -> Settings + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlanSnapshot {
    findings: Vec<Finding>,
    /// A separately confirmed permanent-deletion plan for items whose move to
    /// the Trash failed.
    permanent: bool,
}

struct Job {
    run_id: String,
    plan_id: String,
    snapshot: PlanSnapshot,
    confirmed: Confirmed,
    free_before: u64,
    cancel: Arc<AtomicBool>,
}

pub struct CleanupEngine {
    pub store: Arc<Store>,
    pub guard: Guard,
    pub runner: Runner,
    pub events: Arc<dyn EventSink>,
    pub free_space: FreeSpaceFn,
    trash: Arc<dyn TrashBackend>,
    home: PathBuf,
    settings_fn: Option<SettingsFn>,
    platform: &'static str,
    active: Mutex<Option<String>>,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    secret: String,
    counter: AtomicU64,
}

fn digest(snapshot: &str) -> String {
    hash_hex(snapshot)
}

impl CleanupEngine {
    pub fn new(store: Arc<Store>, guard: Guard, runner: Runner, events: Arc<dyn EventSink>, free_space: FreeSpaceFn) -> Self {
        let home = guard.home().to_path_buf();
        let seed = format!(
            "{:?}|{}|{:p}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos(),
            std::process::id(),
            &store
        );
        CleanupEngine {
            store,
            guard,
            runner,
            events,
            free_space,
            trash: Arc::new(NativeTrash),
            home,
            settings_fn: None,
            platform: crate::platform::platform_name(),
            active: Mutex::new(None),
            cancels: Mutex::new(HashMap::new()),
            secret: hash_hex(&seed),
            counter: AtomicU64::new(0),
        }
    }

    /// Home folder and settings used by the pre-cleanup checks. Defaults:
    /// the guard's home and settings loaded from the store.
    pub fn with_context(mut self, home: PathBuf, settings_fn: SettingsFn) -> Self {
        self.home = home;
        self.settings_fn = Some(settings_fn);
        self
    }

    /// Replace the Trash backend (tests use a directory-backed Trash).
    pub fn with_trash(mut self, trash: Arc<dyn TrashBackend>) -> Self {
        self.trash = trash;
        self
    }

    pub fn with_platform(mut self, platform: &'static str) -> Self {
        self.platform = platform;
        self
    }

    fn settings(&self) -> Settings {
        match &self.settings_fn {
            Some(f) => f(),
            None => self.store.load_settings().unwrap_or_default(),
        }
    }

    fn new_id(&self, kind: &str) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        format!("{kind}_{}", &hash_hex(&format!("{}|{kind}|{n}|{}", self.secret, now_secs()))[..20])
    }

    fn emit<T: Serialize>(&self, name: &str, payload: &T) {
        if let Ok(v) = serde_json::to_value(payload) {
            self.events.emit(name, v);
        }
    }

    // ----- plans ---------------------------------------------------------

    pub fn create_plan(&self, item_ids: &[String], now: i64) -> Result<CleanupPlan, CleanupError> {
        self.create(item_ids, now, false)
    }

    /// A new, separately confirmed plan that permanently deletes items whose
    /// move to the Trash failed because no usable Trash exists. Every item is
    /// marked unrecoverable and needs its own acknowledgment.
    pub fn create_permanent_plan(&self, item_ids: &[String], now: i64) -> Result<CleanupPlan, CleanupError> {
        self.create(item_ids, now, true)
    }

    fn create(&self, item_ids: &[String], now: i64, permanent: bool) -> Result<CleanupPlan, CleanupError> {
        let mut ids: Vec<String> = vec![];
        for id in item_ids {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        if ids.is_empty() {
            return Err(CleanupError::Empty);
        }
        let all = self.store.findings().map_err(|e| CleanupError::Internal(e.to_string()))?;
        let mut picked = vec![];
        for id in &ids {
            let f = all.iter().find(|f| &f.suggestion.id == id).ok_or_else(|| CleanupError::UnknownItem(id.clone()))?;
            let s = &f.suggestion;
            if let Some(r) = &s.blocked_reason {
                return Err(CleanupError::Blocked(r.clone()));
            }
            if s.action == Action::None || matches!(f.target, Target::None) {
                return Err(CleanupError::Blocked("No cleanup action is available.".into()));
            }
            // Items split into parts are cleaned part by part, each through
            // its own pre-cleanup checks. Until that lands (task 4.2), refuse
            // them rather than act on all parts at once.
            if !f.parts.is_empty() || !s.parts.is_empty() {
                return Err(CleanupError::Blocked("Items with parts can't be cleaned up yet.".into()));
            }
            let mut f = f.clone();
            f.suggestion.skip_reason = None;
            if permanent {
                let eligible = s.action == Action::Trash && persist::trash_failed(&self.store.conn(), id)?;
                if !eligible {
                    return Err(CleanupError::NotEligible(id.clone()));
                }
                let s = &mut f.suggestion;
                s.action = Action::Delete;
                s.action_label = "Delete permanently".into();
                s.recovery = "Not recoverable. No Trash is available for it.".into();
                s.recoverable = false;
                s.needs_confirmation = true;
                s.ack_name = Some(s.title.clone());
            }
            picked.push(f);
        }
        // Biggest wins first: the plan lists, and the run cleans, the largest
        // items first (requested by the user, 2026-11-05). Stable, so equal
        // sizes keep the selection order.
        picked.sort_by(|a, b| b.suggestion.size_bytes.cmp(&a.suggestion.size_bytes));
        let snap = PlanSnapshot { findings: picked, permanent };
        let snapshot = serde_json::to_string(&snap).map_err(|e| CleanupError::Internal(e.to_string()))?;
        let row = PlanRow {
            id: self.new_id("plan"),
            created_at: now,
            expires_at: now + PLAN_TTL_SECS,
            generation: self.store.scan_generation(),
            digest: digest(&snapshot),
            snapshot,
            state: "open".into(),
        };
        persist::insert_plan(&self.store.conn(), &row)?;
        self.view(&row, now)
    }

    /// Current state of a plan row, persisting expiry and invalidation.
    fn state_of(&self, row: &PlanRow, now: i64) -> Result<PlanState, CleanupError> {
        let st = match row.state.as_str() {
            "open" => {
                if row.generation != self.store.scan_generation() {
                    persist::set_plan_state(&self.store.conn(), &row.id, "invalidated")?;
                    PlanState::Invalidated
                } else if now >= row.expires_at {
                    persist::set_plan_state(&self.store.conn(), &row.id, "expired")?;
                    PlanState::Expired
                } else {
                    PlanState::Open
                }
            }
            "running" => PlanState::Running,
            "finished" => PlanState::Finished,
            "expired" => PlanState::Expired,
            _ => PlanState::Invalidated,
        };
        Ok(st)
    }

    fn view(&self, row: &PlanRow, now: i64) -> Result<CleanupPlan, CleanupError> {
        let snap: PlanSnapshot = serde_json::from_str(&row.snapshot).map_err(|_| CleanupError::Altered)?;
        let state = self.state_of(row, now)?;
        let acks = persist::acks(&self.store.conn(), &row.id)?;
        let items: Vec<Suggestion> = snap.findings.iter().map(|f| f.suggestion.clone()).collect();
        let needs_ack_ids: Vec<String> = items.iter().filter(|s| s.needs_confirmation).map(|s| s.id.clone()).collect();
        let unrecoverable: Vec<&Suggestion> = items.iter().filter(|s| !s.recoverable).collect();
        let intact = digest(&row.snapshot) == row.digest;
        Ok(CleanupPlan {
            id: row.id.clone(),
            created_at: rfc3339(row.created_at).unwrap_or_default(),
            expires_at: rfc3339(row.expires_at).unwrap_or_default(),
            item_ids: items.iter().map(|s| s.id.clone()).collect(),
            bytes_freed_now: items.iter().filter(|s| s.action != Action::Trash).map(|s| s.size_bytes).sum(),
            bytes_held_in_trash: items.iter().filter(|s| s.action == Action::Trash).map(|s| s.size_bytes).sum(),
            unrecoverable_ids: unrecoverable.iter().map(|s| s.id.clone()).collect(),
            unrecoverable_bytes: unrecoverable.iter().map(|s| s.size_bytes).sum(),
            ready: intact && state == PlanState::Open && needs_ack_ids.iter().all(|i| acks.contains(i)),
            acknowledged_ids: acks.into_iter().filter(|a| needs_ack_ids.contains(a)).collect(),
            needs_ack_ids,
            free_bytes_now: (self.free_space)(),
            items,
            state,
        })
    }

    fn row(&self, plan_id: &str) -> Result<PlanRow, CleanupError> {
        persist::load_plan(&self.store.conn(), plan_id)?.ok_or(CleanupError::UnknownPlan)
    }

    pub fn get_plan(&self, plan_id: &str, now: i64) -> Result<CleanupPlan, CleanupError> {
        let row = self.row(plan_id)?;
        self.view(&row, now)
    }

    pub fn acknowledge_item(&self, plan_id: &str, item_id: &str, now: i64) -> Result<CleanupPlan, CleanupError> {
        let row = self.row(plan_id)?;
        match self.state_of(&row, now)? {
            PlanState::Open => {}
            PlanState::Expired => return Err(CleanupError::Expired),
            PlanState::Invalidated => return Err(CleanupError::Invalidated),
            _ => return Err(CleanupError::AlreadyUsed),
        }
        let snap: PlanSnapshot = serde_json::from_str(&row.snapshot).map_err(|_| CleanupError::Altered)?;
        if !snap.findings.iter().any(|f| f.suggestion.id == item_id && f.suggestion.needs_confirmation) {
            return Err(CleanupError::UnknownItem(item_id.into()));
        }
        persist::add_ack(&self.store.conn(), plan_id, item_id, now)?;
        self.view(&row, now)
    }

    // ----- runs ----------------------------------------------------------

    /// Validate the confirmed plan and start it on a background thread.
    pub fn run_plan(self: &Arc<Self>, plan_id: &str, now: i64) -> Result<String, CleanupError> {
        let job = self.start(plan_id, now)?;
        let run_id = job.run_id.clone();
        let me = self.clone();
        std::thread::Builder::new()
            .name("freeboard-cleanup".into())
            .spawn(move || me.execute(job))
            .map_err(|e| CleanupError::Internal(e.to_string()))?;
        Ok(run_id)
    }

    /// Same as `run_plan` but runs on the calling thread (tests, CLI).
    pub fn run_plan_blocking(&self, plan_id: &str, now: i64) -> Result<String, CleanupError> {
        let job = self.start(plan_id, now)?;
        let run_id = job.run_id.clone();
        self.execute(job);
        Ok(run_id)
    }

    fn start(&self, plan_id: &str, now: i64) -> Result<Job, CleanupError> {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.is_some() || !persist::running_runs(&self.store.conn())?.is_empty() {
            return Err(CleanupError::Busy);
        }
        let row = self.row(plan_id)?;
        if digest(&row.snapshot) != row.digest {
            return Err(CleanupError::Altered);
        }
        match self.state_of(&row, now)? {
            PlanState::Open => {}
            PlanState::Expired => return Err(CleanupError::Expired),
            PlanState::Invalidated => return Err(CleanupError::Invalidated),
            PlanState::Running | PlanState::Finished => return Err(CleanupError::AlreadyUsed),
        }
        let snapshot: PlanSnapshot = serde_json::from_str(&row.snapshot).map_err(|_| CleanupError::Altered)?;
        if snapshot.findings.is_empty() {
            return Err(CleanupError::Empty);
        }
        let acks = persist::acks(&self.store.conn(), plan_id)?;
        if snapshot.findings.iter().any(|f| f.suggestion.needs_confirmation && !acks.contains(&f.suggestion.id)) {
            return Err(CleanupError::NeedsAcknowledgment);
        }
        // The plan is valid and confirmed: only now is a Confirmed token made.
        let confirmed = Confirmed::new(plan_id);
        let run_id = self.new_id("run");
        let free_before = (self.free_space)();
        let items: Vec<(Suggestion, String)> = snapshot
            .findings
            .iter()
            .map(|f| (f.suggestion.clone(), serde_json::to_string(&f.suggestion.action).unwrap_or_default().trim_matches('"').to_string()))
            .collect();
        let expected: u64 = snapshot.findings.iter().filter(|f| f.suggestion.action != Action::Trash).map(|f| f.suggestion.size_bytes).sum();
        let docker = snapshot.findings.iter().any(|f| f.suggestion.group == Group::Docker);
        let plan_json = serde_json::to_string(&self.view(&row, now)?).unwrap_or_default();
        persist::insert_run(&mut self.store.conn(), &run_id, plan_id, &plan_json, now_secs(), free_before, expected, docker, &items)?;
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels.lock().unwrap_or_else(|e| e.into_inner()).insert(run_id.clone(), cancel.clone());
        *active = Some(run_id.clone());
        Ok(Job { run_id, plan_id: plan_id.into(), snapshot, confirmed, free_before, cancel })
    }

    fn set_item(&self, run: &str, item: &str, state: ItemState, reason: Option<&str>) {
        if let Err(e) = persist::set_item(&self.store.conn(), run, item, state, reason, now_secs()) {
            tracing::error!("could not record cleanup item state: {e}");
        }
        self.emit(
            EV_CLEANUP_ITEM_UPDATED,
            &CleanupItemUpdated { run_id: run.into(), item_id: item.into(), state, reason: reason.map(String::from) },
        );
    }

    fn execute(&self, job: Job) {
        let settings = self.settings();
        let scan_cancel = AtomicBool::new(false);
        let no_pause = || false;
        let no_progress = |_: &str, _: Option<&str>, _: u64| {};
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut canceled = false;
            for (i, f) in job.snapshot.findings.iter().enumerate() {
                let id = f.suggestion.id.as_str();
                if job.cancel.load(Ordering::SeqCst) {
                    canceled = true;
                    for rest in &job.snapshot.findings[i..] {
                        self.set_item(&job.run_id, &rest.suggestion.id, ItemState::Canceled, Some("Canceled before it started."));
                    }
                    break;
                }
                self.set_item(&job.run_id, id, ItemState::Checking, None);
                let ctx = ScanContext {
                    runner: &self.runner,
                    home: self.home.clone(),
                    settings: settings.clone(),
                    now: now_secs(),
                    kind: ScanKind::Deep,
                    cancel: &scan_cancel,
                    should_pause: &no_pause,
                    progress: &no_progress,
                    platform: self.platform,
                };
                match detectors::probe_with(f, &ctx, &self.guard) {
                    Probe::Unchanged => {}
                    Probe::Changed(r) | Probe::Unknown(r) => {
                        let _ = self.store.set_skip_note(id, &r);
                        self.set_item(&job.run_id, id, ItemState::Skipped, Some(&r));
                        continue;
                    }
                }
                self.set_item(&job.run_id, id, ItemState::Running, None);
                let (state, reason) = self.act(f, &job, &ctx);
                self.set_item(&job.run_id, id, state, reason.as_deref());
                if matches!(state, ItemState::Done | ItemState::MovedToTrash) {
                    let _ = self.store.clear_skip_note(id);
                    let _ = persist::delete_suggestion(&self.store.conn(), id);
                }
            }
            canceled
        }));
        let free_after = (self.free_space)();
        let state = match result {
            Ok(false) => "finished",
            Ok(true) => "canceled",
            Err(_) => "interrupted",
        };
        let c = self.store.conn();
        if state == "interrupted" {
            let _ = persist::mark_interrupted(&c, &job.run_id, &job.plan_id, now_secs());
        }
        let _ = persist::finish_run(&c, &job.run_id, &job.plan_id, state, free_after, now_secs());
        drop(c);
        let skipped = self.get_run(&job.run_id).map(|r| r.skipped).unwrap_or(0);
        self.cancels.lock().unwrap_or_else(|e| e.into_inner()).remove(&job.run_id);
        *self.active.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.emit(
            EV_CLEANUP_FINISHED,
            &CleanupFinished { run_id: job.run_id.clone(), measured_bytes_freed: free_after as i64 - job.free_before as i64, skipped_count: skipped },
        );
    }

    /// Perform one item's action. Returns the final state and reason.
    fn act(&self, f: &Finding, job: &Job, ctx: &ScanContext) -> (ItemState, Option<String>) {
        let s = &f.suggestion;
        if s.detector == git::ID {
            return self.remove_worktree(f, job);
        }
        match (&f.target, s.action) {
            (Target::Paths { paths }, Action::Delete) => {
                for p in paths {
                    let checked = match self.guard.check(Path::new(p)) {
                        Ok(c) => c,
                        Err(e) => return (ItemState::Failed, Some(format!("Refused: {e}."))),
                    };
                    if let Err(e) = self.guard.remove(&checked) {
                        return (ItemState::Failed, Some(format!("Could not delete it: {e}.")));
                    }
                }
                (ItemState::Done, None)
            }
            (Target::Paths { paths }, Action::Trash) if !job.snapshot.permanent => {
                let mut moved = 0;
                for p in paths {
                    let checked = match self.guard.check(Path::new(p)) {
                        Ok(c) => c,
                        Err(e) => return (ItemState::Failed, Some(format!("Refused: {e}."))),
                    };
                    let bytes = measure(&checked.canonical, &MeasureOpts::default()).reclaimable();
                    // Taken before the move; a rename into the Trash keeps it.
                    let identity = trash::trash_identity(&checked.canonical);
                    if let Err(e) = self.trash.trash(&self.guard, &checked) {
                        let note = if moved > 0 { format!(" {moved} of {} items were already moved.", paths.len()) } else { String::new() };
                        return (
                            ItemState::Failed,
                            Some(format!(
                                "{TRASH_UNAVAILABLE}: {e}. Nothing else was changed.{note} You can plan a permanent deletion separately."
                            )),
                        );
                    }
                    moved += 1;
                    let rec = TrashRecord {
                        id: 0,
                        run_id: job.run_id.clone(),
                        item_id: s.id.clone(),
                        original_path: checked.canonical.to_string_lossy().to_string(),
                        trashed_at: now_secs(),
                        bytes,
                        identity,
                    };
                    if let Err(e) = persist::insert_trash(&self.store.conn(), &rec) {
                        tracing::error!("could not record Trash item: {e}");
                    }
                }
                (ItemState::MovedToTrash, None)
            }
            (Target::Native { program, args, resources, cwd }, Action::NativeCommand) => {
                if cwd.is_some() {
                    return (ItemState::Failed, Some("Commands that need a working folder are not run in v1.".into()));
                }
                let scanned_at: i64 = resources.iter().find_map(|r| r.strip_prefix("scanned_at=")).and_then(|v| v.parse().ok()).unwrap_or(ctx.now);
                let args: Vec<String> = args
                    .iter()
                    .map(|a| if a == docker::UNTIL_PLACEHOLDER { format!("until={}s", (ctx.now - scanned_at).max(1)) } else { a.clone() })
                    .collect();
                let cmd = Cmd::new(program, args.iter());
                match self.runner.run_action(&cmd, &job.confirmed) {
                    Ok(out) if out.success() => (ItemState::Done, None),
                    Ok(out) => {
                        let msg = out.stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("The command failed.").trim().to_string();
                        (ItemState::Failed, Some(msg))
                    }
                    Err(e) => (ItemState::Failed, Some(e.to_string())),
                }
            }
            _ => (ItemState::Failed, Some("This action is not supported.".into())),
        }
    }

    /// `git worktree remove <path>` for a linked worktree (decision
    /// 2026-11-04, task 4.1). Never uses --force: Git itself refuses a worktree
    /// with local changes, which is a second safety net behind the pre-cleanup
    /// probe that already confirmed it is clean and merged or pushed.
    fn remove_worktree(&self, f: &Finding, job: &Job) -> (ItemState, Option<String>) {
        let Target::Native { program, args, resources, cwd: Some(main) } = &f.target else {
            return (ItemState::Failed, Some("Unexpected cleanup target.".into()));
        };
        let (Some(path), Some(expected)) = (args.get(2), resources.first()) else {
            return (ItemState::Failed, Some("Unexpected cleanup target.".into()));
        };
        if program != "git" || args.len() != 3 || args[0] != "worktree" || args[1] != "remove" {
            return (ItemState::Failed, Some("Refused: unexpected worktree command.".into()));
        }
        let checked = match self.guard.check_linked_worktree(Path::new(path), Path::new(main), Path::new(expected)) {
            Ok(c) => c,
            Err(e) => return (ItemState::Skipped, Some(format!("Refused: {e}."))),
        };
        let cmd = Cmd::new("git", ["worktree", "remove", &checked.canonical.to_string_lossy()]).in_dir(Path::new(main));
        match self.runner.run_action(&cmd, &job.confirmed) {
            Ok(out) if out.success() => (ItemState::Done, None),
            Ok(out) => {
                let msg = out.stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("git worktree remove failed.").trim().to_string();
                (ItemState::Failed, Some(msg))
            }
            Err(e) => (ItemState::Failed, Some(e.to_string())),
        }
    }

    /// Request cancellation. Takes effect between items; an item already
    /// running is never interrupted.
    pub fn cancel_run(&self, run_id: &str) -> bool {
        match self.cancels.lock().unwrap_or_else(|e| e.into_inner()).get(run_id) {
            Some(c) => {
                c.store(true, Ordering::SeqCst);
                true
            }
            None => false,
        }
    }

    pub fn is_running(&self) -> bool {
        self.active.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    pub fn get_run(&self, run_id: &str) -> Result<RunDetail, CleanupError> {
        persist::load_run(&self.store.conn(), run_id)?.ok_or(CleanupError::UnknownRun)
    }

    /// Wait for a run to leave the running state (tests and tools).
    pub fn wait_run(&self, run_id: &str, timeout: std::time::Duration) -> Result<RunDetail, CleanupError> {
        let end = std::time::Instant::now() + timeout;
        loop {
            let r = self.get_run(run_id)?;
            if r.state != "running" && !self.is_running() {
                return Ok(r);
            }
            if std::time::Instant::now() > end {
                return Ok(r);
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    pub fn list_history(&self, page: u32) -> Result<HistoryPage, CleanupError> {
        Ok(persist::list_runs(&self.store.conn(), page, HISTORY_PAGE_SIZE)?)
    }

    // ----- Trash ---------------------------------------------------------

    fn trash_token(&self, held: &[TrashRecord]) -> String {
        let ids: Vec<String> = held.iter().map(|r| r.id.to_string()).collect();
        hash_hex(&format!("{}|trash|{}", self.secret, ids.join(",")))[..32].to_string()
    }

    pub fn trash_summary(&self) -> TrashSummary {
        let held = persist::held_trash(&self.store.conn()).unwrap_or_default();
        TrashSummary {
            bytes: held.iter().map(|r| r.bytes).sum(),
            items: held.len() as u32,
            confirmation_token: if held.is_empty() { None } else { Some(self.trash_token(&held)) },
        }
    }

    /// Permanently delete only what Freeboard moved to the Trash. The token
    /// from `trash_summary` binds the confirmation to that exact set.
    pub fn empty_freeboard_trash(&self, token: &str) -> Result<EmptyTrashResult, CleanupError> {
        if self.is_running() {
            return Err(CleanupError::Busy);
        }
        let held = persist::held_trash(&self.store.conn())?;
        if held.is_empty() || token != self.trash_token(&held) {
            return Err(CleanupError::BadToken);
        }
        let (mut emptied, mut failed, mut bytes) = (0, 0, 0);
        for rec in &held {
            match self.trash.purge(&self.guard, rec) {
                Ok(PurgeOutcome::Purged) => {
                    emptied += 1;
                    bytes += rec.bytes;
                    persist::set_trash_state(&self.store.conn(), rec.id, "emptied")?;
                }
                Ok(PurgeOutcome::Gone) => persist::set_trash_state(&self.store.conn(), rec.id, "gone")?,
                Err(e) => {
                    failed += 1;
                    tracing::warn!("could not empty a Trash item: {e}");
                }
            }
        }
        Ok(EmptyTrashResult { emptied, failed, bytes, free_bytes_after: (self.free_space)() })
    }

    /// On startup: mark runs left `running` as interrupted. Never replays.
    pub fn recover_interrupted(&self) {
        let c = self.store.conn();
        if let Ok(runs) = persist::running_runs(&c) {
            for (run, plan) in runs {
                let _ = persist::mark_interrupted(&c, &run, &plan, now_secs());
            }
        }
    }
}
