//! Tool versions: Node versions installed with nvm that no project asks for,
//! and Rust toolchains no project uses. One part per version or toolchain.
//! Both need the projects a deep scan finds.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::common::*;
use super::Probe;
use crate::guard::Guard;
use crate::model::*;
use crate::runner::Cmd;
use crate::scan::{walk, Detector, ScanContext};

pub const NODE_VERSIONS: &str = "tools.node_versions";
pub const RUST_TOOLCHAINS: &str = "tools.rust_toolchains";

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// Project folders a deep scan found: repositories and the folders that hold
/// generated folders, plus the home folder (a `~/.nvmrc`). None while the
/// discovery is incomplete, so an unknown need is never "no need".
fn project_dirs(ctx: &ScanContext) -> Option<Vec<PathBuf>> {
    let d = walk::discover(ctx);
    if d.incomplete || d.canceled {
        return None;
    }
    let mut dirs: BTreeSet<PathBuf> = d.repos.iter().cloned().collect();
    dirs.extend(d.generated.iter().filter_map(|g| g.parent().map(Path::to_path_buf)));
    dirs.insert(ctx.home.clone());
    Some(dirs.into_iter().collect())
}

// ----- Node versions ---------------------------------------------------------

pub struct NodeVersions;

pub type Ver = (u64, u64, u64);

/// "v20.18.1" or "20.18.1" → (20, 18, 1).
pub fn parse_ver(s: &str) -> Option<Ver> {
    let s = s.trim().trim_start_matches('v');
    let mut it = s.split('.');
    let v = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    it.next().is_none().then_some(v)
}

/// LTS code names nvm accepts in `.nvmrc` ("lts/iron").
fn lts_major(name: &str) -> Option<u64> {
    Some(match name {
        "argon" => 4,
        "boron" => 6,
        "carbon" => 8,
        "dubnium" => 10,
        "erbium" => 12,
        "fermium" => 14,
        "gallium" => 16,
        "hydrogen" => 18,
        "iron" => 20,
        "jod" => 22,
        "krypton" => 24,
        _ => return None,
    })
}

/// One comparator of a semver range against a version. None when it cannot
/// be read.
fn comparator(c: &str, v: Ver) -> Option<bool> {
    let c = c.trim();
    let (op, rest) = [">=", "<=", ">", "<", "=", "^", "~"].iter().find_map(|op| c.strip_prefix(op).map(|r| (*op, r))).unwrap_or(("", c));
    let rest = rest.trim().trim_start_matches('v');
    if rest.is_empty() || rest == "*" || rest == "x" {
        return Some(true);
    }
    // Partial versions: "20", "20.1", "20.x", "20.1.x".
    let parts: Vec<&str> = rest.split('.').collect();
    let nums: Vec<u64> = parts.iter().take_while(|p| !matches!(**p, "x" | "X" | "*")).map(|p| p.parse().ok()).collect::<Option<_>>()?;
    if nums.is_empty() || nums.len() > 3 {
        return None;
    }
    let full = (nums[0], *nums.get(1).unwrap_or(&0), *nums.get(2).unwrap_or(&0));
    let prefix = |v: Ver| match nums.len() {
        1 => v.0 == nums[0],
        2 => (v.0, v.1) == (nums[0], nums[1]),
        _ => v == full,
    };
    Some(match op {
        ">=" => v >= full,
        ">" => if nums.len() == 3 { v > full } else { v >= full && !prefix(v) },
        "<=" => v <= full || prefix(v),
        "<" => v < full,
        "^" => v >= full && if full.0 > 0 { v.0 == full.0 } else { (v.0, v.1) == (0, full.1) },
        "~" => v >= full && if nums.len() == 1 { v.0 == full.0 } else { (v.0, v.1) == (full.0, full.1) },
        _ => prefix(v),
    })
}

/// Whether a version request (`.nvmrc`, `.node-version`, `engines.node`,
/// `volta.node`) asks for `v`. None when the request cannot be read, which
/// keeps every version.
pub fn requests(req: &str, v: Ver, newest: Ver) -> Option<bool> {
    let r = req.trim().to_lowercase();
    if let Some(name) = r.strip_prefix("lts/") {
        return match name {
            "*" | "latest" => Some(v.0 % 2 == 0 && v.0 == newest_even(newest)),
            n => lts_major(n).map(|m| v.0 == m),
        };
    }
    if matches!(r.as_str(), "node" | "stable" | "latest" | "current" | "*") {
        return Some(v == newest);
    }
    let mut any = false;
    for alt in r.split("||") {
        // "1.2.3 - 2.3.4" hyphen ranges.
        let alt = alt.trim();
        let ok = if let Some((a, b)) = alt.split_once(" - ") {
            comparator(&format!(">={a}"), v)? && comparator(&format!("<={b}"), v)?
        } else {
            let mut all = true;
            for c in alt.split_whitespace() {
                all &= comparator(c, v)?;
            }
            all
        };
        any |= ok;
    }
    Some(any)
}

fn newest_even(v: Ver) -> u64 {
    if v.0 % 2 == 0 { v.0 } else { v.0.saturating_sub(1) }
}

/// Every Node version request in one project folder.
pub fn node_requests(dir: &Path) -> Vec<String> {
    let mut out = vec![];
    for f in [".nvmrc", ".node-version"] {
        if let Ok(s) = std::fs::read_to_string(dir.join(f)) {
            if let Some(l) = s.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#')) {
                out.push(l.to_string());
            }
        }
    }
    if let Ok(s) = std::fs::read_to_string(dir.join("package.json")) {
        match serde_json::from_str::<Value>(&s) {
            Ok(v) => {
                for p in ["/engines/node", "/volta/node"] {
                    if let Some(r) = v.pointer(p).and_then(|x| x.as_str()) {
                        out.push(r.to_string());
                    }
                }
            }
            // Unreadable: treat as asking for anything.
            Err(_) => out.push("garbage(".into()),
        }
    }
    out
}

pub fn nvm_dir(ctx: &ScanContext) -> PathBuf {
    env_override(ctx, "NVM_DIR").unwrap_or_else(|| ctx.home.join(".nvm"))
}

/// Installed versions nobody asks for: not the default alias, not the newest,
/// and no project's request matches. None when a request cannot be read.
pub fn unneeded_node(installed: &[(Ver, PathBuf)], default: Option<&str>, reqs: &[String]) -> Option<Vec<(Ver, PathBuf)>> {
    let newest = installed.iter().map(|(v, _)| *v).max()?;
    let mut out = vec![];
    for (v, p) in installed {
        if *v == newest {
            continue;
        }
        if let Some(d) = default {
            if requests(d, *v, newest)? {
                continue;
            }
        }
        let mut needed = false;
        for r in reqs {
            needed |= requests(r, *v, newest)?;
        }
        if !needed {
            out.push((*v, p.clone()));
        }
    }
    Some(out)
}

/// Node versions installed with nvm, oldest first.
pub fn installed_node(ctx: &ScanContext) -> Vec<(Ver, PathBuf)> {
    let Ok(rd) = std::fs::read_dir(nvm_dir(ctx).join("versions").join("node")) else { return vec![] };
    let mut v: Vec<(Ver, PathBuf)> = rd.flatten().filter_map(|e| Some((parse_ver(&name_of(&e.path()))?, e.path()))).collect();
    v.sort();
    v
}

impl Detector for NodeVersions {
    fn id(&self) -> &'static str {
        NODE_VERSIONS
    }
    fn deep_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let root = nvm_dir(ctx).join("versions").join("node");
        let installed = installed_node(ctx);
        if installed.len() < 2 {
            return Ok(vec![]);
        }
        let default = std::fs::read_to_string(nvm_dir(ctx).join("alias").join("default")).ok().map(|s| s.trim().to_string());
        let Some(dirs) = project_dirs(ctx) else {
            return Err(err(NODE_VERSIONS, "error", "Freeboard couldn't read which Node versions your projects ask for."));
        };
        let reqs: Vec<String> = dirs.iter().flat_map(|d| node_requests(d)).collect();
        let Some(unneeded) = unneeded_node(&installed, default.as_deref(), &reqs) else {
            return Err(err(NODE_VERSIONS, "malformed", "A project asks for a Node version in a way Freeboard could not read, so every version is kept."));
        };
        let entries = unneeded.into_iter().map(|((a, b, c), p)| (format!("Node {a}.{b}.{c}"), vec![p], None)).collect();
        let tw = trash_word(ctx);
        let item = FolderItem {
            detector: NODE_VERSIONS,
            group: Group::OldToolsApps,
            identity: "nvm-node-versions",
            title: "Node versions no project asks for".into(),
            location: display_path(ctx, &root),
            risk: Risk::Review,
            action: Action::Trash,
            reason: "Installed with nvm, but no project's .nvmrc, .node-version or package.json asks for them, and they are not your default or newest version. Global packages installed in a version go with it.".into(),
            recovery: format!("Restore them from the {tw}, or install the version again with nvm install."),
        };
        Ok(split_finding(ctx, item, entries).into_iter().collect())
    }
}

// ----- Rust toolchains -------------------------------------------------------

pub struct RustToolchains;

/// `rustup toolchain list`: "stable-aarch64-apple-darwin (active, default)".
/// Returns (name, is active or default). None when a line cannot be read.
pub fn parse_toolchains(out: &str) -> Option<Vec<(String, bool)>> {
    let mut v = vec![];
    for l in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if l == "no installed toolchains" {
            continue;
        }
        let (name, flags) = match l.split_once(' ') {
            Some((n, f)) => (n, f),
            None => (l, ""),
        };
        if !safe_toolchain(name) {
            return None;
        }
        v.push((name.to_string(), flags.contains("default") || flags.contains("active") || flags.contains("override")));
    }
    Some(v)
}

/// Toolchain names are passed to rustup as one argument.
pub fn safe_toolchain(n: &str) -> bool {
    !n.is_empty() && !n.starts_with('-') && n.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// The channel a project pins: `rust-toolchain.toml` `[toolchain] channel`
/// or a legacy `rust-toolchain` file.
pub fn rust_request(dir: &Path) -> Option<String> {
    if let Ok(s) = std::fs::read_to_string(dir.join("rust-toolchain.toml")) {
        for l in s.lines() {
            let l = l.trim();
            if let Some(rest) = l.strip_prefix("channel") {
                let v = rest.trim_start().strip_prefix('=')?.trim().trim_matches('"').trim_matches('\'');
                return Some(v.to_string());
            }
        }
        return Some("garbage(".into());
    }
    let s = std::fs::read_to_string(dir.join("rust-toolchain")).ok()?;
    let l = s.lines().map(str::trim).find(|l| !l.is_empty())?;
    // The legacy file may hold TOML too.
    if l.starts_with('[') {
        return rust_request_toml(&s);
    }
    Some(l.to_string())
}

fn rust_request_toml(s: &str) -> Option<String> {
    s.lines().find_map(|l| l.trim().strip_prefix("channel").and_then(|r| r.trim_start().strip_prefix('=')).map(|v| v.trim().trim_matches('"').to_string()))
}

/// Whether a pinned channel names this toolchain ("1.75" → "1.75.0-<host>",
/// "nightly-2024-01-01" → "nightly-2024-01-01-<host>").
pub fn pins(req: &str, toolchain: &str) -> bool {
    let req = req.trim();
    if toolchain == req || toolchain.starts_with(&format!("{req}-")) {
        return true;
    }
    // "1.75" pins the newest 1.75.x.
    let base = toolchain.split('-').next().unwrap_or("");
    req.split('.').count() == 2 && base.starts_with(&format!("{req}."))
}

pub fn rustup_home(ctx: &ScanContext) -> PathBuf {
    env_override(ctx, "RUSTUP_HOME").unwrap_or_else(|| ctx.home.join(".rustup"))
}

fn list_toolchains(ctx: &ScanContext) -> Result<Vec<(String, bool)>, DetectorError> {
    let out = ctx.runner.run_read(&Cmd::new("rustup", ["toolchain", "list"])).map_err(|e| run_err(RUST_TOOLCHAINS, &e))?;
    if !out.success() {
        return Err(err(RUST_TOOLCHAINS, "error", "rustup could not list its toolchains."));
    }
    parse_toolchains(&out.stdout).ok_or_else(|| err(RUST_TOOLCHAINS, "malformed", "rustup returned output Freeboard could not read."))
}

impl Detector for RustToolchains {
    fn id(&self) -> &'static str {
        RUST_TOOLCHAINS
    }
    fn deep_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let all = list_toolchains(ctx)?;
        if all.len() < 2 {
            return Ok(vec![]);
        }
        let Some(dirs) = project_dirs(ctx) else {
            return Err(err(RUST_TOOLCHAINS, "error", "Freeboard couldn't read which Rust toolchains your projects use."));
        };
        let reqs: Vec<String> = dirs.iter().filter_map(|d| rust_request(d)).collect();
        if reqs.iter().any(|r| r.contains('(')) {
            return Err(err(RUST_TOOLCHAINS, "malformed", "A project pins a Rust toolchain in a way Freeboard could not read, so every toolchain is kept."));
        }
        let home = rustup_home(ctx).join("toolchains");
        let mut parts = vec![];
        for (name, in_use) in &all {
            if *in_use || reqs.iter().any(|r| pins(r, name)) {
                continue;
            }
            let dir = home.join(name);
            let Some(st) = path_state(&dir, &[]) else { continue };
            if st.is_link {
                // `rustup toolchain link` points at a folder rustup does not own.
                continue;
            }
            let bytes = st.m.reclaimable();
            let mut s = base(RUST_TOOLCHAINS, Group::OldToolsApps, name);
            s.title = name.clone();
            s.location = display_path(ctx, &dir);
            s.size_bytes = bytes;
            s.full_size_bytes = bytes;
            s.size_is_estimate = st.m.is_estimate();
            let (at, label) = match st.m.newest_mtime {
                Some(t) if t > 0 => (rfc3339(t), format!("Installed or updated {}", ago(ctx.now, t))),
                _ => (None, "Last change unknown".into()),
            };
            s.last_used_at = at;
            s.last_used_label = label;
            s.risk = Risk::Review;
            s.action = Action::NativeCommand;
            s.action_label = format!("rustup toolchain uninstall {name}");
            s.reason = RUST_REASON.into();
            s.recovery = format!("Install it again with rustup toolchain install {name}.");
            s.recoverable = true;
            s.fingerprint = fingerprint_of(&[name, &st.identity]);
            parts.push(Finding::new(
                s,
                Target::Native {
                    program: "rustup".into(),
                    args: vec!["toolchain".into(), "uninstall".into(), name.clone()],
                    resources: vec![name.clone()],
                    cwd: None,
                },
            ));
        }
        Ok(native_toolchains(ctx, parts, &home).into_iter().collect())
    }
}

const RUST_REASON: &str = "Not your default or active toolchain, and no project's rust-toolchain file pins it.";

fn native_toolchains(ctx: &ScanContext, mut parts: Vec<Finding>, home: &Path) -> Option<Finding> {
    if parts.is_empty() {
        return None;
    }
    if parts.len() == 1 {
        let mut f = parts.pop()?;
        f.suggestion.title = format!("Rust toolchain {}", f.suggestion.title);
        return Some(f);
    }
    parts.sort_by_key(|f| std::cmp::Reverse(f.suggestion.size_bytes));
    let n = parts.len();
    let mut s = base(RUST_TOOLCHAINS, Group::OldToolsApps, "rust-toolchains");
    s.title = format!("{n} Rust toolchains no project uses");
    s.location = display_path(ctx, home);
    s.size_bytes = parts.iter().map(|f| f.suggestion.size_bytes).sum();
    s.full_size_bytes = s.size_bytes;
    s.size_is_estimate = parts.iter().any(|f| f.suggestion.size_is_estimate);
    s.last_used_at = parts.iter().filter_map(|f| f.suggestion.last_used_at.clone()).max();
    s.last_used_label = parts[0].suggestion.last_used_label.clone();
    s.risk = Risk::Review;
    s.action = Action::NativeCommand;
    s.action_label = format!("rustup toolchain uninstall (the {n} toolchains)");
    s.reason = RUST_REASON.into();
    s.recovery = "Install them again with rustup toolchain install.".into();
    s.recoverable = true;
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
    let names: Vec<String> = parts.iter().map(|f| f.suggestion.title.clone()).collect();
    let mut args = vec!["toolchain".to_string(), "uninstall".to_string()];
    args.extend(names.iter().cloned());
    Some(Finding { suggestion: s, target: Target::Native { program: "rustup".into(), args, resources: names, cwd: None }, parts })
}

// ----- pre-cleanup checks ----------------------------------------------------

pub fn probe(f: &Finding, ctx: &ScanContext, guard: &Guard) -> Probe {
    match (&f.target, f.suggestion.detector.as_str()) {
        (Target::Paths { .. }, NODE_VERSIONS) => {
            let r = super::files::probe_paths(f, ctx, guard);
            if r != Probe::Unchanged {
                return r;
            }
            // The default (or the newest) may be this version since the scan.
            let Some(v) = f.paths().first().and_then(|p| parse_ver(&name_of(Path::new(p)))) else {
                return Probe::Unknown("Unexpected cleanup target.".into());
            };
            let newest = installed_node(ctx).into_iter().map(|(v, _)| v).max().unwrap_or(v);
            let default = std::fs::read_to_string(nvm_dir(ctx).join("alias").join("default")).unwrap_or_default();
            let default = default.trim();
            if v >= newest {
                return Probe::Changed("This is your newest Node version now.".into());
            }
            match if default.is_empty() { Some(false) } else { requests(default, v, newest) } {
                Some(false) => Probe::Unchanged,
                Some(true) => Probe::Changed("This is your default Node version now.".into()),
                None => Probe::Unknown("Freeboard couldn't read your default Node version.".into()),
            }
        }
        (Target::Native { resources, .. }, RUST_TOOLCHAINS) => {
            let [name] = resources.as_slice() else { return Probe::Unknown("Unexpected cleanup target.".into()) };
            match list_toolchains(ctx) {
                Err(e) => Probe::Unknown(e.message),
                Ok(all) => match all.iter().find(|(n, _)| n == name) {
                    None => Probe::Changed("This toolchain is gone already.".into()),
                    Some((_, true)) => Probe::Changed("This toolchain is your default or active one now.".into()),
                    Some(_) => Probe::Unchanged,
                },
            }
        }
        _ => Probe::Unknown("Unknown tool detector.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: Ver = (24, 18, 0);

    #[test]
    fn node_requests_match_versions() {
        let v20 = (20, 18, 1);
        for (r, want) in [
            ("20", true),
            ("v20.18.1", true),
            ("20.18", true),
            ("22", false),
            (">=18", true),
            (">=21", false),
            ("^20.0.0", true),
            ("~20.17", false),
            ("18.x || 20.x", true),
            (">=18 <20", false),
            ("16 - 20", true),
            ("lts/iron", true),
            ("lts/jod", false),
            ("node", false),
        ] {
            assert_eq!(requests(r, v20, N), Some(want), "{r}");
        }
        assert_eq!(requests("lts/*", (24, 1, 0), N), Some(true));
        assert_eq!(requests("garbage(", v20, N), None);
        assert_eq!(requests("lts/unknown", v20, N), None);
    }

    #[test]
    fn toolchains_and_pins() {
        let out = "stable-aarch64-apple-darwin (active, default)\nnightly-2024-01-01-aarch64-apple-darwin\n1.75.0-aarch64-apple-darwin\n";
        let t = parse_toolchains(out).unwrap();
        assert_eq!(t[0], ("stable-aarch64-apple-darwin".into(), true));
        assert!(!t[1].1);
        assert_eq!(parse_toolchains("$(rm -rf /)"), None);
        assert!(pins("nightly-2024-01-01", "nightly-2024-01-01-aarch64-apple-darwin"));
        assert!(pins("1.75", "1.75.0-aarch64-apple-darwin"));
        assert!(pins("1.75.0", "1.75.0-aarch64-apple-darwin"));
        assert!(!pins("1.7", "1.75.0-aarch64-apple-darwin"));
        assert!(!pins("stable", "nightly-2024-01-01-aarch64-apple-darwin"));
    }
}
