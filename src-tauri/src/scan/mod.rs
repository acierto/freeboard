//! Scan orchestration and size measurement.
//!
//! Interface contract (used by `core.rs`): `ScanContext`, `Detector`,
//! `run_scan`, `default_detectors`, `ScanOutcome`.

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
    /// Grouped by category (source order) and sorted by size, largest first.
    pub findings: Vec<Finding>,
    pub errors: Vec<DetectorError>,
}

/// Run all applicable detectors. One failing detector never stops the others.
pub fn run_scan(ctx: &ScanContext, detectors: &[Box<dyn Detector>]) -> ScanOutcome {
    let mut out = ScanOutcome::default();
    let applicable: Vec<&dyn Detector> = detectors
        .iter()
        .map(|d| d.as_ref())
        .filter(|d| d.platforms().contains(&ctx.platform))
        .filter(|d| !d.deep_only() || ctx.kind == ScanKind::Deep)
        .collect();
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
                        Some(size::with_serial(|| run_one(d, ctx)))
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
        }
    }
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

/// Group by category (source order), largest first within each group.
pub fn sort_findings(f: &mut [Finding]) {
    f.sort_by(|a, b| {
        a.suggestion
            .category
            .rank()
            .cmp(&b.suggestion.category.rank())
            .then(b.suggestion.size_bytes.cmp(&a.suggestion.size_bytes))
            .then(a.suggestion.id.cmp(&b.suggestion.id))
    });
}

/// All detectors in source order.
pub fn default_detectors() -> Vec<Box<dyn Detector>> {
    crate::detectors::all()
}
