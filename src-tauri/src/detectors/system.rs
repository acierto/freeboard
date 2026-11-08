//! macOS system items: local Time Machine snapshots older than the newest
//! confirmed backup, and temporary files that apps left behind.

use std::path::{Path, PathBuf};

use super::common::*;
use super::Probe;
use crate::guard::Guard;
use crate::model::*;
use crate::runner::Cmd;
use crate::scan::{Detector, ScanContext};

pub const SNAPSHOTS: &str = "system.snapshots";
pub const STALE_TEMP: &str = "system.stale_temp";
pub const STALE_DAYS: i64 = 14;

// ----- Time Machine local snapshots ------------------------------------------

pub struct Snapshots;

/// "2026-11-04-101010" → Unix seconds (Time Machine writes local time; read
/// as UTC, a few hours either way never turns an older snapshot newer than a
/// backup taken a day later, and the comparison below needs a whole hour).
pub fn parse_tm_date(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() != 17 || !s.chars().enumerate().all(|(i, c)| if [4, 7, 10].contains(&i) { c == '-' } else { c.is_ascii_digit() }) {
        return None;
    }
    let dt = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d-%H%M%S").ok()?;
    Some(dt.and_utc().timestamp())
}

/// Dates of `com.apple.TimeMachine.<date>.local` snapshots.
pub fn parse_snapshots(out: &str) -> Vec<String> {
    let mut v: Vec<String> = out
        .lines()
        .filter_map(|l| l.trim().strip_prefix("com.apple.TimeMachine.")?.strip_suffix(".local").map(String::from))
        .filter(|d| parse_tm_date(d).is_some())
        .collect();
    v.sort();
    v
}

/// The newest backup's date from `tmutil latestbackup`: a path whose last
/// part holds the date (`.../2026-11-07-101010.backup` or `.../2026-11-07-101010`).
/// tmutil prints an error and still exits 0 when there is no destination, so
/// anything else is unconfirmed.
pub fn parse_latest_backup(out: &str) -> Option<i64> {
    let line = out.lines().map(str::trim).find(|l| !l.is_empty())?;
    if !line.starts_with('/') {
        return None;
    }
    let name = Path::new(line).file_name()?.to_string_lossy().to_string();
    parse_tm_date(name.strip_suffix(".backup").unwrap_or(&name))
}

fn latest_backup(ctx: &ScanContext) -> Result<i64, DetectorError> {
    let out = ctx.runner.run_read(&Cmd::new("tmutil", ["latestbackup"])).map_err(|e| run_err(SNAPSHOTS, &e))?;
    parse_latest_backup(&out.stdout)
        .ok_or_else(|| err(SNAPSHOTS, "unavailable", "Freeboard couldn't confirm your newest Time Machine backup, so local snapshots are left alone."))
}

fn local_snapshots(ctx: &ScanContext) -> Result<Vec<String>, DetectorError> {
    let out = ctx.runner.run_read(&Cmd::new("tmutil", ["listlocalsnapshots", "/"])).map_err(|e| run_err(SNAPSHOTS, &e))?;
    if !out.success() {
        return Err(err(SNAPSHOTS, "error", "Time Machine could not list its local snapshots."));
    }
    Ok(parse_snapshots(&out.stdout))
}

/// Snapshot dates are passed to tmutil as one argument.
pub fn safe_date(d: &str) -> bool {
    parse_tm_date(d).is_some()
}

impl Detector for Snapshots {
    fn id(&self) -> &'static str {
        SNAPSHOTS
    }
    fn platforms(&self) -> &'static [&'static str] {
        &["macos"]
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        // Spec: skip this detector unless the newest backup can be confirmed.
        let backup = latest_backup(ctx)?;
        let older: Vec<String> = local_snapshots(ctx)?.into_iter().filter(|d| parse_tm_date(d).is_some_and(|t| t + 3600 < backup)).collect();
        if older.is_empty() {
            return Ok(vec![]);
        }
        // tmutil reports no size per snapshot. Snapshots are what APFS calls
        // purgeable, so that total is shared out, and every size says "about".
        let purgeable = purgeable(ctx);
        let each = purgeable / older.len() as u64;
        let mut parts = vec![];
        for d in &older {
            let mut s = base(SNAPSHOTS, Group::System, d);
            s.title = format!("Snapshot of {}", &d[..10]);
            s.location = "Time Machine local snapshots".into();
            s.size_bytes = each;
            s.full_size_bytes = each;
            s.size_is_estimate = true;
            s.last_used_at = parse_tm_date(d).and_then(rfc3339);
            s.last_used_label = "Older than your newest backup".into();
            s.risk = Risk::Review;
            s.action = Action::NativeCommand;
            s.action_label = format!("tmutil deletelocalsnapshots {d}");
            s.reason = SNAP_REASON.into();
            s.recovery = "Not recoverable, but the newer backup on your backup disk has the same files.".into();
            s.recoverable = false;
            s.fingerprint = fingerprint_of(&[d]);
            parts.push(Finding::new(
                s,
                Target::Native {
                    program: "tmutil".into(),
                    args: vec!["deletelocalsnapshots".into(), d.clone()],
                    resources: vec![d.clone(), format!("backup={backup}")],
                    cwd: None,
                },
            ));
        }
        Ok(snapshot_item(parts).into_iter().collect())
    }
}

const SNAP_REASON: &str = "Local copies Time Machine keeps on this disk. A newer backup on your backup disk already has these files.";

/// APFS purgeable space on the home volume; 0 when unknown or in fixtures.
fn purgeable(ctx: &ScanContext) -> u64 {
    #[cfg(target_os = "macos")]
    if ctx.platform == "macos" && is_real_home(ctx) {
        let free = crate::platform::macos::available_bytes(&ctx.home).unwrap_or(0);
        return crate::platform::macos::purgeable_bytes(&ctx.home, free).unwrap_or(0);
    }
    let _ = ctx;
    0
}

fn is_real_home(ctx: &ScanContext) -> bool {
    let real = crate::platform::home_dir().and_then(|h| std::fs::canonicalize(h).ok());
    real == std::fs::canonicalize(&ctx.home).ok()
}

fn snapshot_item(mut parts: Vec<Finding>) -> Option<Finding> {
    if parts.len() <= 1 {
        return parts.pop();
    }
    parts.reverse();
    let n = parts.len();
    let mut s = base(SNAPSHOTS, Group::System, "local-snapshots");
    s.title = format!("{n} Time Machine local snapshots");
    s.location = "Time Machine local snapshots".into();
    s.size_bytes = parts.iter().map(|f| f.suggestion.size_bytes).sum();
    s.full_size_bytes = s.size_bytes;
    s.size_is_estimate = true;
    s.last_used_at = parts.iter().filter_map(|f| f.suggestion.last_used_at.clone()).max();
    s.last_used_label = "Older than your newest backup".into();
    s.risk = Risk::Review;
    s.action = Action::NativeCommand;
    s.action_label = format!("tmutil deletelocalsnapshots (the {n} snapshots)");
    s.reason = SNAP_REASON.into();
    s.recovery = parts[0].suggestion.recovery.clone();
    s.recoverable = false;
    s.parts = parts
        .iter()
        .map(|f| SuggestionPart {
            id: f.suggestion.id.clone(),
            label: f.suggestion.title.clone(),
            location: f.suggestion.location.clone(),
            size_bytes: f.suggestion.size_bytes,
            selected: true,
        })
        .collect();
    s.fingerprint = parts.iter().map(|f| f.suggestion.fingerprint.as_str()).collect::<Vec<_>>().join("|");
    // Like every split item, the combined target is never run: each part runs
    // on its own with its own date.
    let dates: Vec<String> = parts.iter().filter_map(|f| match &f.target {
        Target::Native { resources, .. } => resources.first().cloned(),
        _ => None,
    }).collect();
    let mut args = vec!["deletelocalsnapshots".to_string()];
    args.extend(dates.iter().cloned());
    Some(Finding { suggestion: s, target: Target::Native { program: "tmutil".into(), args, resources: dates, cwd: None }, parts })
}

// ----- temporary files left by apps ------------------------------------------

pub struct StaleTemp;

/// The person's own temporary folder (`$TMPDIR`, under `/var/folders`). In
/// fixture homes, a folder inside the home.
pub fn user_temp(ctx: &ScanContext) -> PathBuf {
    if is_real_home(ctx) {
        std::env::temp_dir()
    } else {
        ctx.home.join("tmp")
    }
}

/// Every path a running process holds open inside `dir` (`lsof +D`), or None.
/// lsof exits 1 both when nothing is open and, on macOS, when it listed open
/// files (checked live 2026-11-08), so its exit code says nothing; an error
/// on stderr other than a warning makes the answer unknown.
fn open_inside(ctx: &ScanContext, dir: &Path) -> Option<Vec<PathBuf>> {
    let out = ctx.runner.run_read(&Cmd::new("lsof", ["-Fn", "+D", &dir.to_string_lossy()])).ok()?;
    let failed = out.stderr.lines().map(str::trim).any(|l| !l.is_empty() && !l.contains("WARNING"));
    if failed {
        return None;
    }
    Some(out.stdout.lines().filter_map(|l| l.strip_prefix('n')).map(PathBuf::from).collect())
}

/// Top-level entries unchanged for 14 days that no running process holds open,
/// leaving out Apple's own (`com.apple.*`) and anything owned by someone else.
pub fn stale_entries(ctx: &ScanContext, dir: &Path, open: &[PathBuf]) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out = vec![];
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("com.apple.") || name.starts_with('.') {
            continue;
        }
        let Ok(m) = std::fs::symlink_metadata(&p) else { continue };
        if !owned_by_me(&m) {
            continue;
        }
        let Some(st) = path_state(&p, &[]) else { continue };
        let newest = st.m.newest_mtime.unwrap_or(0).max(crate::platform::modified_secs(&m));
        if days_since(ctx.now, newest) < STALE_DAYS || open.iter().any(|o| o.starts_with(&p)) {
            continue;
        }
        out.push(p);
    }
    out.sort();
    out
}

#[cfg(unix)]
fn owned_by_me(m: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: getuid has no preconditions and cannot fail.
    m.uid() == unsafe { libc::getuid() }
}

#[cfg(not(unix))]
fn owned_by_me(_m: &std::fs::Metadata) -> bool {
    true
}

impl Detector for StaleTemp {
    fn id(&self) -> &'static str {
        STALE_TEMP
    }
    fn platforms(&self) -> &'static [&'static str] {
        &["macos"]
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let dir = user_temp(ctx);
        if !dir.is_dir() {
            return Ok(vec![]);
        }
        let Some(open) = open_inside(ctx, &dir) else {
            return Err(err(STALE_TEMP, "unavailable", "Freeboard couldn't check which temporary files apps still use."));
        };
        let paths = stale_entries(ctx, &dir, &open);
        if paths.is_empty() {
            return Ok(vec![]);
        }
        let states: Vec<PathState> = paths.iter().filter_map(|p| path_state(p, &[])).collect();
        let n = states.len();
        let mut s = base(STALE_TEMP, Group::System, "user-temp");
        s.title = "Temporary files apps left behind".into();
        s.location = "Your temporary folder".into();
        s.size_bytes = states.iter().map(|s| s.m.reclaimable()).sum();
        s.full_size_bytes = s.size_bytes;
        s.size_is_estimate = states.iter().any(|s| s.m.is_estimate());
        s.last_used_label = format!("Unchanged for {STALE_DAYS}+ days, not in use");
        s.risk = Risk::Review;
        s.action = Action::Trash;
        s.action_label = move_label(ctx);
        s.reason = format!("{n} temporary item{} that no running app has open and that have not changed for {STALE_DAYS} days.", if n == 1 { "" } else { "s" });
        s.recovery = "Restore them from the Trash.".into();
        s.recoverable = true;
        s.fingerprint = fingerprint_paths(&states);
        if s.size_bytes == 0 {
            return Ok(vec![]);
        }
        Ok(vec![Finding::new(s, Target::Paths { paths: states.iter().map(|s| s.canonical.to_string_lossy().to_string()).collect() })])
    }
}

// ----- pre-cleanup checks ----------------------------------------------------

pub fn probe(f: &Finding, ctx: &ScanContext, guard: &Guard) -> Probe {
    match (&f.target, f.suggestion.detector.as_str()) {
        (Target::Paths { .. }, STALE_TEMP) => {
            let r = super::files::probe_paths(f, ctx, guard);
            if r != Probe::Unchanged {
                return r;
            }
            match open_inside(ctx, &user_temp(ctx)) {
                None => Probe::Unknown("Freeboard couldn't check which temporary files apps still use.".into()),
                Some(open) if f.paths().iter().any(|p| open.iter().any(|o| o.starts_with(p))) => {
                    Probe::Changed("An app is using these files now.".into())
                }
                Some(_) => Probe::Unchanged,
            }
        }
        (Target::Native { resources, .. }, SNAPSHOTS) => {
            let Some(date) = resources.first() else { return Probe::Unknown("Unexpected cleanup target.".into()) };
            let backup = match latest_backup(ctx) {
                Ok(b) => b,
                Err(e) => return Probe::Unknown(e.message),
            };
            match local_snapshots(ctx) {
                Err(e) => Probe::Unknown(e.message),
                Ok(all) if !all.contains(date) => Probe::Changed("This snapshot is gone already.".into()),
                Ok(_) if parse_tm_date(date).is_some_and(|t| t + 3600 < backup) => Probe::Unchanged,
                Ok(_) => Probe::Changed("There is no newer backup than this snapshot any more.".into()),
            }
        }
        _ => Probe::Unknown("Unknown system detector.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tmutil_output() {
        assert_eq!(parse_snapshots("Snapshots for disk /:\ncom.apple.TimeMachine.2026-11-04-101010.local\ncom.apple.TimeMachine.bad.local\n"), ["2026-11-04-101010"]);
        assert!(parse_latest_backup("/Volumes/.timemachine/ABC/2026-11-07-101010.backup/2026-11-07-101010.backup\n").is_some());
        assert!(parse_latest_backup("/Volumes/Backups/Backups.backupdb/Mac/2026-11-07-101010\n").is_some());
        // What tmutil prints, exit code 0, with no destination (checked live).
        assert_eq!(parse_latest_backup("Failed to mount backup destination, error: Error Domain=com.apple.backupd.ErrorDomain Code=17\n"), None);
        assert_eq!(parse_latest_backup(""), None);
        assert!(!safe_date("/") && !safe_date("2026-11-04") && safe_date("2026-11-04-101010"));
    }
}
