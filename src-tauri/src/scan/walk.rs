//! Deep-scan discovery: walks the configured folders once per scan to find
//! repositories and generated folders.
//!
//! Rules: never follow links, junctions or reparse points; never descend into
//! `.git` or generated folders (`node_modules`, `target`, `.venv`, `dist`,
//! `build`, `.next`); skip exclusions and WSL paths; pause while the context
//! asks to (battery); stop on cancel. Only directory entries are read; no file
//! is opened and nothing is written.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use crate::platform;
use crate::scan::ScanContext;

pub const GENERATED: [&str; 6] = ["node_modules", "target", ".venv", "dist", "build", ".next"];

#[derive(Debug, Default, Clone)]
pub struct Discovery {
    /// Directories that contain a `.git` entry (directory or file).
    pub repos: Vec<PathBuf>,
    /// Generated folders found (never descended into).
    pub generated: Vec<PathBuf>,
    pub entries: u64,
    pub incomplete: bool,
    pub canceled: bool,
}

/// Wait while the scan must pause. Returns false when canceled.
pub fn wait_if_paused(ctx: &ScanContext) -> bool {
    while (ctx.should_pause)() {
        if ctx.cancel.load(Ordering::Relaxed) {
            return false;
        }
        (ctx.progress)("paused", None, 0);
        std::thread::sleep(Duration::from_millis(250));
    }
    !ctx.cancel.load(Ordering::Relaxed)
}

/// Canonical scan roots (absolute, existing, not links, not WSL).
pub fn roots(ctx: &ScanContext) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for f in &ctx.settings.scan_folders {
        let p = Path::new(f);
        if !p.is_absolute() || platform::is_wsl_path(p) || platform::is_link(p) {
            continue;
        }
        if let Ok(c) = std::fs::canonicalize(p) {
            if !out.iter().any(|o| c.starts_with(o)) {
                out.retain(|o| !o.starts_with(&c));
                out.push(c);
            }
        }
    }
    out
}

pub fn exclusions(ctx: &ScanContext) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for e in &ctx.settings.exclusions {
        let p = PathBuf::from(e);
        if let Ok(c) = std::fs::canonicalize(&p) {
            v.push(c);
        }
        v.push(p);
    }
    v
}

type CacheSlot = Mutex<Option<(usize, i64, Vec<PathBuf>, Arc<Discovery>)>>;
fn cache() -> &'static CacheSlot {
    static C: OnceLock<CacheSlot> = OnceLock::new();
    C.get_or_init(|| Mutex::new(None))
}

/// Discovery for this scan, computed once and shared by the deep detectors.
pub fn discover(ctx: &ScanContext) -> Arc<Discovery> {
    let key = ctx.cancel as *const _ as usize;
    // The folders walked are part of the key: an address and a time alone
    // can repeat for another scan (tests share a fixed clock and reuse
    // addresses; CI got another test's discovery, 2026-11-07).
    let walked = roots(ctx);
    {
        let c = cache().lock().unwrap_or_else(|e| e.into_inner());
        if let Some((k, now, r, d)) = c.as_ref() {
            if *k == key && *now == ctx.now && *r == walked {
                return d.clone();
            }
        }
    }
    let d = Arc::new(walk(ctx));
    *cache().lock().unwrap_or_else(|e| e.into_inner()) = Some((key, ctx.now, walked, d.clone()));
    d
}

fn walk(ctx: &ScanContext) -> Discovery {
    let ex = exclusions(ctx);
    let mut d = Discovery::default();
    let mut stack: Vec<PathBuf> = roots(ctx);
    while let Some(dir) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => {
                d.incomplete = true;
                continue;
            }
        };
        for e in rd {
            let Ok(e) = e else {
                d.incomplete = true;
                continue;
            };
            d.entries += 1;
            if d.entries % 4096 == 0 {
                (ctx.progress)("discovering", None, d.entries);
                if !wait_if_paused(ctx) {
                    d.canceled = true;
                    return d;
                }
            }
            let name = e.file_name();
            let Ok(ft) = e.file_type() else {
                d.incomplete = true;
                continue;
            };
            if name == ".git" {
                if !ft.is_symlink() {
                    d.repos.push(dir.clone());
                }
                continue;
            }
            if ft.is_symlink() || !ft.is_dir() {
                continue;
            }
            let p = e.path();
            if cfg!(windows) && platform::is_link(&p) {
                continue;
            }
            if ex.iter().any(|x| p.starts_with(x)) {
                continue;
            }
            if name.to_str().is_some_and(|n| GENERATED.contains(&n)) {
                d.generated.push(p);
                continue;
            }
            stack.push(p);
        }
    }
    d.repos.sort();
    d.repos.dedup();
    d.generated.sort();
    d
}
