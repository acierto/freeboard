//! Backend state shared by Tauri commands, the tray and background tasks.
//!
//! The core outlives the webview: monitoring, scheduled scans and cleanup
//! runs continue when the window is hidden or destroyed. Every command input
//! is an ID, enum or number (invariant 3); paths and commands are resolved
//! here from backend-owned state only.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};

use crate::cleanup::{CleanupEngine, CleanupError};
use crate::guard::Guard;
use crate::model::*;
use crate::monitor::{self, Monitor, VolumeSource};
use crate::runner::Runner;
use crate::scan::{self, Detector, ScanContext};
use crate::settings::{QuickScanSchedule, DeepScanSchedule, Settings, SettingsPatch, UiSettingsPatch};
use crate::store::Store;

/// Native surfaces owned by the shell (tray, notifications, autostart).
pub trait Shell: Send + Sync {
    fn notify(&self, title: &str, body: &str);
    fn update_tray(&self, status: DiskStatus, free_bytes: u64);
    fn set_autostart(&self, enabled: bool);
}

#[derive(Default)]
pub struct NullShell;
impl Shell for NullShell {
    fn notify(&self, _: &str, _: &str) {}
    fn update_tray(&self, _: DiskStatus, _: u64) {}
    fn set_autostart(&self, _: bool) {}
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        CommandError { code: code.into(), message: message.into() }
    }
}

impl From<CleanupError> for CommandError {
    fn from(e: CleanupError) -> Self {
        let code = match &e {
            CleanupError::UnknownItem(_) | CleanupError::UnknownPlan | CleanupError::UnknownRun => "unknown_id",
            CleanupError::Expired => "expired",
            CleanupError::Invalidated => "invalidated",
            CleanupError::Altered => "altered",
            CleanupError::NeedsAcknowledgment => "needs_acknowledgment",
            CleanupError::Blocked(_) => "blocked",
            CleanupError::Busy => "busy",
            CleanupError::Empty => "empty",
            CleanupError::BadToken => "bad_token",
            _ => "internal",
        };
        CommandError::new(code, e.to_string())
    }
}

impl From<crate::store::StoreError> for CommandError {
    fn from(e: crate::store::StoreError) -> Self {
        CommandError::new("storage", e.to_string())
    }
}

impl From<crate::settings::SettingsError> for CommandError {
    fn from(e: crate::settings::SettingsError) -> Self {
        CommandError::new("invalid_settings", e.to_string())
    }
}

pub type CmdResult<T> = Result<T, CommandError>;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuggestionFilter {
    pub risk: Option<Risk>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FolderList {
    ScanFolders,
    Exclusions,
}

/// Backend IDs are short opaque tokens. Anything that looks like a path or a
/// command is rejected before lookup (invariant 3).
pub fn validate_id(id: &str) -> CmdResult<()> {
    let ok = !id.is_empty() && id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
    if !ok || id.contains("..") {
        return Err(CommandError::new("invalid_id", "Expected an ID."));
    }
    Ok(())
}

pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

#[derive(Default)]
struct ScanRuntime {
    running: Option<String>,
    kind: Option<ScanKind>,
    phase: Option<String>,
    last_finished_at: Option<i64>,
    last_quick_at: Option<i64>,
    last_deep_at: Option<i64>,
}

pub struct AppCore {
    pub store: Arc<Store>,
    pub runner: Runner,
    pub home: PathBuf,
    settings: Arc<RwLock<Settings>>,
    monitor: Mutex<Monitor>,
    volumes: Arc<dyn VolumeSource>,
    pub cleanup: Arc<CleanupEngine>,
    events: Arc<dyn EventSink>,
    shell: RwLock<Arc<dyn Shell>>,
    detectors: Arc<Vec<Box<dyn Detector>>>,
    scan: Mutex<ScanRuntime>,
    scan_cancel: Arc<AtomicBool>,
    notifications_paused: AtomicBool,
    /// macOS local snapshot count per mount point, with the time it was read.
    snapshots: Mutex<std::collections::HashMap<String, (i64, Option<u32>)>>,
    clock: Clock,
    platform: &'static str,
}

pub struct CoreConfig {
    pub store: Arc<Store>,
    pub runner: Runner,
    pub home: PathBuf,
    pub volumes: Arc<dyn VolumeSource>,
    pub events: Arc<dyn EventSink>,
    pub detectors: Vec<Box<dyn Detector>>,
    pub clock: Clock,
    pub platform: &'static str,
    /// Extra protected locations (e.g. the app data directory).
    pub extra_protected: Vec<PathBuf>,
}

pub fn system_clock() -> Clock {
    Arc::new(|| chrono::Utc::now().timestamp())
}

fn iso(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0).map(|d| d.to_rfc3339()).unwrap_or_default()
}

impl AppCore {
    pub fn new(cfg: CoreConfig) -> Arc<AppCore> {
        let settings = cfg.store.load_settings().unwrap_or_default();
        let mut protected = cfg.extra_protected.clone();
        protected.extend(settings.scan_folders.iter().map(PathBuf::from));
        protected.push(crate::platform::downloads_dir(&cfg.home));
        let guard = Guard::for_system(&cfg.home, &protected);
        let vols = cfg.volumes.clone();
        let home = cfg.home.clone();
        let free_space: crate::cleanup::FreeSpaceFn = Arc::new(move || monitor::home_free_bytes(vols.as_ref(), &home));
        let shared_settings = Arc::new(RwLock::new(settings));
        let sfn = shared_settings.clone();
        let cleanup = Arc::new(
            CleanupEngine::new(cfg.store.clone(), guard, cfg.runner.clone(), cfg.events.clone(), free_space)
                .with_context(cfg.home.clone(), Arc::new(move || sfn.read().unwrap().clone()))
                .with_platform(cfg.platform),
        );
        cleanup.recover_interrupted();
        let mut mon = Monitor::new();
        mon.restore(&cfg.store);
        let paused = cfg.store.meta_get("notifications_paused").as_deref() == Some("true");
        Arc::new(AppCore {
            store: cfg.store,
            runner: cfg.runner,
            home: cfg.home,
            settings: shared_settings,
            monitor: Mutex::new(mon),
            volumes: cfg.volumes,
            cleanup,
            events: cfg.events,
            shell: RwLock::new(Arc::new(NullShell)),
            detectors: Arc::new(cfg.detectors),
            scan: Mutex::new(ScanRuntime::default()),
            scan_cancel: Arc::new(AtomicBool::new(false)),
            notifications_paused: AtomicBool::new(paused),
            snapshots: Mutex::new(Default::default()),
            clock: cfg.clock,
            platform: cfg.platform,
        })
    }

    pub fn set_shell(&self, shell: Arc<dyn Shell>) {
        *self.shell.write().unwrap() = shell;
    }

    fn shell(&self) -> Arc<dyn Shell> {
        self.shell.read().unwrap().clone()
    }

    pub fn now(&self) -> i64 {
        (self.clock)()
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    fn emit<T: Serialize>(&self, event: &str, payload: &T) {
        self.events.emit(event, serde_json::to_value(payload).unwrap_or_default());
    }

    // ----- monitoring -----------------------------------------------------

    /// One monitoring sample. Called every five minutes by the scheduler.
    pub fn tick_monitor(&self) -> CmdResult<()> {
        let now = self.now();
        let s = self.settings();
        let readings = self.volumes.read(&self.home, &s.extra_volumes);
        let paused = self.notifications_paused.load(Ordering::Relaxed);
        let tick = self.monitor.lock().unwrap().tick(now, &s, readings, &self.store, paused)?;
        for c in &tick.changes {
            self.emit(EV_STATUS_CHANGED, c);
        }
        let safe_gb = self.safe_bytes() as f64 / GB as f64;
        for (_vol, kind, free) in &tick.notices {
            let n = monitor::notify::notice_text(*kind, *free as f64 / GB as f64, safe_gb);
            self.shell().notify(&n.title, &n.body);
            let _ = self.store.meta_set("notification_pending_view", "suggestions");
        }
        if let Some(home) = tick.volumes.first() {
            self.shell().update_tray(home.status, home.free_bytes);
        }
        Ok(())
    }

    fn safe_bytes(&self) -> u64 {
        self.store
            .findings()
            .map(|f| f.iter().filter(|f| f.suggestion.preselected()).map(|f| f.suggestion.size_bytes).sum())
            .unwrap_or(0)
    }

    /// Navigation requested by a notification click, consumed once.
    pub fn take_pending_view(&self) -> Option<String> {
        let v = self.store.meta_get("notification_pending_view").filter(|v| !v.is_empty());
        if v.is_some() {
            let _ = self.store.meta_set("notification_pending_view", "");
        }
        v
    }

    fn cleanup_times(&self) -> Vec<i64> {
        let c = self.store.conn();
        let mut v: Vec<i64> = c
            .prepare("SELECT finished_at FROM cleanup_runs WHERE finished_at IS NOT NULL")
            .and_then(|mut st| st.query_map([], |r| r.get::<_, i64>(0)).and_then(|r| r.collect()))
            .unwrap_or_default();
        if let Ok(mut st) = c.prepare("SELECT value FROM meta WHERE key = 'trash_emptied_at'") {
            if let Ok(t) = st.query_row([], |r| r.get::<_, String>(0)) {
                if let Ok(t) = t.parse() {
                    v.push(t);
                }
            }
        }
        v
    }

    // ----- commands -------------------------------------------------------

    pub fn get_status(&self) -> CmdResult<AppStatus> {
        if self.monitor.lock().unwrap().latest().is_empty() {
            self.tick_monitor()?;
        }
        let now = self.now();
        let (mut volumes, status) = {
            let m = self.monitor.lock().unwrap();
            (m.latest().to_vec(), m.overall())
        };
        self.add_platform_details(&mut volumes, now);
        let samples = self.store.samples("home", now - monitor::forecast::WINDOW_SECS)?;
        let mut pts: Vec<(i64, u64)> = samples.iter().map(|(t, f, _)| (*t, *f)).collect();
        if let Some(h) = volumes.first() {
            pts.push((now, h.free_bytes));
        }
        let forecast = monitor::forecast::forecast(&pts, now, &self.cleanup_times());
        let mut trend = monitor::daily_trend(&samples, now);
        if let Some(h) = volumes.first() {
            trend.push(TrendPoint { at: iso(now), free_bytes: h.free_bytes });
        }
        let scan = self.scan_state();
        let s = self.settings();
        Ok(AppStatus {
            volumes,
            status,
            forecast,
            trash_bytes: self.cleanup.trash_summary().bytes,
            trend,
            scan,
            notifications_paused: self.notifications_paused.load(Ordering::Relaxed),
            platform: self.platform.to_string(),
            first_run: !s.onboarding_complete,
        })
    }

    /// macOS: APFS purgeable space and Time Machine local snapshots, reported
    /// separately because Finder may count them as free (platform notes).
    #[allow(unused_variables)]
    fn add_platform_details(&self, volumes: &mut [VolumeStatus], now: i64) {
        #[cfg(target_os = "macos")]
        for v in volumes.iter_mut() {
            let mount = PathBuf::from(&v.mount_point);
            v.purgeable_bytes = crate::platform::macos::purgeable_bytes(&mount, v.free_bytes);
            let mut cache = self.snapshots.lock().unwrap();
            let fresh = cache.get(&v.mount_point).filter(|(t, _)| now - t < 600).map(|(_, c)| *c);
            v.snapshot_count = match fresh {
                Some(c) => c,
                None => {
                    let c = crate::platform::macos::local_snapshot_count(&mount, &self.runner);
                    cache.insert(v.mount_point.clone(), (now, c));
                    c
                }
            };
        }
    }

    fn scan_state(&self) -> ScanState {
        let r = self.scan.lock().unwrap();
        let errors: Vec<DetectorError> = self
            .store
            .meta_get("detector_errors")
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default();
        let last = self.store.meta_get("last_scan_at").and_then(|v| v.parse::<i64>().ok()).or(r.last_finished_at);
        ScanState { running: r.running.clone(), phase: r.phase.clone(), last_finished_at: last.map(iso), detector_errors: errors }
    }

    pub fn start_scan(self: &Arc<Self>, kind: ScanKind) -> CmdResult<String> {
        let id = {
            let mut r = self.scan.lock().unwrap();
            if let Some(id) = &r.running {
                return Ok(id.clone());
            }
            let id = format!("scan-{}-{}", self.now(), kind_str(kind));
            r.running = Some(id.clone());
            r.kind = Some(kind);
            r.phase = Some("starting".into());
            id
        };
        self.scan_cancel.store(false, Ordering::Relaxed);
        let core = self.clone();
        let sid = id.clone();
        std::thread::Builder::new()
            .name(format!("freeboard-{}", kind_str(kind)))
            .spawn(move || core.run_scan_blocking(&sid, kind))
            .map_err(|e| CommandError::new("internal", e.to_string()))?;
        Ok(id)
    }

    /// Runs a scan on the calling thread (tests and the scheduler thread).
    pub fn run_scan_blocking(&self, scan_id: &str, kind: ScanKind) {
        if kind == ScanKind::Deep {
            crate::platform::lower_thread_priority();
        }
        let settings = self.settings();
        let pause_on_battery = settings.pause_deep_scans_on_battery && kind == ScanKind::Deep;
        let should_pause = move || pause_on_battery && crate::platform::on_battery() == Some(true);
        let sid = scan_id.to_string();
        let progress = |phase: &str, detector: Option<&str>, count: u64| {
            if let Ok(mut r) = self.scan.try_lock() {
                r.phase = Some(phase.to_string());
            }
            self.emit(
                EV_SCAN_PROGRESS,
                &ScanProgress { scan_id: sid.clone(), phase: phase.into(), detector: detector.map(|s| s.into()), current_count: count },
            );
        };
        let ctx = ScanContext {
            runner: &self.runner,
            home: self.home.clone(),
            settings,
            now: self.now(),
            kind,
            cancel: &self.scan_cancel,
            should_pause: &should_pause,
            progress: &progress,
            platform: self.platform,
        };
        let outcome = scan::run_scan(&ctx, &self.detectors);
        let canceled = self.scan_cancel.load(Ordering::Relaxed);
        let mut findings = outcome.findings;
        if kind == ScanKind::Quick {
            // Keep the latest deep-only results; replace everything a quick scan re-checked.
            let ran: Vec<&'static str> = self.detectors.iter().filter(|d| !d.deep_only()).map(|d| d.id()).collect();
            if let Ok(prev) = self.store.findings() {
                findings.extend(prev.into_iter().filter(|f| !ran.iter().any(|id| f.suggestion.detector.starts_with(id))));
            }
            scan::sort_findings(&mut findings);
        }
        let now = self.now();
        if !canceled {
            let _ = self.store.replace_findings(scan_id, &findings);
            let _ = self.store.meta_set("detector_errors", &serde_json::to_string(&outcome.errors).unwrap_or_default());
            let _ = self.store.meta_set("last_scan_at", &now.to_string());
        }
        let mut totals = ScanTotals::default();
        for f in &findings {
            let s = &f.suggestion;
            totals.count += 1;
            totals.bytes += s.size_bytes;
            match s.risk {
                Risk::Safe => totals.safe_bytes += s.size_bytes,
                Risk::Review => totals.review_bytes += s.size_bytes,
                Risk::Risky => totals.risky_bytes += s.size_bytes,
            }
        }
        {
            let mut r = self.scan.lock().unwrap();
            r.running = None;
            r.phase = None;
            r.last_finished_at = Some(now);
            match kind {
                ScanKind::Quick => r.last_quick_at = Some(now),
                ScanKind::Deep => {
                    r.last_deep_at = Some(now);
                    r.last_quick_at = Some(now);
                }
            }
        }
        self.emit(EV_SCAN_FINISHED, &ScanFinished { scan_id: scan_id.into(), detector_errors: outcome.errors, totals });
    }

    pub fn get_suggestions(&self, filter: Option<SuggestionFilter>) -> CmdResult<Vec<Suggestion>> {
        let risk = filter.and_then(|f| f.risk);
        Ok(self
            .store
            .findings()?
            .into_iter()
            .map(|f| f.suggestion)
            .filter(|s| risk.map(|r| r == s.risk).unwrap_or(true))
            .collect())
    }

    pub fn create_plan(&self, item_ids: Vec<String>) -> CmdResult<CleanupPlan> {
        for id in &item_ids {
            validate_id(id)?;
        }
        Ok(self.cleanup.create_plan(&item_ids, self.now())?)
    }

    /// A separately confirmed permanent-delete plan, offered when an item
    /// could not be moved to the Trash.
    pub fn create_permanent_plan(&self, item_ids: Vec<String>) -> CmdResult<CleanupPlan> {
        for id in &item_ids {
            validate_id(id)?;
        }
        Ok(self.cleanup.create_permanent_plan(&item_ids, self.now())?)
    }

    pub fn get_plan(&self, plan_id: &str) -> CmdResult<CleanupPlan> {
        validate_id(plan_id)?;
        Ok(self.cleanup.get_plan(plan_id, self.now())?)
    }

    pub fn acknowledge_item(&self, plan_id: &str, item_id: &str) -> CmdResult<CleanupPlan> {
        validate_id(plan_id)?;
        validate_id(item_id)?;
        Ok(self.cleanup.acknowledge_item(plan_id, item_id, self.now())?)
    }

    pub fn run_plan(&self, plan_id: &str) -> CmdResult<String> {
        validate_id(plan_id)?;
        Ok(self.cleanup.run_plan(plan_id, self.now())?)
    }

    pub fn cancel_run(&self, run_id: &str) -> CmdResult<bool> {
        validate_id(run_id)?;
        Ok(self.cleanup.cancel_run(run_id))
    }

    pub fn get_run(&self, run_id: &str) -> CmdResult<RunDetail> {
        validate_id(run_id)?;
        Ok(self.cleanup.get_run(run_id)?)
    }

    pub fn get_trash(&self) -> CmdResult<TrashSummary> {
        Ok(self.cleanup.trash_summary())
    }

    pub fn empty_freeboard_trash(&self, token: &str) -> CmdResult<EmptyTrashResult> {
        validate_id(token)?;
        let r = self.cleanup.empty_freeboard_trash(token)?;
        let _ = self.store.meta_set("trash_emptied_at", &self.now().to_string());
        let _ = self.tick_monitor();
        Ok(r)
    }

    pub fn list_history(&self, page: u32) -> CmdResult<HistoryPage> {
        Ok(self.cleanup.list_history(page)?)
    }

    pub fn get_settings(&self) -> CmdResult<Settings> {
        Ok(self.settings())
    }

    pub fn update_settings(&self, patch: UiSettingsPatch) -> CmdResult<Settings> {
        let login = patch.start_at_login;
        self.apply_patch(patch.into())?;
        if let Some(l) = login {
            self.shell().set_autostart(l);
        }
        Ok(self.settings())
    }

    fn apply_patch(&self, patch: SettingsPatch) -> CmdResult<Settings> {
        let next = self.settings().apply(patch)?;
        self.store.save_settings(&next)?;
        *self.settings.write().unwrap() = next.clone();
        self.monitor.lock().unwrap().rethreshold(&next);
        Ok(next)
    }

    /// `path` comes from the backend's own native folder picker, never from the interface.
    pub fn add_folder(&self, list: FolderList, path: &Path) -> CmdResult<Settings> {
        let canon = std::fs::canonicalize(path).map_err(|_| CommandError::new("invalid_folder", "That folder is not available."))?;
        if !canon.is_dir() || crate::platform::is_wsl_path(&canon) {
            return Err(CommandError::new("invalid_folder", "Choose a folder on a local drive."));
        }
        let p = canon.to_string_lossy().into_owned();
        let s = self.settings();
        let mut patch = SettingsPatch::default();
        match list {
            FolderList::ScanFolders => {
                let mut v = s.scan_folders.clone();
                if !v.contains(&p) {
                    v.push(p);
                }
                patch.scan_folders = Some(v);
            }
            FolderList::Exclusions => {
                let mut v = s.exclusions.clone();
                if !v.contains(&p) {
                    v.push(p);
                }
                patch.exclusions = Some(v);
            }
        }
        self.apply_patch(patch)
    }

    pub fn remove_folder(&self, list: FolderList, index: u32) -> CmdResult<Settings> {
        let s = self.settings();
        let mut patch = SettingsPatch::default();
        let mut v = match list {
            FolderList::ScanFolders => s.scan_folders.clone(),
            FolderList::Exclusions => s.exclusions.clone(),
        };
        if index as usize >= v.len() {
            return Err(CommandError::new("unknown_id", "No folder at that position."));
        }
        v.remove(index as usize);
        match list {
            FolderList::ScanFolders => patch.scan_folders = Some(v),
            FolderList::Exclusions => patch.exclusions = Some(v),
        }
        self.apply_patch(patch)
    }

    pub fn set_notifications_paused(&self, paused: bool) -> CmdResult<bool> {
        self.notifications_paused.store(paused, Ordering::Relaxed);
        self.store.meta_set("notifications_paused", if paused { "true" } else { "false" })?;
        Ok(paused)
    }

    /// True while a scan or a cleanup run is in progress.
    pub fn is_busy(&self) -> bool {
        self.scan.lock().unwrap().running.is_some() || self.cleanup.is_running()
    }

    pub fn notifications_paused(&self) -> bool {
        self.notifications_paused.load(Ordering::Relaxed)
    }

    // ----- background -----------------------------------------------------

    /// Decide which scheduled work is due. Pure, so it can be tested with a fake clock.
    pub fn due_scan(&self, now: i64) -> Option<ScanKind> {
        let s = self.settings();
        if s.scan_folders.is_empty() && !s.onboarding_complete {
            return None;
        }
        let r = self.scan.lock().unwrap();
        if r.running.is_some() {
            return None;
        }
        if s.deep_scan == DeepScanSchedule::Daily && r.last_deep_at.map(|t| now - t >= 86_400).unwrap_or(true) {
            if !(s.pause_deep_scans_on_battery && crate::platform::on_battery() == Some(true)) {
                return Some(ScanKind::Deep);
            }
        }
        let interval = s.quick_scan.interval_secs()?;
        let last = r.last_quick_at.or_else(|| self.store.meta_get("last_scan_at").and_then(|v| v.parse().ok()));
        if last.map(|t| now - t >= interval as i64).unwrap_or(true) {
            return Some(ScanKind::Quick);
        }
        None
    }

    /// Background loop: monitor every five minutes, scheduled scans, daily
    /// retention. Runs in Rust independent of any webview.
    pub fn spawn_background(self: &Arc<Self>) {
        let core = self.clone();
        std::thread::Builder::new()
            .name("freeboard-monitor".into())
            .spawn(move || {
                let mut last_prune = 0i64;
                loop {
                    let now = core.now();
                    if let Err(e) = core.tick_monitor() {
                        tracing::warn!("monitor tick failed: {}", e.message);
                    }
                    if now - last_prune >= 86_400 {
                        let _ = core.store.prune(now);
                        last_prune = now;
                    }
                    if let Some(kind) = core.due_scan(now) {
                        if let Ok(id) = core.start_scan(kind) {
                            tracing::info!("scheduled {} scan {id}", kind_str(kind));
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_secs(monitor::SAMPLE_INTERVAL_SECS));
                }
            })
            .expect("spawn monitor thread");
    }
}

fn kind_str(k: ScanKind) -> &'static str {
    match k {
        ScanKind::Quick => "quick",
        ScanKind::Deep => "deep",
    }
}

impl QuickScanSchedule {
    pub fn label(self) -> &'static str {
        match self {
            QuickScanSchedule::Hourly => "Every hour",
            QuickScanSchedule::Every6Hours => "Every 6 hours",
            QuickScanSchedule::Daily => "Every day",
            QuickScanSchedule::OnRequest => "Only when I ask",
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::monitor::VolumeReading;
    use std::sync::atomic::AtomicI64;

    pub struct FakeVolumes(pub Arc<AtomicI64>);
    impl VolumeSource for FakeVolumes {
        fn read(&self, _home: &Path, _extra: &[String]) -> Vec<VolumeReading> {
            vec![VolumeReading {
                id: "home".into(),
                name: "Disk".into(),
                mount_point: "/".into(),
                free: self.0.load(Ordering::Relaxed) as u64,
                total: 512 * GB,
            }]
        }
    }

    #[derive(Default)]
    pub struct RecShell(pub Mutex<Vec<String>>);
    impl Shell for RecShell {
        fn notify(&self, title: &str, _body: &str) {
            self.0.lock().unwrap().push(title.to_string());
        }
        fn update_tray(&self, _: DiskStatus, _: u64) {}
        fn set_autostart(&self, _: bool) {}
    }

    pub fn core_with(free_gb: u64, home: &Path) -> (Arc<AppCore>, Arc<AtomicI64>, Arc<AtomicI64>, Arc<MemorySink>) {
        let free = Arc::new(AtomicI64::new((free_gb * GB) as i64));
        let clock_v = Arc::new(AtomicI64::new(1_800_000_000));
        let cv = clock_v.clone();
        let sink = Arc::new(MemorySink::default());
        let tools = home.join(".fake-tools");
        std::fs::create_dir_all(&tools).unwrap();
        let core = AppCore::new(CoreConfig {
            store: Arc::new(Store::open_in_memory().unwrap()),
            runner: Runner::with_path(&tools),
            home: home.to_path_buf(),
            volumes: Arc::new(FakeVolumes(free.clone())),
            events: sink.clone(),
            detectors: Vec::new(),
            clock: Arc::new(move || cv.load(Ordering::Relaxed)),
            platform: crate::platform::platform_name(),
            extra_protected: vec![],
        });
        (core, free, clock_v, sink)
    }

    #[test]
    fn inv03_ids_only_rejects_paths_and_commands() {
        let d = tempfile::tempdir().unwrap();
        let (core, ..) = core_with(50, d.path());
        for bad in ["/etc/passwd", "../../x", "C:\\Windows", "rm -rf /", "docker system prune", "", "a/b", "$(id)", "a;b"] {
            let e = core.create_plan(vec![bad.into()]).unwrap_err();
            assert_eq!(e.code, "invalid_id", "{bad}");
            assert_eq!(core.get_plan(bad).unwrap_err().code, "invalid_id");
            assert_eq!(core.run_plan(bad).unwrap_err().code, "invalid_id");
            assert_eq!(core.acknowledge_item("p1", bad).unwrap_err().code, "invalid_id");
            assert_eq!(core.cancel_run(bad).unwrap_err().code, "invalid_id");
            assert_eq!(core.empty_freeboard_trash(bad).unwrap_err().code, "invalid_id");
        }
        // Well-formed but unknown IDs are rejected by the engine, not executed.
        assert!(core.run_plan("plan-unknown").is_err());
        assert!(core.create_plan(vec!["deadbeef".into()]).is_err());
    }

    #[test]
    fn inv03_settings_patch_cannot_carry_paths_or_commands() {
        for body in [
            r#"{"scanFolders":["/"]}"#,
            r#"{"exclusions":["/"]}"#,
            r#"{"command":"rm"}"#,
            r#"{"extraVolumes":["/"]}"#,
        ] {
            assert!(serde_json::from_str::<UiSettingsPatch>(body).is_err(), "{body}");
        }
        assert!(serde_json::from_str::<SuggestionFilter>(r#"{"path":"/"}"#).is_err());
    }

    #[test]
    fn status_and_notification_through_core() {
        let d = tempfile::tempdir().unwrap();
        let (core, free, clock, sink) = core_with(30, d.path());
        let shell = Arc::new(RecShell::default());
        core.set_shell(shell.clone());
        core.tick_monitor().unwrap();
        assert_eq!(core.get_status().unwrap().status, DiskStatus::Healthy);
        free.store((19 * GB) as i64, Ordering::Relaxed);
        for _ in 0..10 {
            clock.fetch_add(300, Ordering::Relaxed);
            core.tick_monitor().unwrap();
        }
        assert_eq!(core.get_status().unwrap().status, DiskStatus::Low);
        assert_eq!(shell.0.lock().unwrap().as_slice(), ["Disk space is low"]);
        assert_eq!(sink.events(EV_STATUS_CHANGED).len(), 2);
        assert_eq!(core.take_pending_view().as_deref(), Some("suggestions"));
        assert_eq!(core.take_pending_view(), None);
    }

    #[test]
    fn paused_notifications_are_suppressed_and_persisted() {
        let d = tempfile::tempdir().unwrap();
        let (core, free, _clock, _) = core_with(30, d.path());
        let shell = Arc::new(RecShell::default());
        core.set_shell(shell.clone());
        core.set_notifications_paused(true).unwrap();
        free.store((5 * GB) as i64, Ordering::Relaxed);
        core.tick_monitor().unwrap();
        assert!(shell.0.lock().unwrap().is_empty());
        assert!(core.get_status().unwrap().notifications_paused);
        assert_eq!(core.store.meta_get("notifications_paused").as_deref(), Some("true"));
    }

    #[test]
    fn threshold_settings_apply_to_status() {
        let d = tempfile::tempdir().unwrap();
        let (core, ..) = core_with(25, d.path());
        core.tick_monitor().unwrap();
        assert_eq!(core.get_status().unwrap().status, DiskStatus::Healthy);
        core.update_settings(UiSettingsPatch { low_threshold_bytes: Some(30 * GB), ..Default::default() }).unwrap();
        assert_eq!(core.get_status().unwrap().status, DiskStatus::Low);
        let bad = core.update_settings(UiSettingsPatch { critical_threshold_bytes: Some(40 * GB), ..Default::default() });
        assert_eq!(bad.unwrap_err().code, "invalid_settings");
        // Persisted: a new core over the same store sees it.
        assert_eq!(core.store.load_settings().unwrap().low_threshold_bytes, 30 * GB);
    }

    #[test]
    fn folders_change_only_through_backend_paths() {
        let d = tempfile::tempdir().unwrap();
        let (core, ..) = core_with(50, d.path());
        let f = d.path().join("projects");
        std::fs::create_dir_all(&f).unwrap();
        let s = core.add_folder(FolderList::ScanFolders, &f).unwrap();
        assert_eq!(s.scan_folders.len(), 1);
        assert!(core.remove_folder(FolderList::ScanFolders, 5).is_err());
        let s = core.remove_folder(FolderList::ScanFolders, 0).unwrap();
        assert!(s.scan_folders.is_empty());
        assert!(core.add_folder(FolderList::Exclusions, &d.path().join("missing")).is_err());
    }

    #[test]
    fn scheduler_due_scans_follow_settings() {
        let d = tempfile::tempdir().unwrap();
        let (core, ..) = core_with(50, d.path());
        let now = core.now();
        // Before onboarding (no folders chosen) nothing is scheduled.
        assert_eq!(core.due_scan(now), None);
        core.update_settings(UiSettingsPatch { onboarding_complete: Some(true), ..Default::default() }).unwrap();
        assert_eq!(core.due_scan(now), Some(ScanKind::Quick));
        core.run_scan_blocking("s1", ScanKind::Quick);
        assert_eq!(core.due_scan(now + 10), None);
        assert_eq!(core.due_scan(now + 3600), Some(ScanKind::Quick));
        core.update_settings(UiSettingsPatch { quick_scan: Some(QuickScanSchedule::OnRequest), ..Default::default() }).unwrap();
        assert_eq!(core.due_scan(now + 99_999), None);
    }

    #[test]
    fn scan_emits_progress_and_finished() {
        let d = tempfile::tempdir().unwrap();
        let (core, _, _, sink) = core_with(50, d.path());
        core.run_scan_blocking("s1", ScanKind::Quick);
        assert_eq!(sink.events(EV_SCAN_FINISHED).len(), 1);
        assert!(core.get_status().unwrap().scan.last_finished_at.is_some());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_reports_purgeable_and_snapshots_separately() {
        // Uses the real home volume through CoreFoundation, and a fake tmutil.
        let d = tempfile::tempdir().unwrap();
        let tools = d.path().join(".fake-tools");
        std::fs::create_dir_all(&tools).unwrap();
        crate::runner::fake::tool(&tools, "tmutil", "echo 'Snapshots for disk /:'; echo com.apple.TimeMachine.2026-11-04-101010.local");
        let (core, ..) = core_with(50, d.path());
        let mut vols = vec![VolumeStatus {
            id: "home".into(),
            name: "Disk".into(),
            mount_point: "/".into(),
            free_bytes: 1,
            total_bytes: 2,
            status: DiskStatus::Healthy,
            purgeable_bytes: None,
            snapshot_count: None,
        }];
        core.add_platform_details(&mut vols, core.now());
        assert!(vols[0].purgeable_bytes.is_some());
        assert_eq!(vols[0].snapshot_count, Some(1));
    }

    #[test]
    fn state_survives_webview_destruction() {
        // The core has no dependency on a webview: status, settings and
        // findings are served from backend state after the window is gone.
        let d = tempfile::tempdir().unwrap();
        let (core, ..) = core_with(12, d.path());
        core.tick_monitor().unwrap();
        let before = core.get_status().unwrap();
        let again = core.get_status().unwrap();
        assert_eq!(before.status, again.status);
        // Purgeable space is read live from the OS and may move between calls.
        let strip = |v: &[VolumeStatus]| v.iter().map(|v| (v.id.clone(), v.free_bytes, v.total_bytes, v.status)).collect::<Vec<_>>();
        assert_eq!(strip(&before.volumes), strip(&again.volumes));
    }
}
