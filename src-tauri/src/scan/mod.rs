//! Scan orchestration and size measurement.
//!
//! Interface contract (used by `core.rs`): `ScanContext`, `Detector`,
//! `run_scan`, `default_detectors`, `ScanOutcome`.

pub mod folders;
pub mod size;
pub mod walk;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use crate::model::{DetectorError, Finding, ScanKind};
use crate::runner::Runner;
use crate::settings::Settings;

/// Everything a detector may use. Detectors only read (invariant 1): they get
/// a runner that enforces the read-only policy and no guard (no mutation API).
pub struct ScanContext<'a> {
    pub runner: &'a Runner,
    pub home: PathBuf,
    pub settings: Settings,
    /// Unix seconds; injected so tests are deterministic.
    pub now: i64,
    pub kind: ScanKind,
    pub cancel: &'a AtomicBool,
    /// Returns true while the scan must pause (deep scan on battery).
    pub should_pause: &'a (dyn Fn() -> bool + Sync),
    /// (phase, detector, current count)
    pub progress: &'a (dyn Fn(&str, Option<&str>, u64) + Sync),
    /// Platform override for tests ("macos" | "linux" | "windows").
    pub platform: &'static str,
    /// macOS Full Disk Access: None asks the system when the real home is
    /// scanned (fixture homes count as granted); tests set it to act as a Mac
    /// without it.
    pub full_disk_access: Option<bool>,
}

pub trait Detector: Send + Sync {
    /// Detector ID, e.g. "docker.build_cache". Grouped detectors may report
    /// several detector IDs in their findings; this is the group name.
    fn id(&self) -> &'static str;
    /// Quick detectors run in both scan kinds; deep-only detectors only in deep scans.
    fn deep_only(&self) -> bool {
        false
    }
    fn platforms(&self) -> &'static [&'static str] {
        &["macos", "linux", "windows"]
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError>;
}

#[derive(Debug, Clone, Default)]
pub struct ScanOutcome {
    /// Grouped (spec group order) and sorted by size, largest first.
    pub findings: Vec<Finding>,
    pub errors: Vec<DetectorError>,
}

type CwdList = std::sync::Arc<Vec<(u32, std::path::PathBuf)>>;
type CwdCache = std::sync::Mutex<std::collections::HashMap<usize, Option<CwdList>>>;
fn cwd_cache() -> &'static CwdCache {
    static C: std::sync::OnceLock<CwdCache> = std::sync::OnceLock::new();
    C.get_or_init(Default::default)
}
fn scan_key(ctx: &ScanContext) -> usize {
    ctx.cancel as *const _ as usize
}

/// Processes running inside `dir`. During a scan, every process's working
/// folder is read once and reused for the rest of that scan, so hundreds of
/// worktrees and projects cost one check. Outside a scan (pre-cleanup checks,
/// which must see the state right before acting: invariant 4) it always
/// reads fresh. None means unknown, and callers raise risk (invariant 11).
pub fn processes_in(ctx: &ScanContext, dir: &std::path::Path) -> Option<Vec<u32>> {
    let key = scan_key(ctx);
    let in_scan = cwd_cache().lock().unwrap_or_else(|e| e.into_inner()).contains_key(&key);
    if !in_scan {
        return crate::platform::processes_in_dir(dir, ctx.runner);
    }
    let canon = std::fs::canonicalize(dir).ok()?;
    let cached = cwd_cache().lock().unwrap_or_else(|e| e.into_inner()).get(&key).cloned().flatten();
    let list = match cached {
        Some(l) => l,
        None => {
            let l: CwdList = std::sync::Arc::new(crate::platform::process_cwds(ctx.runner)?);
            cwd_cache().lock().unwrap_or_else(|e| e.into_inner()).insert(key, Some(l.clone()));
            l
        }
    };
    Some(list.iter().filter(|(_, c)| c.starts_with(&canon)).map(|(p, _)| *p).collect())
}

/// Ends the per-scan process cache when the scan returns, however it returns.
struct ScanCacheGuard(usize);
impl Drop for ScanCacheGuard {
    fn drop(&mut self) {
        cwd_cache().lock().unwrap_or_else(|e| e.into_inner()).remove(&self.0);
    }
}

/// Run all applicable detectors. One failing detector never stops the others.
pub fn run_scan(ctx: &ScanContext, detectors: &[Box<dyn Detector>]) -> ScanOutcome {
    cwd_cache().lock().unwrap_or_else(|e| e.into_inner()).insert(scan_key(ctx), None);
    let _guard = ScanCacheGuard(scan_key(ctx));
    let mut out = ScanOutcome::default();
    let applicable: Vec<&dyn Detector> = detectors
        .iter()
        .map(|d| d.as_ref())
        .filter(|d| d.platforms().contains(&ctx.platform))
        .filter(|d| !d.deep_only() || ctx.kind == ScanKind::Deep)
        .collect();
    // How many places this scan checks, then one event per place done, so
    // the first look can say "23 / 49".
    (ctx.progress)("detectors", None, applicable.len() as u64);
    let done = std::sync::atomic::AtomicU64::new(0);
    let finished = |d: &dyn Detector| {
        let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        (ctx.progress)("detector_done", Some(d.id()), n);
    };
    if ctx.kind == ScanKind::Quick {
        // Quick scans mostly wait on tools (docker, brew, pnpm). Run the
        // detectors concurrently; each still fails on its own. Results are
        // collected in detector order so output stays deterministic.
        let results: Vec<_> = std::thread::scope(|s| {
            let handles: Vec<_> = applicable
                .iter()
                .map(|d| {
                    let d = *d;
                    s.spawn(move || {
                        if ctx.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                            return None;
                        }
                        (ctx.progress)("detector", Some(d.id()), 0);
                        let r = size::with_serial(|| run_one(d, ctx));
                        finished(d);
                        Some(r)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().ok().flatten()).collect()
        });
        for r in results.into_iter().flatten() {
            match r {
                Ok(f) => out.findings.extend(f),
                Err(e) => out.errors.push(e),
            }
        }
    } else {
        // Deep scans run at low priority. Callers run scans on a dedicated thread.
        crate::platform::lower_thread_priority();
        for d in applicable {
            if ctx.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            if !walk::wait_if_paused(ctx) {
                break;
            }
            (ctx.progress)("detector", Some(d.id()), out.findings.len() as u64);
            match run_one(d, ctx) {
                Ok(f) => out.findings.extend(f),
                Err(e) => out.errors.push(e),
            }
            finished(d);
        }
    }
    for f in &mut out.findings {
        // Items without parts: the full size is the size.
        if f.suggestion.full_size_bytes == 0 {
            f.suggestion.full_size_bytes = f.suggestion.size_bytes;
        }
    }
    crate::detectors::postgres::link_worktrees(&mut out.findings);
    sort_findings(&mut out.findings);
    out
}

fn run_one(d: &dyn Detector, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| d.run(ctx))) {
        Ok(r) => r,
        Err(_) => Err(DetectorError {
            detector: d.id().to_string(),
            kind: "error".into(),
            message: "The detector stopped unexpectedly.".into(),
        }),
    }
}

/// Group order (spec groups), largest first within each group.
pub fn sort_findings(f: &mut [Finding]) {
    f.sort_by(|a, b| {
        a.suggestion
            .group
            .rank()
            .cmp(&b.suggestion.group.rank())
            .then(b.suggestion.size_bytes.cmp(&a.suggestion.size_bytes))
            .then(a.suggestion.id.cmp(&b.suggestion.id))
    });
}

/// All detectors in source order.
pub fn default_detectors() -> Vec<Box<dyn Detector>> {
    crate::detectors::all()
}
