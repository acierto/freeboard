//! Package and tool caches: pnpm, pip, npm, Homebrew, Cargo, Gradle, Xcode
//! DerivedData and unavailable iOS simulators.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::common::*;
use super::Probe;
use crate::guard::Guard;
use crate::model::*;
use crate::platform;
use crate::runner::Cmd;
use crate::scan::size::{measure, MeasureOpts};
use crate::scan::{walk, Detector, ScanContext};

pub const PNPM: &str = "cache.pnpm";
pub const PIP: &str = "cache.pip";
pub const NPM: &str = "cache.npm";
pub const HOMEBREW: &str = "cache.homebrew";
pub const CARGO: &str = "cache.cargo";
pub const GRADLE: &str = "cache.gradle";
pub const XCODE: &str = "cache.xcode";
pub const SIMULATORS: &str = "cache.simulators";

/// Growth allowed between scan and cleanup for unbounded cache commands
/// before the item is skipped (5% plus 50 MB).
fn grew(before: u64, now: u64) -> bool {
    now > before + before / 20 + 50_000_000
}

fn read_line(ctx: &ScanContext, det: &str, program: &str, args: &[&str]) -> Result<String, DetectorError> {
    let out = ctx.runner.run_read(&Cmd::new(program, args.iter().copied())).map_err(|e| run_err(det, &e))?;
    if !out.success() {
        return Err(err(det, "unavailable", format!("{program} could not report its cache: {}", out.stderr.lines().next().unwrap_or("").trim())));
    }
    let line = out.stdout.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default().to_string();
    Ok(line)
}

fn abs_dir(det: &str, s: &str) -> Result<PathBuf, DetectorError> {
    let p = PathBuf::from(s);
    if !p.is_absolute() {
        return Err(err(det, "malformed", "The tool reported a cache location Freeboard could not read."));
    }
    Ok(p)
}

fn mtime_label(ctx: &ScanContext, newest: Option<i64>) -> (Option<String>, String) {
    match newest {
        Some(t) if t > 0 => (rfc3339(t), format!("Last changed {}", ago(ctx.now, t))),
        _ => (None, "Last change unknown".into()),
    }
}

// ----- pnpm ------------------------------------------------------------------

pub struct Pnpm;

/// Bytes in the store that no project links to: files whose hard links are
/// all inside the store. Accurate for pnpm's default hard-link import.
fn pnpm_unreferenced(store: &Path) -> (u64, bool) {
    let m = measure(store, &MeasureOpts::default());
    (m.reclaimable(), m.incomplete)
}

impl Detector for Pnpm {
    fn id(&self) -> &'static str {
        PNPM
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let store = abs_dir(PNPM, &read_line(ctx, PNPM, "pnpm", &["store", "path"])?)?;
        if !store.is_dir() {
            return Ok(vec![]);
        }
        let (bytes, _) = pnpm_unreferenced(&store);
        if bytes == 0 {
            return Ok(vec![]);
        }
        let mut s = base(PNPM, Category::Caches, "pnpm-store");
        s.title = "Unreferenced pnpm packages".into();
        s.location = "pnpm content store".into();
        s.size_bytes = bytes;
        s.size_is_estimate = true;
        s.last_used_label = "Not linked to any project".into();
        s.risk = Risk::Safe;
        s.action = Action::NativeCommand;
        s.action_label = "pnpm store prune".into();
        s.reason = "Packages in the store that no project links to.".into();
        s.recovery = "pnpm downloads them when needed.".into();
        s.recoverable = true;
        s.fingerprint = fingerprint_of(&[&store.to_string_lossy()]);
        Ok(vec![Finding {
            suggestion: s,
            target: Target::Native {
                program: "pnpm".into(),
                args: vec!["store".into(), "prune".into()],
                resources: vec![store.to_string_lossy().to_string(), format!("bytes={bytes}")],
                cwd: None,
            },
        }])
    }
}

// ----- pip -------------------------------------------------------------------

pub struct Pip;

fn pip_program(ctx: &ScanContext) -> &'static str {
    if ctx.runner.available("pip3") {
        "pip3"
    } else {
        "pip"
    }
}

impl Detector for Pip {
    fn id(&self) -> &'static str {
        PIP
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let prog = pip_program(ctx);
        let dir = abs_dir(PIP, &read_line(ctx, PIP, prog, &["cache", "dir"])?)?;
        if !dir.is_dir() || platform::is_link(&dir) {
            return Ok(vec![]);
        }
        let m = measure(&dir, &MeasureOpts::default());
        if m.bytes == 0 {
            return Ok(vec![]);
        }
        let (at, label) = mtime_label(ctx, m.newest_mtime);
        let mut s = base(PIP, Category::Caches, "pip-cache");
        s.title = "pip download cache".into();
        s.location = display_path(ctx, &dir);
        s.size_bytes = m.reclaimable();
        s.size_is_estimate = m.is_estimate();
        s.last_used_at = at;
        s.last_used_label = label;
        s.risk = Risk::Safe;
        s.action = Action::NativeCommand;
        s.action_label = format!("{prog} cache purge");
        s.reason = "Downloaded wheels that pip can fetch again.".into();
        s.recovery = "pip downloads them when needed.".into();
        s.recoverable = true;
        s.fingerprint = fingerprint_of(&[&dir.to_string_lossy()]);
        Ok(vec![Finding {
            suggestion: s,
            target: Target::Native {
                program: prog.into(),
                args: vec!["cache".into(), "purge".into()],
                resources: vec![dir.to_string_lossy().to_string(), format!("bytes={}", m.bytes)],
                cwd: None,
            },
        }])
    }
}

// ----- npm -------------------------------------------------------------------

pub struct Npm;

fn npm_cache(ctx: &ScanContext) -> PathBuf {
    if let Ok(line) = read_line(ctx, NPM, "npm", &["config", "get", "cache"]) {
        let p = PathBuf::from(&line);
        if p.is_absolute() && p.starts_with(&ctx.home) {
            return p.join("_cacache");
        }
    }
    if ctx.platform == "windows" {
        ctx.home.join("AppData").join("Local").join("npm-cache").join("_cacache")
    } else {
        ctx.home.join(".npm").join("_cacache")
    }
}

/// A single cache folder deleted (or trashed) through the guard.
fn folder_finding(
    ctx: &ScanContext,
    det: &str,
    dir: &Path,
    risk: Risk,
    action: Action,
    title: &str,
    reason: &str,
    recovery: &str,
) -> Option<Finding> {
    paths_finding(ctx, det, &[dir.to_path_buf()], dir, risk, action, title, reason, recovery)
}

#[allow(clippy::too_many_arguments)]
fn paths_finding(
    ctx: &ScanContext,
    det: &str,
    dirs: &[PathBuf],
    shown: &Path,
    risk: Risk,
    action: Action,
    title: &str,
    reason: &str,
    recovery: &str,
) -> Option<Finding> {
    let ex = walk::exclusions(ctx);
    let mut states = vec![];
    for d in dirs {
        let st = path_state(d, &ex)?;
        if st.is_link {
            return None;
        }
        states.push(st);
    }
    let bytes: u64 = states.iter().map(|s| s.m.reclaimable()).sum();
    if bytes == 0 {
        return None;
    }
    let newest = states.iter().filter_map(|s| s.m.newest_mtime).max();
    let (at, label) = mtime_label(ctx, newest);
    let mut s = base(det, Category::Caches, &shown.to_string_lossy());
    s.title = title.into();
    s.location = display_path(ctx, shown);
    s.size_bytes = bytes;
    s.size_is_estimate = states.iter().any(|s| s.m.is_estimate());
    s.last_used_at = at;
    s.last_used_label = label;
    s.risk = risk;
    s.action = action;
    s.action_label = match action {
        Action::Trash => move_label(ctx),
        _ if dirs.len() == 1 => "Delete the folder".into(),
        _ => format!("Delete the {} folders", dirs.len()),
    };
    s.reason = reason.into();
    s.recovery = recovery.into();
    s.recoverable = true;
    s.fingerprint = fingerprint_paths(&states);
    Some(Finding { suggestion: s, target: Target::Paths { paths: dirs.iter().map(|d| d.to_string_lossy().to_string()).collect() } })
}

impl Detector for Npm {
    fn id(&self) -> &'static str {
        NPM
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let dir = npm_cache(ctx);
        if !dir.is_dir() {
            return Ok(vec![]);
        }
        // `npm cache clean` needs --force, which is not allowed; the folder is
        // removed through the normal guarded delete instead.
        Ok(folder_finding(
            ctx,
            NPM,
            &dir,
            Risk::Safe,
            Action::Delete,
            "npm content cache",
            "Downloaded packages that npm can fetch again.",
            "npm downloads them when needed.",
        )
        .into_iter()
        .collect())
    }
}

// ----- Homebrew --------------------------------------------------------------

pub struct Homebrew;

/// Parse `brew cleanup --dry-run`: the paths it would remove and the total.
pub fn parse_brew(out: &str) -> (BTreeSet<String>, Option<u64>) {
    let mut paths = BTreeSet::new();
    let mut total = None;
    for l in out.lines() {
        if let Some(rest) = l.strip_prefix("Would remove: ") {
            let p = rest.rsplit_once(" (").map(|(p, _)| p).unwrap_or(rest);
            paths.insert(p.trim().to_string());
        } else if let Some(i) = l.find("would free approximately ") {
            let rest = &l[i + "would free approximately ".len()..];
            total = parse_human(rest.split(" of").next().unwrap_or(""));
        }
    }
    (paths, total)
}

fn brew_dry_run(ctx: &ScanContext) -> Result<(BTreeSet<String>, Option<u64>), DetectorError> {
    let out = ctx.runner.run_read(&Cmd::new("brew", ["cleanup", "--dry-run"])).map_err(|e| run_err(HOMEBREW, &e))?;
    if !out.success() {
        return Err(err(HOMEBREW, "error", "Homebrew could not report what it would clean up."));
    }
    Ok(parse_brew(&out.stdout))
}

impl Detector for Homebrew {
    fn id(&self) -> &'static str {
        HOMEBREW
    }
    fn platforms(&self) -> &'static [&'static str] {
        &["macos", "linux"]
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let (paths, total) = brew_dry_run(ctx)?;
        let Some(bytes) = total.filter(|b| *b > 0) else { return Ok(vec![]) };
        let mut s = base(HOMEBREW, Category::Caches, "homebrew");
        s.title = "Old Homebrew downloads".into();
        s.location = "Homebrew cache".into();
        s.size_bytes = bytes;
        s.size_is_estimate = true;
        s.last_used_label = "Old versions and downloads".into();
        s.risk = Risk::Safe;
        s.action = Action::NativeCommand;
        s.action_label = "brew cleanup".into();
        s.reason = format!("Old versions and downloads of installed packages ({} items).", paths.len());
        s.recovery = "Homebrew downloads them when needed.".into();
        s.recoverable = true;
        let list: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        s.fingerprint = fingerprint_of(&list);
        Ok(vec![Finding {
            suggestion: s,
            target: Target::Native { program: "brew".into(), args: vec!["cleanup".into()], resources: paths.into_iter().collect(), cwd: None },
        }])
    }
}

// ----- Cargo -----------------------------------------------------------------

pub struct Cargo;

pub fn cargo_home(ctx: &ScanContext) -> PathBuf {
    env_override(ctx, "CARGO_HOME").unwrap_or_else(|| ctx.home.join(".cargo"))
}

impl Detector for Cargo {
    fn id(&self) -> &'static str {
        CARGO
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let h = cargo_home(ctx);
        let dirs: Vec<PathBuf> = [["registry", "cache"], ["registry", "src"], ["git", "db"], ["git", "checkouts"]]
            .iter()
            .map(|p| h.join(p[0]).join(p[1]))
            .filter(|p| p.is_dir() && !platform::is_link(p))
            .collect();
        if dirs.is_empty() {
            return Ok(vec![]);
        }
        Ok(paths_finding(
            ctx,
            CARGO,
            &dirs,
            &h,
            Risk::Review,
            Action::Trash,
            "Cargo download caches",
            "Downloaded crates and Git sources that Cargo can fetch again. Offline builds need them.",
            &format!("Restore them from the {}, or let Cargo download them again.", trash_word(ctx)),
        )
        .into_iter()
        .collect())
    }
}

// ----- Gradle ----------------------------------------------------------------

pub struct Gradle;

pub fn gradle_home(ctx: &ScanContext) -> PathBuf {
    env_override(ctx, "GRADLE_USER_HOME").unwrap_or_else(|| ctx.home.join(".gradle"))
}

/// Some(true) when a Gradle daemon runs, None when unknown.
pub fn gradle_daemon_running(ctx: &ScanContext) -> Option<bool> {
    if ctx.platform == "windows" {
        return gradle_daemon_from_logs(&gradle_home(ctx).join("daemon"));
    }
    let out = ctx.runner.run_read(&Cmd::new("ps", ["-A", "-o", "args="])).ok()?;
    if !out.success() {
        return None;
    }
    Some(out.stdout.lines().any(|l| l.contains("GradleDaemon") || l.contains("org.gradle.launcher.daemon")))
}

/// Windows has no `ps` and command lines are not readable without WMI, so
/// the daemons' own logs are used: every Gradle daemon writes
/// `<gradle home>/daemon/<version>/daemon-<pid>.out.log`. A daemon counts as
/// running while a process with that ID exists. A reused ID only blocks the
/// cleanup, never allows it.
pub fn gradle_daemon_from_logs(daemon_dir: &std::path::Path) -> Option<bool> {
    let versions = match std::fs::read_dir(daemon_dir) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(false),
        Err(_) => return None,
    };
    for v in versions {
        let v = v.ok()?;
        if !v.file_type().ok()?.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(v.path()).ok()? {
            let name = f.ok()?.file_name();
            let name = name.to_string_lossy();
            let Some(pid) = name.strip_prefix("daemon-").and_then(|r| r.strip_suffix(".out.log")) else { continue };
            let Ok(pid) = pid.parse::<u32>() else { continue };
            match crate::platform::process_alive(pid) {
                Some(false) => {}
                Some(true) => return Some(true),
                None => return None,
            }
        }
    }
    Some(false)
}

const GRADLE_DAEMON_BLOCK: &str = "A Gradle daemon is running. Stop it with gradle --stop, then scan again.";
const GRADLE_UNKNOWN_BLOCK: &str = "Freeboard couldn't check for a running Gradle daemon, so this is blocked.";

impl Detector for Gradle {
    fn id(&self) -> &'static str {
        GRADLE
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let dir = gradle_home(ctx).join("caches");
        if !dir.is_dir() {
            return Ok(vec![]);
        }
        let mut f = match folder_finding(
            ctx,
            GRADLE,
            &dir,
            Risk::Review,
            Action::Trash,
            "Gradle caches",
            "Dependencies and build caches that Gradle downloads or rebuilds.",
            &format!("Restore them from the {}, or let Gradle download them again.", trash_word(ctx)),
        ) {
            Some(f) => f,
            None => return Ok(vec![]),
        };
        match gradle_daemon_running(ctx) {
            Some(false) => {}
            Some(true) => f.suggestion.blocked_reason = Some(GRADLE_DAEMON_BLOCK.into()),
            None => f.suggestion.blocked_reason = Some(GRADLE_UNKNOWN_BLOCK.into()),
        }
        Ok(vec![f])
    }
}

// ----- Xcode -----------------------------------------------------------------

pub struct Xcode;

impl Detector for Xcode {
    fn id(&self) -> &'static str {
        XCODE
    }
    fn platforms(&self) -> &'static [&'static str] {
        &["macos"]
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let dir = ctx.home.join("Library/Developer/Xcode/DerivedData");
        if !dir.is_dir() {
            return Ok(vec![]);
        }
        Ok(folder_finding(
            ctx,
            XCODE,
            &dir,
            Risk::Safe,
            Action::Delete,
            "Xcode DerivedData",
            "Xcode regenerates this on the next build.",
            "Build the project again.",
        )
        .into_iter()
        .collect())
    }
}

// ----- iOS simulators --------------------------------------------------------

pub struct Simulators;

#[derive(Debug, Clone, PartialEq)]
pub struct Sim {
    pub udid: String,
    pub data_path: Option<PathBuf>,
    pub size: Option<u64>,
}

pub fn is_udid(s: &str) -> bool {
    s.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

pub fn parse_simctl(out: &str) -> Option<Vec<Sim>> {
    let v: Value = serde_json::from_str(out).ok()?;
    let devs = v.get("devices")?.as_object()?;
    let mut sims = vec![];
    for list in devs.values() {
        for d in list.as_array()? {
            if d.get("isAvailable").and_then(|a| a.as_bool()) != Some(false) {
                continue;
            }
            let udid = d.get("udid")?.as_str()?.to_string();
            if !is_udid(&udid) {
                continue;
            }
            let size = match (d.get("dataPathSize").and_then(|x| x.as_u64()), d.get("logPathSize").and_then(|x| x.as_u64())) {
                (None, None) => None,
                (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
            };
            sims.push(Sim { udid, data_path: d.get("dataPath").and_then(|p| p.as_str()).map(PathBuf::from), size });
        }
    }
    sims.sort_by(|a, b| a.udid.cmp(&b.udid));
    Some(sims)
}

fn list_unavailable(ctx: &ScanContext) -> Result<Vec<Sim>, DetectorError> {
    let out = ctx
        .runner
        .run_read(&Cmd::new("xcrun", ["simctl", "list", "devices", "unavailable", "--json"]))
        .map_err(|e| run_err(SIMULATORS, &e))?;
    if !out.success() {
        return Err(err(SIMULATORS, "unavailable", "Xcode's simulator tools are not available."));
    }
    parse_simctl(&out.stdout).ok_or_else(|| err(SIMULATORS, "malformed", "simctl returned output Freeboard could not read."))
}

impl Detector for Simulators {
    fn id(&self) -> &'static str {
        SIMULATORS
    }
    fn platforms(&self) -> &'static [&'static str] {
        &["macos"]
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let sims = list_unavailable(ctx)?;
        if sims.is_empty() {
            return Ok(vec![]);
        }
        let mut bytes = 0;
        let mut estimate = false;
        for s in &sims {
            match (s.size, s.data_path.as_ref().and_then(|p| p.parent())) {
                (_, Some(dev)) if dev.is_dir() => bytes += measure(dev, &MeasureOpts::default()).bytes,
                (Some(n), _) => {
                    bytes += n;
                    estimate = true;
                }
                _ => estimate = true,
            }
        }
        let udids: Vec<String> = sims.iter().map(|s| s.udid.clone()).collect();
        let mut s = base(SIMULATORS, Category::Caches, "unavailable-simulators");
        s.title = "Unavailable iOS simulators".into();
        s.location = format!("{} simulator device{}", udids.len(), if udids.len() == 1 { "" } else { "s" });
        s.size_bytes = bytes;
        s.size_is_estimate = estimate;
        s.last_used_label = "Runtime no longer installed".into();
        s.risk = Risk::Review;
        s.action = Action::NativeCommand;
        s.action_label = "xcrun simctl delete unavailable".into();
        s.reason = "These simulators depend on runtimes that are gone.".into();
        s.recovery = "Create the simulator again in Xcode.".into();
        s.recoverable = true;
        s.fingerprint = fingerprint_of(&udids.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        // Bounded: delete exactly the devices listed, not "unavailable".
        let mut args = vec!["simctl".to_string(), "delete".to_string()];
        args.extend(udids.iter().cloned());
        Ok(vec![Finding { suggestion: s, target: Target::Native { program: "xcrun".into(), args, resources: udids, cwd: None } }])
    }
}

// ----- pre-cleanup checks ----------------------------------------------------

fn bytes_res(res: &[String]) -> u64 {
    res.iter().find_map(|r| r.strip_prefix("bytes=")).and_then(|b| b.parse().ok()).unwrap_or(0)
}

pub fn probe(f: &Finding, ctx: &ScanContext, guard: &Guard) -> Probe {
    let det = f.suggestion.detector.as_str();
    match (&f.target, det) {
        (Target::Paths { .. }, GRADLE) => {
            let r = super::files::probe_paths(f, ctx, guard);
            if r != Probe::Unchanged {
                return r;
            }
            match gradle_daemon_running(ctx) {
                Some(false) => Probe::Unchanged,
                Some(true) => Probe::Changed(GRADLE_DAEMON_BLOCK.into()),
                None => Probe::Unknown(GRADLE_UNKNOWN_BLOCK.into()),
            }
        }
        (Target::Paths { .. }, _) => super::files::probe_paths(f, ctx, guard),
        (Target::Native { resources, .. }, PNPM) => match read_line(ctx, PNPM, "pnpm", &["store", "path"]) {
            Err(e) => Probe::Unknown(e.message),
            Ok(p) if Path::new(&p) != Path::new(&resources[0]) => Probe::Changed("pnpm now uses a different store.".into()),
            Ok(p) => {
                let (now, _) = pnpm_unreferenced(Path::new(&p));
                if grew(bytes_res(resources), now) {
                    Probe::Changed("More packages became unreferenced after the scan. Scan again to include them.".into())
                } else {
                    Probe::Unchanged
                }
            }
        },
        (Target::Native { program, resources, .. }, PIP) => match read_line(ctx, PIP, program, &["cache", "dir"]) {
            Err(e) => Probe::Unknown(e.message),
            Ok(p) if Path::new(&p) != Path::new(&resources[0]) => Probe::Changed("pip now uses a different cache folder.".into()),
            Ok(p) => {
                if grew(bytes_res(resources), measure(Path::new(&p), &MeasureOpts::default()).bytes) {
                    Probe::Changed("The pip cache grew after the scan. Scan again to include it.".into())
                } else {
                    Probe::Unchanged
                }
            }
        },
        (Target::Native { resources, .. }, HOMEBREW) => match brew_dry_run(ctx) {
            Err(e) => Probe::Unknown(e.message),
            Ok((paths, _)) => {
                let before: BTreeSet<String> = resources.iter().cloned().collect();
                if paths.is_subset(&before) {
                    Probe::Unchanged
                } else {
                    Probe::Changed("Homebrew found more to remove after the scan. Scan again to include it.".into())
                }
            }
        },
        (Target::Native { resources, .. }, SIMULATORS) => match list_unavailable(ctx) {
            Err(e) => Probe::Unknown(e.message),
            Ok(sims) => {
                if resources.iter().all(|u| sims.iter().any(|s| &s.udid == u)) {
                    Probe::Unchanged
                } else {
                    Probe::Changed("A simulator in this group is no longer unavailable.".into())
                }
            }
        },
        _ => Probe::Unknown("Unknown cache detector.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_brew_dry_run() {
        let out = "Would remove: /opt/homebrew/Library/Homebrew/vendor/portable-ruby/4.0.6_2 (1,707 files, 34.6MB)\nWould remove: /Users/x/Library/Caches/Homebrew/wget--1.0.tar.gz (1.2MB)\n==> This operation would free approximately 1GB of disk space.\n";
        let (p, t) = parse_brew(out);
        assert_eq!(p.len(), 2);
        assert!(p.contains("/opt/homebrew/Library/Homebrew/vendor/portable-ruby/4.0.6_2"));
        assert_eq!(t, Some(1_000_000_000));
    }

    #[test]
    fn parses_simctl_json() {
        let out = include_str!("../../../tests/fixtures/simctl/unavailable.json");
        let sims = parse_simctl(out).unwrap();
        assert_eq!(sims.len(), 2);
        assert!(sims.iter().all(|s| is_udid(&s.udid)));
        assert_eq!(parse_simctl("{nope"), None);
    }
}
