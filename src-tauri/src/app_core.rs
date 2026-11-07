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
use crate::diskmap;
use crate::guard::Guard;
use crate::model::*;
use crate::monitor::{self, Monitor, VolumeSource};
use crate::runner::Runner;
use crate::scan::{self, Detector, ScanContext};
use crate::selection;
use crate::settings::{DeepScanSchedule, Goal, QuickScanSchedule, Settings, SettingsPatch, UiSettingsPatch};
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

/// Where a plan's items come from (spec `create_plan`: `{ source: "pile" }`
/// or `itemIds`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlanSource {
    /// Everything in the pile (Toss).
    Pile,
    /// The given item IDs (Remove now, Remove all here).
    Items,
}

/// The volume a disk-map command is about. Only the home volume has a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeId {
    Home,
}

/// `resolve_blind_spot` answers with the updated map and blind spots.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlindSpotResult {
    pub map: diskmap::DiskMap,
    pub blind_spots: Vec<diskmap::BlindSpot>,
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
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    snapshots: Mutex<std::collections::HashMap<String, (i64, Option<u32>)>>,
    clock: Clock,
    platform: &'static str,
    /// Folders last offered to the user; the interface picks one by index.
    folder_offers: Mutex<Vec<PathBuf>>,
    /// The last folder removed in Settings: (list, position, path), for Undo.
    removed_folder: Mutex<Option<(FolderList, usize, String)>>,
    /// Disk map: also measure system locations outside the home folder.
    /// Off for cores over fixture homes (tests), on for the real app.
    pub measure_outside_home: bool,
    /// Disk map: size from which a folder becomes a blind spot.
    blind_spot_min: std::sync::atomic::AtomicU64,
}

/// A folder Freeboard offers to scan (shown to the user; chosen by index).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderSuggestion {
    pub label: String,
    pub repos: u32,
}

/// What Settings shows next to a chosen folder: repositories inside it and
/// its size at the last deep scan (`None` until one measured it).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    pub repos: u32,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderDetails {
    /// One entry per `settings.scanFolders`, in order.
    pub scan: Vec<FolderInfo>,
    /// One entry per `settings.exclusions`, in order.
    pub skip: Vec<FolderInfo>,
    /// Offered folders, by index (as `folder_suggestions`).
    pub suggestions: Vec<FolderSuggestion>,
    /// Their sizes at the last deep scan, in the same order.
    pub suggestion_bytes: Vec<Option<u64>>,
    pub last_scan_at: Option<String>,
    /// The folder removed last, while it can still be put back.
    pub removed: Option<RemovedFolder>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedFolder {
    pub list: FolderList,
    pub label: String,
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
    /// Disk map: measure system locations outside home (the real app only).
    pub measure_outside_home: bool,
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
        let core = Arc::new(AppCore {
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
            folder_offers: Mutex::new(Vec::new()),
            removed_folder: Mutex::new(None),
            measure_outside_home: cfg.measure_outside_home,
            blind_spot_min: std::sync::atomic::AtomicU64::new(diskmap::roots::BLIND_SPOT_MIN),
        });
        // After a cleanup, read free space again so the status, the tray and
        // the result show the measured change straight away.
        let weak = Arc::downgrade(&core);
        core.cleanup.set_after_run(Box::new(move || {
            if let Some(c) = weak.upgrade() {
                let _ = c.tick_monitor();
            }
        }));
        core
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
            let _ = self.store.meta_set("notification_pending_view", "home");
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
    /// Ask the main window to show a view the next time it is focused
    /// ("home" after a notification, "pile" from the tray view).
    pub fn set_pending_view(&self, view: &str) {
        let _ = self.store.meta_set("notification_pending_view", view);
    }

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

    /// A scan the person asked for (Scan now, Run deep scan now, Scan this
    /// folder, the tray's Scan Now). It runs on battery too: the person is
    /// waiting for it (found 2026-11-06: a requested deep scan sat "paused
    /// while on battery" for minutes).
    pub fn start_scan(self: &Arc<Self>, kind: ScanKind) -> CmdResult<String> {
        self.start_scan_with(kind, true)
    }

    /// A scheduled scan. Deep scans pause on battery when the setting is on
    /// (spec "Background behavior").
    pub fn start_scheduled_scan(self: &Arc<Self>, kind: ScanKind) -> CmdResult<String> {
        self.start_scan_with(kind, false)
    }

    fn start_scan_with(self: &Arc<Self>, kind: ScanKind, requested: bool) -> CmdResult<String> {
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
            .spawn(move || core.run_scan_inner(&sid, kind, requested))
            .map_err(|e| CommandError::new("internal", e.to_string()))?;
        Ok(id)
    }

    /// Runs a scan on the calling thread (tests and the scheduler thread).
    pub fn run_scan_blocking(&self, scan_id: &str, kind: ScanKind) {
        self.run_scan_inner(scan_id, kind, false)
    }

    fn run_scan_inner(&self, scan_id: &str, kind: ScanKind, requested: bool) {
        if kind == ScanKind::Deep {
            crate::platform::lower_thread_priority();
        }
        let settings = self.settings();
        let pause_on_battery = settings.pause_deep_scans_on_battery && kind == ScanKind::Deep && !requested;
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
        let mut canceled = self.scan_cancel.load(Ordering::Relaxed);
        let mut findings = outcome.findings;
        if kind == ScanKind::Quick {
            // Keep the latest deep-only results; replace everything a quick scan re-checked.
            let ran: Vec<&'static str> = self.detectors.iter().filter(|d| !d.deep_only()).map(|d| d.id()).collect();
            if let Ok(prev) = self.store.findings() {
                findings.extend(prev.into_iter().filter(|f| !ran.iter().any(|id| f.suggestion.detector.starts_with(id))));
            }
            scan::sort_findings(&mut findings);
        }
        // Deep scans also measure the area roots for the disk map (design D4);
        // quick scans reuse the latest measurement and never walk the disk.
        if kind == ScanKind::Deep && !canceled {
            let m = self.measure_areas(&ctx, &findings, scan_id);
            canceled = m.canceled || self.scan_cancel.load(Ordering::Relaxed);
            if !canceled {
                let _ = self.store.meta_set("disk_measurement", &serde_json::to_string(&m).unwrap_or_default());
            }
        }
        let now = self.now();
        if !canceled {
            let _ = self.store.replace_findings(scan_id, &findings);
            let ran: Vec<&'static str> = self
                .detectors
                .iter()
                .filter(|d| d.platforms().contains(&self.platform) && (!d.deep_only() || kind == ScanKind::Deep))
                .map(|d| d.id())
                .collect();
            let places = diskmap::merge_places(self.places(), &ran, &findings, &outcome.errors);
            let _ = self.store.meta_set("places_checked", &serde_json::to_string(&places).unwrap_or_default());
            // Ask me later lasts until the next scan.
            let _ = self.store.clear_later_decisions();
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
        if let Ok(p) = self.get_pile() {
            self.emit(EV_PILE_CHANGED, &p);
        }
        if !canceled {
            self.map_changed(true);
        }
    }

    // ----- disk map (spec "Disk accounting", "Blind spots", "Places checked") ----

    fn measure_areas(&self, ctx: &ScanContext, findings: &[Finding], scan_id: &str) -> diskmap::Measurement {
        let discovery = scan::walk::discover(ctx);
        let (claims, _) = diskmap::claims_of(findings);
        let progress = |n: u64| {
            if let Ok(mut r) = self.scan.try_lock() {
                r.phase = Some("measuring".into());
            }
            self.emit(EV_SCAN_PROGRESS, &ScanProgress { scan_id: scan_id.into(), phase: "measuring".into(), detector: None, current_count: n });
        };
        let env = diskmap::roots::MeasureEnv {
            home: self.home.clone(),
            platform: self.platform,
            scan_roots: scan::walk::roots(ctx),
            exclusions: scan::walk::exclusions(ctx),
            repos: &discovery.repos,
            full_disk_access: self.volumes.full_disk_access(self.platform),
            outside_home: self.measure_outside_home,
            cancel: ctx.cancel,
            progress: &progress,
            now: ctx.now,
            min_blind_spot: self.blind_spot_min.load(Ordering::Relaxed),
        };
        diskmap::roots::measure(&env, &claims)
    }

    /// Fixtures are small: let tests lower the blind-spot size.
    #[cfg(test)]
    pub(crate) fn set_blind_spot_min(&self, bytes: u64) {
        self.blind_spot_min.store(bytes, Ordering::Relaxed);
    }

    fn measurement(&self) -> Option<diskmap::Measurement> {
        self.store.meta_get("disk_measurement").and_then(|v| serde_json::from_str(&v).ok())
    }

    fn spot_decisions(&self) -> std::collections::HashMap<String, diskmap::SpotDecision> {
        self.store
            .spot_decisions()
            .unwrap_or_default()
            .into_iter()
            .map(|(id, (text, at))| {
                let d = serde_json::from_str::<diskmap::SpotDecision>(&text).unwrap_or(diskmap::SpotDecision {
                    decision: text,
                    at,
                    kind: None,
                    title: String::new(),
                    location: String::new(),
                    path: String::new(),
                    bytes: None,
                });
                (id, diskmap::SpotDecision { at, ..d })
            })
            .collect()
    }

    fn places(&self) -> Vec<diskmap::PlaceChecked> {
        self.store.meta_get("places_checked").and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default()
    }

    fn home_volume(&self) -> Option<VolumeStatus> {
        let s = self.settings();
        let r = self.volumes.read(&self.home, &s.extra_volumes).into_iter().next()?;
        let mut v = vec![VolumeStatus {
            id: r.id,
            name: r.name,
            mount_point: r.mount_point,
            free_bytes: r.free,
            total_bytes: r.total,
            status: DiskStatus::Healthy,
            purgeable_bytes: None,
            snapshot_count: None,
        }];
        self.add_platform_details(&mut v, self.now());
        v.pop()
    }

    fn build_map(&self) -> CmdResult<diskmap::DiskMap> {
        let vol = self.home_volume();
        let used = vol.as_ref().map(|v| v.total_bytes.saturating_sub(v.free_bytes)).unwrap_or(0);
        let findings = self.store.findings()?;
        let (claims, native) = diskmap::claims_of(&findings);
        let m = self.measurement();
        let decisions = self.spot_decisions();
        Ok(diskmap::build(&diskmap::BuildInput {
            used_bytes: used,
            platform: self.platform,
            claims: &claims,
            native: &native,
            measurement: m.as_ref(),
            system_volume_bytes: self.volumes.platform_volume_bytes(self.platform),
            decisions: &decisions,
            purgeable_bytes: vol.as_ref().and_then(|v| v.purgeable_bytes),
            snapshot_count: vol.as_ref().and_then(|v| v.snapshot_count),
        }))
    }

    fn spots(&self) -> CmdResult<Vec<diskmap::BlindSpot>> {
        let s = self.settings();
        let m = self.measurement();
        let errors: Vec<DetectorError> = self.store.meta_get("detector_errors").and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
        // Plain form on Windows (C:\..., not \\?\C:\...), as measured paths are.
        let canon = |v: &[String]| -> Vec<PathBuf> { v.iter().map(|p| crate::platform::plain_path(&std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p)))).collect() };
        let findings = self.store.findings()?;
        let decisions = self.spot_decisions();
        let scanning = self.scan.lock().unwrap().running.is_some();
        Ok(diskmap::blind_spots(&diskmap::SpotInput {
            measurement: m.as_ref(),
            errors: &errors,
            decisions: &decisions,
            scan_folders: &canon(&s.scan_folders),
            exclusions: &canon(&s.exclusions),
            full_disk_access: self.volumes.full_disk_access(self.platform),
            scanning,
            now: self.now(),
            last_scan_at: self.store.meta_get("last_scan_at").and_then(|v| v.parse().ok()),
            platform: self.platform,
            home: &self.home,
            findings: &findings,
        }))
    }

    /// Recompute the map, store today's area totals (growth) and tell the interface.
    fn map_changed(&self, snapshot: bool) {
        let Ok(map) = self.build_map() else { return };
        if snapshot {
            let day = diskmap::DaySnapshot {
                used_bytes: map.used_bytes,
                not_explained_bytes: map.not_explained_bytes,
                areas: map.areas.iter().map(|a| (a.id.clone(), a.bytes)).collect(),
            };
            let _ = self.store.save_map_day("home", self.now().div_euclid(86_400), &serde_json::to_string(&day).unwrap_or_default());
        }
        self.emit(EV_DISK_MAP_CHANGED, &map);
        if let Ok(spots) = self.spots() {
            self.emit(EV_BLIND_SPOTS_CHANGED, &spots);
        }
    }

    pub fn get_disk_map(&self, _volume: VolumeId) -> CmdResult<diskmap::DiskMap> {
        self.build_map()
    }

    /// Blind spots, re-checked on every call (the interface asks again when
    /// the window regains focus, so granted access shows up without a scan).
    pub fn get_blind_spots(&self, _volume: VolumeId) -> CmdResult<Vec<diskmap::BlindSpot>> {
        self.spots()
    }

    pub fn get_places_checked(&self) -> CmdResult<Vec<diskmap::PlaceChecked>> {
        Ok(self.places())
    }

    pub fn get_growth(&self, days: u32) -> CmdResult<Vec<diskmap::AreaGrowth>> {
        let days = days.clamp(1, 30);
        let today = self.now().div_euclid(86_400);
        let rows = self.store.map_days("home", today - 30)?;
        let parsed: Vec<(i64, diskmap::DaySnapshot)> = rows.into_iter().filter_map(|(d, j)| serde_json::from_str(&j).ok().map(|s| (d, s))).collect();
        Ok(diskmap::growth(&parsed, days, self.platform))
    }

    /// The copyable disk report. Sizes, folder names and risk levels only.
    pub fn create_disk_report(&self) -> CmdResult<serde_json::Value> {
        let map = self.build_map()?;
        let spots = self.spots()?;
        let vol = self.home_volume();
        let suggestions = self.decided_suggestions()?;
        Ok(diskmap::report(&diskmap::ReportInput {
            total_bytes: vol.as_ref().map(|v| v.total_bytes).unwrap_or(0),
            free_bytes: vol.as_ref().map(|v| v.free_bytes).unwrap_or(0),
            platform: self.platform,
            map: &map,
            spots: &spots,
            suggestions: &suggestions,
        }))
    }

    /// Fix a blind spot (design D5). The interface sends the spot's ID and the
    /// action it was offered; paths come from the backend's own records.
    pub fn resolve_blind_spot(self: &Arc<Self>, id: &str, action: diskmap::BlindSpotAction) -> CmdResult<BlindSpotResult> {
        self.resolve_blind_spots(&[id.to_string()], action)
    }

    /// Resolve several blind spots with one action ("Scan all 8" in the
    /// Why-not-explained card): every folder is added, then one deep scan
    /// runs. "Skip" on a folder outside the scan list marks it as the user's
    /// own files, as Mark as mine does for unknown files.
    pub fn resolve_blind_spots(self: &Arc<Self>, ids: &[String], action: diskmap::BlindSpotAction) -> CmdResult<BlindSpotResult> {
        use diskmap::BlindSpotAction as A;
        use diskmap::BlindSpotKind as K;
        if ids.is_empty() {
            return Err(CommandError::new("invalid_request", "No blind spots given."));
        }
        let open = self.spots()?;
        let m = self.measurement();
        let mut spots = Vec::new();
        for id in ids {
            validate_id(id)?;
            let spot = open
                .iter()
                .find(|s| &s.id == id && s.state != diskmap::SpotState::Done)
                .cloned()
                .ok_or_else(|| CommandError::new("unknown_id", "That blind spot is no longer open."))?;
            let allowed = spot.action == action || (action == A::MarkMine && spot.kind == K::UnscannedFolder);
            if !allowed {
                return Err(CommandError::new("invalid_action", "That is not the action for this blind spot."));
            }
            let path = diskmap::spot_path(m.as_ref(), id).unwrap_or_default();
            if action == A::ScanFolder && path.is_empty() {
                return Err(CommandError::new("unknown_id", "That folder is no longer there."));
            }
            spots.push((spot, path));
        }
        let record = |spot: &diskmap::BlindSpot, path: &str, decision: &str| {
            let d = diskmap::SpotDecision {
                decision: decision.into(),
                at: self.now(),
                kind: Some(spot.kind),
                title: spot.title.clone(),
                location: spot.location.clone(),
                path: path.to_string(),
                bytes: spot.size_bytes,
            };
            self.store.set_spot_decision(&spot.id, &serde_json::to_string(&d).unwrap_or_default(), d.at)
        };
        match action {
            A::ScanFolder => {
                for (spot, path) in &spots {
                    if !self.settings().scan_folders.iter().any(|f| Path::new(f) == Path::new(path)) {
                        self.add_folder(FolderList::ScanFolders, Path::new(path))?;
                    }
                    record(spot, path, "scan")?;
                }
                self.start_scan(ScanKind::Deep)?;
            }
            A::MarkMine => {
                for (spot, path) in &spots {
                    record(spot, path, "mine")?;
                }
            }
            A::OpenSettings => {
                // Opens the pane only; the spot stays open until a scan can
                // really read the place (the result is never simulated).
                self.runner
                    .open_settings(crate::runner::SettingsPane::FullDiskAccess, self.platform)
                    .map_err(|e| CommandError::new("unavailable", format!("Could not open System Settings: {e}")))?;
                for (spot, path) in &spots {
                    record(spot, path, "asked")?;
                }
            }
            A::ScanAgain => {
                for (spot, path) in &spots {
                    record(spot, path, "rescan")?;
                }
                let tool = spots.iter().all(|(s, _)| s.kind == K::ToolUnavailable);
                self.start_scan(if tool { ScanKind::Quick } else { ScanKind::Deep })?;
            }
        }
        self.map_changed(false);
        Ok(BlindSpotResult { map: self.build_map()?, blind_spots: self.spots()? })
    }

    pub fn get_suggestions(&self, filter: Option<SuggestionFilter>) -> CmdResult<Vec<Suggestion>> {
        let risk = filter.and_then(|f| f.risk);
        Ok(self.decided_suggestions()?.into_iter().filter(|s| risk.map(|r| r == s.risk).unwrap_or(true)).collect())
    }

    /// The latest scan's suggestions with the user's Keep / Ask me later
    /// decisions and part choices applied.
    fn decided_suggestions(&self) -> CmdResult<Vec<Suggestion>> {
        let mut items: Vec<Suggestion> = self.store.findings()?.into_iter().map(|f| f.suggestion).collect();
        items.iter_mut().for_each(crate::risk_notes::annotate);
        selection::apply_decisions(&mut items, &self.store.load_decisions()?);
        Ok(items)
    }

    // ----- the pile (spec "Building the pile") -------------------------------

    fn pile_store(&self) -> selection::PileStore {
        self.store.load_pile().unwrap_or_else(|| {
            let s = self.settings();
            selection::PileStore::new(s.goal, s.where_from)
        })
    }

    /// Save the pile, compute it, and tell the interface and the tray.
    fn commit_pile(&self, p: &selection::PileStore) -> CmdResult<selection::PileState> {
        self.store.save_pile(p)?;
        let state = selection::fill(p, &self.decided_suggestions()?);
        self.emit(EV_PILE_CHANGED, &state);
        Ok(state)
    }

    pub fn get_pile(&self) -> CmdResult<selection::PileState> {
        Ok(selection::fill(&self.pile_store(), &self.decided_suggestions()?))
    }

    pub fn set_pile_goal(&self, goal: Goal) -> CmdResult<selection::PileState> {
        if let Goal::Bytes(b) = goal {
            if !Goal::SIZES.contains(&b) {
                return Err(CommandError::new("invalid_goal", "Choose 20, 50 or 100 GB, or All I can."));
            }
        }
        let mut p = self.pile_store();
        selection::set_goal(&mut p, goal);
        self.commit_pile(&p)
    }

    pub fn set_pile_areas(&self, areas: Vec<Group>, none: bool) -> CmdResult<selection::PileState> {
        let mut p = self.pile_store();
        selection::set_areas(&mut p, areas, none);
        self.commit_pile(&p)
    }

    /// IDs must name current suggestions; anything else is rejected.
    fn known_ids(&self, ids: &[String]) -> CmdResult<()> {
        let all = self.store.findings()?;
        for id in ids {
            validate_id(id)?;
            if !all.iter().any(|f| &f.suggestion.id == id) {
                return Err(CommandError::new("unknown_item", "That item is not in the latest scan."));
            }
        }
        Ok(())
    }

    pub fn add_to_pile(&self, ids: Vec<String>) -> CmdResult<selection::PileState> {
        self.known_ids(&ids)?;
        let all = self.store.findings()?;
        for id in &ids {
            let s = &all.iter().find(|f| &f.suggestion.id == id).unwrap().suggestion;
            if s.blocked_reason.is_some() || s.action == Action::None {
                return Err(CommandError::new("blocked", "This item is protected and can't go in the pile."));
            }
        }
        let mut p = self.pile_store();
        selection::add(&mut p, &ids);
        self.commit_pile(&p)
    }

    pub fn take_out_of_pile(&self, ids: Vec<String>) -> CmdResult<selection::PileState> {
        self.known_ids(&ids)?;
        let mut p = self.pile_store();
        selection::take_out(&mut p, &ids, &self.decided_suggestions()?);
        self.commit_pile(&p)
    }

    /// Keep, Ask me later, or clear the decision (Stop keeping).
    pub fn set_item_decision(&self, id: String, decision: Option<ItemDecision>) -> CmdResult<Suggestion> {
        self.known_ids(std::slice::from_ref(&id))?;
        self.store.set_decision(&id, decision)?;
        if decision == Some(ItemDecision::Keep) {
            let mut p = self.pile_store();
            selection::on_keep(&mut p, &id);
            self.commit_pile(&p)?;
        } else {
            self.commit_pile(&self.pile_store())?;
        }
        self.decided_suggestions()?.into_iter().find(|s| s.id == id).ok_or_else(|| CommandError::new("unknown_item", "That item is gone."))
    }

    /// Choose which parts of a split item go (spec "Choose which").
    pub fn set_item_parts(&self, id: String, part_ids: Vec<String>) -> CmdResult<Suggestion> {
        self.known_ids(std::slice::from_ref(&id))?;
        let item = self.store.findings()?.into_iter().find(|f| f.suggestion.id == id).unwrap().suggestion;
        for p in &part_ids {
            validate_id(p)?;
            if !item.parts.iter().any(|x| &x.id == p) {
                return Err(CommandError::new("unknown_part", "That part is not in this item."));
            }
        }
        let off: std::collections::BTreeSet<String> = item.parts.iter().filter(|x| !part_ids.contains(&x.id)).map(|x| x.id.clone()).collect();
        self.store.set_deselected_parts(&id, &off)?;
        self.commit_pile(&self.pile_store())?;
        self.decided_suggestions()?.into_iter().find(|s| s.id == id).ok_or_else(|| CommandError::new("unknown_item", "That item is gone."))
    }

    /// A plan from the pile (Toss) or from item IDs (Remove now, Remove all
    /// here). Split items cover their selected parts.
    pub fn create_plan(&self, source: PlanSource, item_ids: Vec<String>) -> CmdResult<CleanupPlan> {
        for id in &item_ids {
            validate_id(id)?;
        }
        let ids = match source {
            PlanSource::Pile if !item_ids.is_empty() => return Err(CommandError::new("invalid_request", "A plan from the pile takes no item IDs.")),
            PlanSource::Pile => self.get_pile()?.item_ids,
            PlanSource::Items => item_ids,
        };
        Ok(self.cleanup.create_plan(&ids, self.now())?)
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

    /// A run with the free space measured now, so the progress sheet's boat
    /// rises as space comes back.
    pub fn get_run(&self, run_id: &str) -> CmdResult<RunDetail> {
        validate_id(run_id)?;
        let mut r = self.cleanup.get_run(run_id)?;
        r.free_bytes_now = Some((self.cleanup.free_space)());
        Ok(r)
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

    pub fn list_logbook(&self, page: u32) -> CmdResult<HistoryPage> {
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
        // Windows: store C:\... rather than the verbatim \\?\C:\... form.
        let canon = crate::platform::plain_path(&canon);
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

    /// Folders in the home folder that hold Git repositories and are not
    /// chosen yet. The interface shows the labels and sends back an index.
    pub fn folder_suggestions(&self) -> CmdResult<Vec<FolderSuggestion>> {
        let s = self.settings();
        // Chosen folders are stored canonical, so compare canonical paths.
        let home = crate::platform::plain_path(&std::fs::canonicalize(&self.home).unwrap_or_else(|_| self.home.clone()));
        let found = crate::scan::folders::suggest(&home, &s.scan_folders, 3);
        let label = |p: &Path| -> String {
            if self.platform != "windows" {
                for h in [&self.home, &home] {
                    if let Ok(rest) = p.strip_prefix(h) {
                        return format!("~/{}", rest.display());
                    }
                }
            }
            p.display().to_string()
        };
        let out = found.iter().map(|c| FolderSuggestion { label: label(&c.path), repos: c.repos }).collect();
        *self.folder_offers.lock().unwrap() = found.into_iter().map(|c| c.path).collect();
        Ok(out)
    }

    /// Add a folder from the last `folder_suggestions` list.
    pub fn add_suggested_folder(&self, index: u32) -> CmdResult<Settings> {
        let path = self.folder_offers.lock().unwrap().get(index as usize).cloned();
        match path {
            Some(p) => self.add_folder(FolderList::ScanFolders, &p),
            None => Err(CommandError::new("unknown_id", "That folder suggestion is no longer available.")),
        }
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
        let gone = v.remove(index as usize);
        *self.removed_folder.lock().unwrap() = Some((list, index as usize, gone));
        match list {
            FolderList::ScanFolders => patch.scan_folders = Some(v),
            FolderList::Exclusions => patch.exclusions = Some(v),
        }
        self.apply_patch(patch)
    }

    /// Put back the folder removed last, at its old position.
    pub fn undo_remove_folder(&self) -> CmdResult<Settings> {
        let Some((list, at, path)) = self.removed_folder.lock().unwrap().take() else {
            return Err(CommandError::new("unknown_id", "There is nothing to undo."));
        };
        let s = self.settings();
        let mut v = match list {
            FolderList::ScanFolders => s.scan_folders.clone(),
            FolderList::Exclusions => s.exclusions.clone(),
        };
        if !v.contains(&path) {
            v.insert(at.min(v.len()), path);
        }
        let mut patch = SettingsPatch::default();
        match list {
            FolderList::ScanFolders => patch.scan_folders = Some(v),
            FolderList::Exclusions => patch.exclusions = Some(v),
        }
        self.apply_patch(patch)
    }

    /// Settings' folder rows: repositories and last measured size per
    /// folder, sized suggestions and the last scan time. Paths stay here;
    /// the interface matches rows by position.
    pub fn folder_details(&self) -> CmdResult<FolderDetails> {
        let s = self.settings();
        let m = self.measurement();
        let info = |p: &str, repos: bool| FolderInfo {
            repos: if repos { crate::scan::folders::count_repos(Path::new(p)) } else { 0 },
            bytes: m.as_ref().and_then(|m| diskmap::roots::folder_bytes(m, Path::new(p))),
        };
        let suggestions = self.folder_suggestions()?;
        let offers = self.folder_offers.lock().unwrap().clone();
        let suggestion_bytes = offers.iter().map(|p| m.as_ref().and_then(|m| diskmap::roots::folder_bytes(m, p))).collect();
        let removed = self.removed_folder.lock().unwrap().as_ref().map(|(list, _, p)| RemovedFolder { list: *list, label: self.folder_label(Path::new(p)) });
        Ok(FolderDetails {
            scan: s.scan_folders.iter().map(|p| info(p, true)).collect(),
            skip: s.exclusions.iter().map(|p| info(p, false)).collect(),
            suggestions,
            suggestion_bytes,
            last_scan_at: self.store.meta_get("last_scan_at").and_then(|v| v.parse::<i64>().ok()).map(diskmap::iso),
            removed,
        })
    }

    /// `~/...` for a folder in the home folder, as the interface shows paths.
    fn folder_label(&self, p: &Path) -> String {
        if self.platform != "windows" {
            let home = crate::platform::plain_path(&std::fs::canonicalize(&self.home).unwrap_or_else(|_| self.home.clone()));
            for h in [&self.home, &home] {
                if let Ok(rest) = p.strip_prefix(h) {
                    return format!("~/{}", rest.display());
                }
            }
        }
        p.display().to_string()
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
        // No scheduled scan until first-run setup is finished: a scan reads
        // places such as Downloads, and macOS asks for permission when it
        // does. First run must not trigger a prompt (spec "Default
        // settings"). Found 2026-11-06 on a database with folders but
        // without finished setup.
        if !s.onboarding_complete {
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
                        if let Ok(id) = core.start_scheduled_scan(kind) {
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
            measure_outside_home: false,
        });
        (core, free, clock_v, sink)
    }

    #[test]
    fn inv03_ids_only_rejects_paths_and_commands() {
        let d = tempfile::tempdir().unwrap();
        let (core, ..) = core_with(50, d.path());
        for bad in ["/etc/passwd", "../../x", "C:\\Windows", "rm -rf /", "docker system prune", "", "a/b", "$(id)", "a;b"] {
            let e = core.create_plan(PlanSource::Items, vec![bad.into()]).unwrap_err();
            assert_eq!(e.code, "invalid_id", "{bad}");
            assert_eq!(core.create_plan(PlanSource::Pile, vec![bad.into()]).unwrap_err().code, "invalid_id");
            assert_eq!(core.get_plan(bad).unwrap_err().code, "invalid_id");
            assert_eq!(core.run_plan(bad).unwrap_err().code, "invalid_id");
            assert_eq!(core.acknowledge_item("p1", bad).unwrap_err().code, "invalid_id");
            assert_eq!(core.cancel_run(bad).unwrap_err().code, "invalid_id");
            assert_eq!(core.empty_freeboard_trash(bad).unwrap_err().code, "invalid_id");
        }
        // Well-formed but unknown IDs are rejected by the engine, not executed.
        assert!(core.run_plan("plan-unknown").is_err());
        assert!(core.create_plan(PlanSource::Items, vec!["deadbeef".into()]).is_err());
        // A pile plan takes no IDs, and an empty pile makes no plan.
        assert_eq!(core.create_plan(PlanSource::Pile, vec!["deadbeef".into()]).unwrap_err().code, "invalid_request");
        assert_eq!(core.create_plan(PlanSource::Pile, vec![]).unwrap_err().code, "empty");
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
        assert_eq!(core.take_pending_view().as_deref(), Some("home"), "a notification click opens the home view (spec Disk status)");
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
    fn suggested_folders_are_added_by_index_only() {
        let d = tempfile::tempdir().unwrap();
        let (core, ..) = core_with(50, d.path());
        std::fs::create_dir_all(core.home.join("proj/app/.git")).unwrap();
        assert!(core.add_suggested_folder(0).is_err(), "nothing offered yet");
        let offers = core.folder_suggestions().unwrap();
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].repos, 1);
        assert!(offers[0].label.ends_with("proj"));
        let s = core.add_suggested_folder(0).unwrap();
        assert_eq!(s.scan_folders.len(), 1);
        assert!(s.scan_folders[0].ends_with("proj"));
        assert!(core.add_suggested_folder(7).is_err());
        assert!(core.folder_suggestions().unwrap().is_empty(), "chosen folders are not offered again");
    }

    #[test]
    fn folders_change_only_through_backend_paths() {
        let d = tempfile::tempdir().unwrap();
        let (core, ..) = core_with(50, d.path());
        let f = d.path().join("projects");
        std::fs::create_dir_all(&f).unwrap();
        let s = core.add_folder(FolderList::ScanFolders, &f).unwrap();
        assert_eq!(s.scan_folders.len(), 1);
        // Stored and shown in plain form (Windows canonicalizes to \\?\C:\...).
        assert!(!s.scan_folders[0].starts_with(r"\\?\"), "{}", s.scan_folders[0]);
        assert!(core.remove_folder(FolderList::ScanFolders, 5).is_err());
        let s = core.remove_folder(FolderList::ScanFolders, 0).unwrap();
        assert!(s.scan_folders.is_empty());
        assert!(core.add_folder(FolderList::Exclusions, &d.path().join("missing")).is_err());
    }

    #[test]
    fn folder_details_and_undo_follow_settings_by_position() {
        let d = tempfile::tempdir().unwrap();
        let (core, ..) = core_with(50, d.path());
        let a = d.path().join("a");
        let b = d.path().join("b");
        std::fs::create_dir_all(a.join("x/.git")).unwrap();
        std::fs::create_dir_all(a.join("y/.git")).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        core.add_folder(FolderList::ScanFolders, &a).unwrap();
        core.add_folder(FolderList::ScanFolders, &b).unwrap();
        let fd = core.folder_details().unwrap();
        assert_eq!(fd.scan.iter().map(|f| f.repos).collect::<Vec<_>>(), vec![2, 0]);
        assert!(fd.scan.iter().all(|f| f.bytes.is_none()), "no deep scan yet: size unknown");
        assert!(fd.removed.is_none());
        assert!(core.undo_remove_folder().is_err(), "nothing to undo");
        // Remove the first one, then put it back where it was.
        let s = core.remove_folder(FolderList::ScanFolders, 0).unwrap();
        assert_eq!(s.scan_folders.len(), 1);
        let removed = core.folder_details().unwrap().removed.unwrap();
        assert_eq!(removed.list, FolderList::ScanFolders);
        assert!(removed.label.ends_with("/a") || removed.label.ends_with("\\a"), "{}", removed.label);
        let s = core.undo_remove_folder().unwrap();
        assert!(s.scan_folders[0].ends_with("a") && s.scan_folders[1].ends_with("b"), "{:?}", s.scan_folders);
        assert!(core.undo_remove_folder().is_err(), "undo works once");
    }

    #[test]
    fn scheduler_due_scans_follow_settings() {
        let d = tempfile::tempdir().unwrap();
        let (core, ..) = core_with(50, d.path());
        let now = core.now();
        // Before onboarding nothing is scheduled.
        assert_eq!(core.due_scan(now), None);
        // Not even when folders are already chosen: a scan before setup is
        // finished would trigger macOS permission prompts during first run.
        let dir = d.path().join("projects");
        std::fs::create_dir_all(&dir).unwrap();
        core.add_folder(FolderList::ScanFolders, &dir).unwrap();
        assert_eq!(core.due_scan(now), None, "no scan until first-run setup is finished");
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
