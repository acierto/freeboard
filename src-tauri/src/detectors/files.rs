//! `files.installers` (old installers and disk images in Downloads) and
//! `files.windows_temp` (old entries in %TEMP%). Documents and media are
//! never suggested in v1.

use std::path::{Path, PathBuf};

use super::common::*;
use super::Probe;
use crate::guard::Guard;
use crate::model::*;
use crate::platform;
use crate::scan::{walk, Detector, ScanContext};

pub const INSTALLERS: &str = "files.installers";
pub const WINDOWS_TEMP: &str = "files.windows_temp";
pub const INSTALLER_EXTS: [&str; 8] = ["dmg", "pkg", "exe", "msi", "appimage", "deb", "rpm", "iso"];
pub const TEMP_DAYS: i64 = 14;

pub struct Installers;
pub struct WindowsTemp;

pub fn is_installer(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| INSTALLER_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Last time a file was opened or modified. Uses the newest of modification
/// and access time so a recently opened installer is never "old".
fn last_touched(meta: &std::fs::Metadata) -> i64 {
    platform::modified_secs(meta).max(platform::accessed_secs(meta))
}

pub fn downloads(ctx: &ScanContext) -> PathBuf {
    platform::downloads_dir(&ctx.home)
}

pub fn temp_dir(ctx: &ScanContext) -> PathBuf {
    env_override(ctx, "TEMP").unwrap_or_else(|| ctx.home.join("AppData").join("Local").join("Temp"))
}

fn grouped(ctx: &ScanContext, det: &str, dir: &Path, paths: Vec<(PathBuf, i64)>) -> Option<Finding> {
    if paths.is_empty() {
        return None;
    }
    let ex = walk::exclusions(ctx);
    let mut states = vec![];
    for (p, _) in &paths {
        states.push(path_state(p, &ex)?);
    }
    let newest = paths.iter().map(|(_, t)| *t).max()?;
    let n = paths.len();
    let tw = trash_word(ctx);
    // Old %TEMP% entries are system leftovers; installers are the person's files.
    let group = if det == WINDOWS_TEMP { Group::System } else { Group::YourFiles };
    let mut s = base(det, group, &dir.to_string_lossy());
    if det == INSTALLERS {
        s.title = "Installers in Downloads".into();
        s.last_used_label = format!("Not opened in {}+ days", days_since(ctx.now, newest));
        s.reason = format!(
            "{n} installer{} and disk image{} that you have not opened recently.",
            if n == 1 { "" } else { "s" },
            if n == 1 { "" } else { "s" }
        );
    } else {
        s.title = "Old temporary files".into();
        s.last_used_label = format!("Older than {TEMP_DAYS} days");
        s.reason = format!("{n} temporary item{} that have not changed in {TEMP_DAYS} days.", if n == 1 { "" } else { "s" });
    }
    s.location = display_path(ctx, dir);
    s.size_bytes = states.iter().map(|st| st.m.reclaimable()).sum();
    s.size_is_estimate = states.iter().any(|st| st.m.is_estimate());
    s.last_used_at = rfc3339(newest);
    s.risk = Risk::Review;
    s.action = Action::Trash;
    s.action_label = move_label(ctx);
    s.recovery = format!("Restore them from the {tw}.");
    s.recoverable = true;
    s.fingerprint = fingerprint_paths(&states);
    Some(Finding {
        parts: vec![],
        suggestion: s,
        target: Target::Paths { paths: paths.iter().map(|(p, _)| p.to_string_lossy().to_string()).collect() },
    })
}

pub fn scan_installers(ctx: &ScanContext, dir: &Path) -> Vec<(PathBuf, i64)> {
    let limit = ctx.settings.old_installer_days as i64 * DAY;
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(m) = std::fs::symlink_metadata(&p) else { continue };
        // Unknown age or links: not suggested.
        if platform::is_link_meta(&p, &m) || !m.is_file() || !is_installer(&p) {
            continue;
        }
        let t = last_touched(&m);
        if t <= 0 || ctx.now - t < limit {
            continue;
        }
        out.push((p, t));
    }
    out.sort();
    out
}

pub fn scan_temp(ctx: &ScanContext, dir: &Path) -> Vec<(PathBuf, i64)> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(m) = std::fs::symlink_metadata(&p) else { continue };
        if platform::is_link_meta(&p, &m) {
            continue;
        }
        let t = if m.is_dir() {
            match crate::scan::size::measure(&p, &Default::default()) {
                ms if ms.incomplete => continue,
                ms => ms.newest_mtime.unwrap_or(platform::modified_secs(&m)).max(platform::modified_secs(&m)),
            }
        } else {
            last_touched(&m)
        };
        if t <= 0 || ctx.now - t < TEMP_DAYS * DAY {
            continue;
        }
        out.push((p, t));
    }
    out.sort();
    out
}

impl Detector for Installers {
    fn id(&self) -> &'static str {
        INSTALLERS
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let dir = downloads(ctx);
        // macOS asks before an app reads Downloads: only once it is a scan
        // folder (Settings, or the disk map's Scan this folder) or with Full
        // Disk Access.
        if would_prompt(ctx, &dir) {
            return Err(err(INSTALLERS, "permission", "Downloads is read once you add it as a scan folder, so macOS does not ask for access you did not choose to give."));
        }
        if !dir.is_dir() {
            return Ok(vec![]);
        }
        if std::fs::read_dir(&dir).is_err() {
            return Err(err(INSTALLERS, "permission", "Freeboard can't read the Downloads folder."));
        }
        Ok(grouped(ctx, INSTALLERS, &dir, scan_installers(ctx, &dir)).into_iter().collect())
    }
}

impl Detector for WindowsTemp {
    fn id(&self) -> &'static str {
        WINDOWS_TEMP
    }
    fn platforms(&self) -> &'static [&'static str] {
        &["windows"]
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let dir = temp_dir(ctx);
        if !dir.is_dir() {
            return Ok(vec![]);
        }
        Ok(grouped(ctx, WINDOWS_TEMP, &dir, scan_temp(ctx, &dir)).into_iter().collect())
    }
}

/// Generic pre-cleanup path check for path targets: canonical path,
/// protected-path rules, no symlink swap, same fingerprint.
pub fn probe_paths(f: &Finding, ctx: &ScanContext, guard: &Guard) -> Probe {
    let Target::Paths { paths } = &f.target else { return Probe::Unknown("Unexpected cleanup target.".into()) };
    let ex = walk::exclusions(ctx);
    let mut states = vec![];
    for p in paths {
        let path = Path::new(p);
        if std::fs::symlink_metadata(path).is_err() {
            return Probe::Changed(format!("{} no longer exists.", display_path(ctx, path)));
        }
        if let Err(e) = guard.check(path) {
            return Probe::Changed(format!("Refused: {e}."));
        }
        let Some(st) = path_state(path, &ex) else { return Probe::Unknown("Freeboard couldn't read it again.".into()) };
        if st.is_link {
            return Probe::Changed("It was replaced by a link after the scan.".into());
        }
        states.push(st);
    }
    let now = fingerprint_paths(&states);
    let expected = f.suggestion.fingerprint.split(";m:").next().unwrap_or_default();
    let (Some(now_loc), Some(exp_loc)) = (now.split(';').next(), expected.split(';').next()) else {
        return Probe::Unknown("Missing fingerprint.".into());
    };
    if now_loc != exp_loc {
        return Probe::Changed("Its location or identity changed after the scan (for example, a folder was swapped for a link).".into());
    }
    if now != expected && fingerprint_paths_legacy(&states) != expected {
        return Probe::Changed("Files inside it changed after the scan.".into());
    }
    if f.suggestion.detector == INSTALLERS {
        let limit = ctx.settings.old_installer_days as i64 * DAY;
        for p in paths {
            if let Ok(m) = std::fs::symlink_metadata(p) {
                if ctx.now - last_touched(&m) < limit {
                    return Probe::Changed("An installer was opened after the scan.".into());
                }
            }
        }
    }
    Probe::Unchanged
}
