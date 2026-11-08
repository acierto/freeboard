//! `apps.unused`: macOS apps not opened for 12 months or more, by
//! Spotlight's last-opened date. Apps without that date are never suggested
//! (spec "Apps: use last-opened data"). One part per app; each goes to the
//! Trash with its own acknowledgment, through the guard's narrow app
//! exception (decision 2026-11-06).
//!
//! Windows and Linux keep no last-opened date Freeboard can read without
//! administrator rights, so the detector runs on macOS only.

use std::path::{Path, PathBuf};

use super::common::*;
use super::Probe;
use crate::guard::Guard;
use crate::model::*;
use crate::runner::Cmd;
use crate::scan::{Detector, ScanContext};

pub const UNUSED: &str = "apps.unused";
pub const UNUSED_DAYS: i64 = 365;

pub struct UnusedApps;

#[derive(Debug, Clone, PartialEq)]
pub struct AppInfo {
    pub bundle_id: Option<String>,
    pub last_used: Option<i64>,
}

/// The app folders: `/Applications` and `~/Applications`. In fixture homes
/// both are inside the home, so tests never read the real `/Applications`.
pub fn app_dirs(ctx: &ScanContext) -> Vec<PathBuf> {
    let real = crate::platform::home_dir().and_then(|h| std::fs::canonicalize(h).ok());
    let system = if real == std::fs::canonicalize(&ctx.home).ok() { PathBuf::from("/Applications") } else { ctx.home.join("SystemApplications") };
    vec![system, ctx.home.join("Applications")]
}

/// `mdls -name kMDItemCFBundleIdentifier -name kMDItemLastUsedDate` output:
/// two lines per file, attributes in that order. None when the count or a
/// line cannot be read.
pub fn parse_mdls(out: &str, files: usize) -> Option<Vec<AppInfo>> {
    let mut vals: Vec<(String, String)> = vec![];
    for l in out.lines().filter(|l| !l.trim().is_empty()) {
        let (k, v) = l.split_once('=')?;
        vals.push((k.trim().to_string(), v.trim().to_string()));
    }
    if vals.len() != files * 2 {
        return None;
    }
    let mut out = vec![];
    for pair in vals.chunks(2) {
        let [(k1, id), (k2, used)] = pair else { return None };
        if k1 != "kMDItemCFBundleIdentifier" || k2 != "kMDItemLastUsedDate" {
            return None;
        }
        let bundle_id = (id != "(null)").then(|| id.trim_matches('"').to_string());
        let last_used = if used == "(null)" { None } else { Some(chrono::DateTime::parse_from_str(used, "%Y-%m-%d %H:%M:%S %z").ok()?.timestamp()) };
        out.push(AppInfo { bundle_id, last_used });
    }
    Some(out)
}

fn mdls(ctx: &ScanContext, apps: &[PathBuf]) -> Option<Vec<AppInfo>> {
    let mut args = vec!["-name".to_string(), "kMDItemCFBundleIdentifier".into(), "-name".into(), "kMDItemLastUsedDate".into()];
    args.extend(apps.iter().map(|a| a.to_string_lossy().to_string()));
    let out = ctx.runner.run_read(&Cmd::new("mdls", args.iter())).ok()?;
    if !out.success() {
        return None;
    }
    parse_mdls(&out.stdout, apps.len())
}

/// Spotlight data for each app: one call for all of them, or one per app when
/// the batch cannot be read (a missing app breaks the batch's order).
pub fn app_info(ctx: &ScanContext, apps: &[PathBuf]) -> Result<Vec<Option<AppInfo>>, DetectorError> {
    if apps.is_empty() {
        return Ok(vec![]);
    }
    if let Some(v) = mdls(ctx, apps) {
        return Ok(v.into_iter().map(Some).collect());
    }
    if !ctx.runner.available("mdls") {
        return Err(err(UNUSED, "unavailable", "Spotlight's mdls tool is not available."));
    }
    Ok(apps.iter().map(|a| mdls(ctx, std::slice::from_ref(a)).and_then(|mut v| v.pop())).collect())
}

/// Top-level app bundles (never links, never inside another bundle).
fn list_apps(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir() && !t.is_symlink()))
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("app")))
        .collect();
    v.sort();
    v
}

fn is_system(id: &Option<String>) -> bool {
    id.as_deref().map_or(true, |i| i.starts_with("com.apple."))
}

fn running(app: &Path, procs: &Option<Vec<String>>) -> Option<bool> {
    let needle = format!("{}/contents/macos/", app.to_string_lossy().to_lowercase());
    Some(procs.as_ref()?.iter().any(|p| p.contains(&needle)))
}

fn app_name(p: &Path) -> String {
    p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

impl Detector for UnusedApps {
    fn id(&self) -> &'static str {
        UNUSED
    }
    fn platforms(&self) -> &'static [&'static str] {
        &["macos"]
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let apps: Vec<PathBuf> = app_dirs(ctx).iter().flat_map(|d| list_apps(d)).collect();
        let info = app_info(ctx, &apps)?;
        let procs = running_commands(ctx);
        let mut entries = vec![];
        for (app, info) in apps.iter().zip(info) {
            // No last-opened date: never suggested.
            let Some(AppInfo { bundle_id, last_used: Some(last) }) = info else { continue };
            if is_system(&bundle_id) || days_since(ctx.now, last) < UNUSED_DAYS || running(app, &procs) != Some(false) {
                continue;
            }
            entries.push((app_name(app), vec![app.clone()], Some((Some(last), format!("Not opened in {}+ days", days_since(ctx.now, last))))));
        }
        let item = FolderItem {
            detector: UNUSED,
            group: Group::OldToolsApps,
            identity: "unused-apps",
            title: "Apps not opened for a year".into(),
            location: "Applications".into(),
            risk: Risk::Review,
            action: Action::Trash,
            reason: "macOS records that you have not opened these apps for 12 months or more. Their settings and documents stay where they are.".into(),
            recovery: "Restore them from the Trash, or install them again.".into(),
        };
        let Some(mut f) = split_finding(ctx, item, entries) else { return Ok(vec![]) };
        // Each app needs its own acknowledgment (decision 2026-11-06).
        f.suggestion.needs_confirmation = true;
        for p in &mut f.parts {
            p.suggestion.needs_confirmation = true;
            p.suggestion.ack_name = Some(p.suggestion.title.clone());
        }
        f.suggestion.ack_name = Some(if f.parts.is_empty() { f.paths().first().map(|p| app_name(Path::new(p))).unwrap_or_default() } else { format!("{} apps", f.parts.len()) });
        Ok(vec![f])
    }
}

/// Before the move: the bundle passes the app exception, is unchanged since
/// the scan, still unused and not running.
pub fn probe(f: &Finding, ctx: &ScanContext, guard: &Guard) -> Probe {
    let Target::Paths { paths } = &f.target else { return Probe::Unknown("Unexpected cleanup target.".into()) };
    // The same two folders the real guard allows; inside fixture homes, theirs.
    let guard = guard.clone().with_app_dirs(&app_dirs(ctx));
    let apps: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let mut states = vec![];
    for a in &apps {
        if let Err(e) = guard.check_unused_app(a) {
            return Probe::Changed(format!("Refused: {e}."));
        }
        let Some(st) = path_state(a, &[]) else { return Probe::Unknown("Freeboard couldn't read it again.".into()) };
        states.push(st);
    }
    if fingerprint_paths(&states).split(';').next() != f.suggestion.fingerprint.split(';').next() {
        return Probe::Changed("The app was replaced or moved after the scan.".into());
    }
    let info = match app_info(ctx, &apps) {
        Ok(i) => i,
        Err(e) => return Probe::Unknown(e.message),
    };
    let procs = running_commands(ctx);
    for (a, i) in apps.iter().zip(info) {
        match i.and_then(|i| i.last_used) {
            Some(t) if days_since(ctx.now, t) >= UNUSED_DAYS => {}
            Some(_) => return Probe::Changed("The app was opened after the scan.".into()),
            None => return Probe::Unknown("macOS no longer reports when this app was last opened.".into()),
        }
        match running(a, &procs) {
            Some(false) => {}
            Some(true) => return Probe::Changed("The app is open. Quit it, then scan again.".into()),
            None => return Probe::Unknown("Freeboard couldn't check whether the app is open.".into()),
        }
    }
    Probe::Unchanged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mdls_output() {
        let out = "kMDItemCFBundleIdentifier = \"com.SweetScape.010Editor\"\nkMDItemLastUsedDate       = (null)\nkMDItemCFBundleIdentifier = \"com.amazon.Lassen\"\nkMDItemLastUsedDate       = 2026-10-17 20:17:56 +0000\n";
        let v = parse_mdls(out, 2).unwrap();
        assert_eq!(v[0], AppInfo { bundle_id: Some("com.SweetScape.010Editor".into()), last_used: None });
        assert_eq!(v[1].last_used, Some(chrono::DateTime::parse_from_rfc3339("2026-10-17T20:17:56Z").unwrap().timestamp()));
        assert_eq!(parse_mdls(out, 3), None);
        assert_eq!(parse_mdls("garbage", 1), None);
    }
}
