//! Application caches: JetBrains caches from removed IDE versions, Slack,
//! Discord and Teams caches, and Playwright browsers no project needs.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::common::*;
use super::Probe;
use crate::guard::Guard;
use crate::model::*;
use crate::scan::{walk, Detector, ScanContext};

pub const JETBRAINS: &str = "cache.jetbrains";
pub const ELECTRON: &str = "cache.electron_apps";
pub const PLAYWRIGHT: &str = "cache.playwright";

/// The per-user cache folder for the platform being scanned.
pub fn cache_root(ctx: &ScanContext) -> PathBuf {
    match ctx.platform {
        "macos" => ctx.home.join("Library/Caches"),
        "windows" => ctx.home.join("AppData").join("Local"),
        _ => env_override(ctx, "XDG_CACHE_HOME").unwrap_or_else(|| ctx.home.join(".cache")),
    }
}

/// The per-user application settings folder (Application Support, Roaming,
/// `~/.config`).
pub fn config_root(ctx: &ScanContext) -> PathBuf {
    match ctx.platform {
        "macos" => ctx.home.join("Library/Application Support"),
        "windows" => ctx.home.join("AppData").join("Roaming"),
        _ => env_override(ctx, "XDG_CONFIG_HOME").unwrap_or_else(|| ctx.home.join(".config")),
    }
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut v: Vec<PathBuf> = rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect();
    v.sort();
    v
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

// ----- JetBrains -------------------------------------------------------------

pub struct JetBrains;

/// "IntelliJIdea2024.2" → ("IntelliJIdea", (2024, 2)). Folders without a
/// year.release version (shared caches, Toolbox) are not IDE versions.
pub fn parse_ide_folder(name: &str) -> Option<(String, (u32, u32))> {
    let i = name.find(|c: char| c.is_ascii_digit())?;
    let (product, ver) = name.split_at(i);
    let (y, r) = ver.split_once('.')?;
    if product.is_empty() || y.len() != 4 || r.is_empty() || !r.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((product.to_string(), (y.parse().ok()?, r.parse().ok()?)))
}

/// Cache folders of IDE versions older than the newest one of the same
/// product. The newest version is judged from both caches and settings, so a
/// version whose settings are newer than any cache still counts.
pub fn old_ide_versions(cache_dirs: &[PathBuf], config_dirs: &[PathBuf]) -> Vec<(String, PathBuf)> {
    let mut newest: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for d in cache_dirs.iter().chain(config_dirs) {
        if let Some((p, v)) = parse_ide_folder(&name_of(d)) {
            let e = newest.entry(p).or_insert(v);
            *e = (*e).max(v);
        }
    }
    cache_dirs
        .iter()
        .filter_map(|d| {
            let name = name_of(d);
            let (p, v) = parse_ide_folder(&name)?;
            (v < newest[&p]).then(|| (name, d.clone()))
        })
        .collect()
}

impl Detector for JetBrains {
    fn id(&self) -> &'static str {
        JETBRAINS
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let caches = cache_root(ctx).join("JetBrains");
        let config = config_root(ctx).join("JetBrains");
        let logs = (ctx.platform == "macos").then(|| ctx.home.join("Library/Logs/JetBrains"));
        let old = old_ide_versions(&subdirs(&caches), &subdirs(&config));
        let entries = old
            .into_iter()
            .map(|(name, dir)| {
                let mut paths = vec![dir];
                if let Some(l) = &logs {
                    let p = l.join(&name);
                    if p.is_dir() {
                        paths.push(p);
                    }
                }
                (name, paths, None)
            })
            .collect();
        let item = FolderItem {
            detector: JETBRAINS,
            group: Group::Caches,
            identity: "jetbrains-old-versions",
            title: "Caches from older JetBrains IDEs".into(),
            location: display_path(ctx, &caches),
            risk: Risk::Safe,
            action: Action::Delete,
            reason: "A newer version of the same IDE is installed. The older version's caches and logs are no longer used.".into(),
            recovery: "The IDE rebuilds its caches if that version is opened again.".into(),
        };
        Ok(split_finding(ctx, item, entries).into_iter().collect())
    }
}

// ----- Slack, Discord and Teams ---------------------------------------------

pub struct ElectronApps;

/// (name, settings folder below the config root, words that identify its
/// running process, lowercased).
fn electron_apps(ctx: &ScanContext) -> Vec<(&'static str, PathBuf, &'static [&'static str])> {
    let c = config_root(ctx);
    let teams = match ctx.platform {
        "linux" => c.join("Microsoft").join("Microsoft Teams"),
        _ => c.join("Microsoft").join("Teams"),
    };
    vec![
        ("Slack", c.join("Slack"), &["slack"][..]),
        ("Discord", c.join("discord"), &["discord"][..]),
        ("Microsoft Teams", teams, &["teams"][..]),
    ]
}

/// The cache folders Chromium-based apps rebuild on their own.
pub const ELECTRON_CACHES: [&str; 4] = ["Cache", "Code Cache", "GPUCache", "Service Worker/CacheStorage"];

/// Some(true) while the app runs, None when that cannot be checked. On
/// Windows, Chromium keeps a `lockfile` in the app's folder while it runs and
/// deletes it on exit; a leftover file after a crash only blocks the cleanup.
fn app_running(ctx: &ScanContext, dir: &Path, words: &[&str], procs: &Option<Vec<String>>) -> Option<bool> {
    if ctx.platform == "windows" {
        return match std::fs::symlink_metadata(dir.join("lockfile")) {
            Ok(_) => Some(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(false),
            Err(_) => None,
        };
    }
    let procs = procs.as_ref()?;
    Some(procs.iter().any(|p| {
        let program = p.split_whitespace().next().unwrap_or("");
        words.iter().any(|w| program.contains(w))
    }))
}

fn running_block(app: &str) -> String {
    format!("{app} is open. Quit it, then scan again.")
}

fn unknown_block(app: &str) -> String {
    format!("Freeboard couldn't check whether {app} is open, so this is blocked.")
}

impl Detector for ElectronApps {
    fn id(&self) -> &'static str {
        ELECTRON
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let procs = running_commands(ctx);
        let mut closed = vec![];
        let mut out = vec![];
        for (app, dir, words) in electron_apps(ctx) {
            if !dir.is_dir() {
                continue;
            }
            let paths: Vec<PathBuf> = ELECTRON_CACHES.iter().map(|c| dir.join(c)).filter(|p| p.is_dir()).collect();
            if paths.is_empty() {
                continue;
            }
            match app_running(ctx, &dir, words, &procs) {
                Some(false) => closed.push((app, dir, paths)),
                state => {
                    // An open app is its own blocked item, so the closed ones
                    // can still go.
                    let item = electron_item(ctx, app, &dir);
                    if let Some(mut f) = split_finding(ctx, item, vec![(app.to_string(), paths, None)]) {
                        f.suggestion.blocked_reason = Some(if state == Some(true) { running_block(app) } else { unknown_block(app) });
                        out.push(f);
                    }
                }
            }
        }
        let item = match closed.as_slice() {
            [(app, dir, _)] => electron_item(ctx, app, dir),
            _ => FolderItem {
                detector: ELECTRON,
                group: Group::Caches,
                identity: "electron-app-caches",
                title: "Slack, Discord and Teams caches".into(),
                location: display_path(ctx, &config_root(ctx)),
                risk: Risk::Safe,
                action: Action::Delete,
                reason: ELECTRON_REASON.into(),
                recovery: ELECTRON_RECOVERY.into(),
            },
        };
        let entries = closed.into_iter().map(|(app, _, paths)| (app.to_string(), paths, None)).collect();
        out.extend(split_finding(ctx, item, entries));
        Ok(out)
    }
}

fn electron_item<'a>(ctx: &ScanContext, app: &str, dir: &Path) -> FolderItem<'a> {
    FolderItem {
        detector: ELECTRON,
        group: Group::Caches,
        identity: match app {
            "Slack" => "electron-slack",
            "Discord" => "electron-discord",
            _ => "electron-teams",
        },
        title: format!("{app} cache"),
        location: display_path(ctx, dir),
        risk: Risk::Safe,
        action: Action::Delete,
        reason: ELECTRON_REASON.into(),
        recovery: ELECTRON_RECOVERY.into(),
    }
}

const ELECTRON_REASON: &str = "Cached pages, scripts and images that the app downloads again. Messages and settings are kept.";
const ELECTRON_RECOVERY: &str = "The app rebuilds its cache the next time it opens.";

// ----- Playwright ------------------------------------------------------------

pub struct Playwright;

pub fn playwright_root(ctx: &ScanContext) -> PathBuf {
    env_override(ctx, "PLAYWRIGHT_BROWSERS_PATH").unwrap_or_else(|| cache_root(ctx).join("ms-playwright"))
}

/// "chromium-1140" → ("chromium", 1140).
pub fn parse_browser_folder(name: &str) -> Option<(String, u32)> {
    let (b, rev) = name.rsplit_once('-')?;
    if b.is_empty() || !b.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        return None;
    }
    Some((b.to_string(), rev.parse().ok()?))
}

/// Folder names a `playwright-core/browsers.json` asks for. None when the file
/// is unreadable, so an unknown need is never treated as no need.
pub fn browsers_needed(json: &str) -> Option<BTreeSet<String>> {
    let v: Value = serde_json::from_str(json).ok()?;
    let mut out = BTreeSet::new();
    for b in v.get("browsers")?.as_array()? {
        let name = b.get("name")?.as_str()?.replace('-', "_");
        let rev = b.get("revision")?.as_str()?;
        out.insert(format!("{name}-{rev}"));
    }
    Some(out)
}

/// `browsers.json` files of every Playwright installed in the projects found.
fn project_needs(ctx: &ScanContext) -> Option<BTreeSet<String>> {
    let d = walk::discover(ctx);
    if d.incomplete || d.canceled {
        return None;
    }
    let mut need = BTreeSet::new();
    for nm in d.generated.iter().filter(|g| g.file_name().is_some_and(|n| n == "node_modules")) {
        let mut files = vec![nm.join("playwright-core/browsers.json")];
        // pnpm keeps packages under .pnpm/<name>@<version>/node_modules.
        for e in subdirs(&nm.join(".pnpm")) {
            if name_of(&e).starts_with("playwright-core@") {
                files.push(e.join("node_modules/playwright-core/browsers.json"));
            }
        }
        for f in files {
            match std::fs::read_to_string(&f) {
                Ok(s) => need.extend(browsers_needed(&s)?),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return None,
            }
        }
    }
    Some(need)
}

/// Browser folders that no project asks for and that are not the newest
/// revision of their browser (kept for a Playwright installed outside the
/// scan folders).
pub fn unneeded_browsers(folders: &[PathBuf], need: &BTreeSet<String>) -> Vec<PathBuf> {
    let mut newest: BTreeMap<String, u32> = BTreeMap::new();
    for f in folders {
        if let Some((b, r)) = parse_browser_folder(&name_of(f)) {
            let e = newest.entry(b).or_insert(r);
            *e = (*e).max(r);
        }
    }
    folders
        .iter()
        .filter(|f| {
            let n = name_of(f);
            matches!(parse_browser_folder(&n), Some((b, r)) if r < newest[&b] && !need.contains(&n))
        })
        .cloned()
        .collect()
}

impl Detector for Playwright {
    fn id(&self) -> &'static str {
        PLAYWRIGHT
    }
    /// Needs the projects a deep scan finds.
    fn deep_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let root = playwright_root(ctx);
        let folders = subdirs(&root);
        if folders.is_empty() {
            return Ok(vec![]);
        }
        let Some(need) = project_needs(ctx) else {
            return Err(err(PLAYWRIGHT, "error", "Freeboard couldn't read which browser versions your projects use."));
        };
        let entries = unneeded_browsers(&folders, &need).into_iter().map(|p| (name_of(&p), vec![p], None)).collect();
        let item = FolderItem {
            detector: PLAYWRIGHT,
            group: Group::Caches,
            identity: "playwright-browsers",
            title: "Playwright browsers no project uses".into(),
            location: display_path(ctx, &root),
            risk: Risk::Safe,
            action: Action::Delete,
            reason: "Older browser versions that none of your projects' Playwright asks for. The newest of each browser is kept.".into(),
            recovery: "Run npx playwright install in a project that needs them.".into(),
        };
        Ok(split_finding(ctx, item, entries).into_iter().collect())
    }
}

// ----- pre-cleanup checks ----------------------------------------------------

/// Path checks for every item; Slack, Discord and Teams must also be closed.
pub fn probe(f: &Finding, ctx: &ScanContext, guard: &Guard) -> Probe {
    let r = super::files::probe_paths(f, ctx, guard);
    if r != Probe::Unchanged || f.suggestion.detector != ELECTRON {
        return r;
    }
    let procs = running_commands(ctx);
    for (app, dir, words) in electron_apps(ctx) {
        if !f.paths().iter().any(|p| Path::new(p).starts_with(&dir)) {
            continue;
        }
        match app_running(ctx, &dir, words, &procs) {
            Some(false) => {}
            Some(true) => return Probe::Changed(running_block(app)),
            None => return Probe::Unknown(unknown_block(app)),
        }
    }
    Probe::Unchanged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ide_folders() {
        assert_eq!(parse_ide_folder("IntelliJIdea2024.2"), Some(("IntelliJIdea".into(), (2024, 2))));
        assert_eq!(parse_ide_folder("PyCharmCE2023.3"), Some(("PyCharmCE".into(), (2023, 3))));
        assert_eq!(parse_ide_folder("Toolbox"), None);
        assert_eq!(parse_ide_folder("2024.1"), None);
        assert_eq!(parse_ide_folder("consentOptions"), None);
    }

    #[test]
    fn browser_folders_and_needs() {
        assert_eq!(parse_browser_folder("chromium-1140"), Some(("chromium".into(), 1140)));
        assert_eq!(parse_browser_folder("chromium_headless_shell-1140"), Some(("chromium_headless_shell".into(), 1140)));
        assert_eq!(parse_browser_folder(".links"), None);
        let j = r#"{"comment":"x","browsers":[{"name":"chromium","revision":"1140","installByDefault":true},{"name":"chromium-headless-shell","revision":"1140"}]}"#;
        let n = browsers_needed(j).unwrap();
        assert!(n.contains("chromium-1140") && n.contains("chromium_headless_shell-1140"));
        assert_eq!(browsers_needed("{nope"), None);
    }
}
