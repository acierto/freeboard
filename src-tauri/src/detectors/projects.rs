//! `project.dependencies` (node_modules, .venv) and `project.build_output`
//! (target, dist, build, .next).
//!
//! A generated folder is suggested only with clear ownership evidence, when
//! the project has been inactive for the configured number of days, and when
//! no process is running in the project.

use std::path::{Path, PathBuf};

use super::common::*;
use super::Probe;
use crate::guard::Guard;
use crate::model::*;
use crate::platform;
use crate::runner::Cmd;
use crate::scan::{walk, Detector, ScanContext};

pub const DEPS: &str = "project.dependencies";
pub const BUILD: &str = "project.build_output";
const SOURCE_SCAN_CAP: u64 = 200_000;

pub const LOCKFILES: [(&str, &str); 7] = [
    ("pnpm-lock.yaml", "pnpm"),
    ("yarn.lock", "yarn"),
    ("package-lock.json", "npm"),
    ("npm-shrinkwrap.json", "npm"),
    ("bun.lockb", "bun"),
    ("bun.lock", "bun"),
    ("deno.lock", "deno"),
];
pub const MANIFESTS: [&str; 11] = [
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "setup.py",
    "build.gradle",
    "build.gradle.kts",
    "pom.xml",
    "go.mod",
    "CMakeLists.txt",
    "composer.json",
    "mix.exs",
];

pub struct Dependencies;
pub struct BuildOutput;

#[derive(Debug, Clone, PartialEq)]
pub struct Evidence {
    pub risk: Risk,
    pub action: Action,
    pub manifest: String,
    pub reason: String,
    pub recovery: String,
}

fn exists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).map(|m| m.is_file()).unwrap_or(false)
}

/// Is `name` ignored by Git in `project`? None when unknown (not a repo, git
/// missing or failed).
fn git_ignored(ctx: &ScanContext, project: &Path, name: &str) -> Option<bool> {
    let d = project.to_string_lossy().to_string();
    for cand in [name.to_string(), format!("{name}/")] {
        let out = ctx.runner.run_read(&Cmd::new("git", ["-C", d.as_str(), "check-ignore", "-q", "--", cand.as_str()])).ok()?;
        match out.code {
            Some(0) => return Some(true),
            Some(1) => continue,
            _ => return None,
        }
    }
    Some(false)
}

/// Source ownership-evidence table.
pub fn evidence(ctx: &ScanContext, project: &Path, name: &str) -> Option<Evidence> {
    let trash = trash_word(ctx);
    match name {
        "node_modules" => {
            if !exists(&project.join("package.json")) {
                return None;
            }
            let (lock, pm) = LOCKFILES.iter().find(|(l, _)| exists(&project.join(l)))?;
            Some(Evidence {
                risk: Risk::Safe,
                action: Action::Delete,
                manifest: format!("package.json+{lock}"),
                reason: "The project has a lockfile, so this can be installed again.".into(),
                recovery: format!("Run {pm} install in the project."),
            })
        }
        "target" => exists(&project.join("Cargo.toml")).then(|| Evidence {
            risk: Risk::Safe,
            action: Action::Delete,
            manifest: "Cargo.toml".into(),
            reason: "Build output that Cargo can regenerate.".into(),
            recovery: "Run cargo build.".into(),
        }),
        ".venv" => {
            let m = ["pyproject.toml", "requirements.txt"].into_iter().find(|m| exists(&project.join(m)))?;
            Some(Evidence {
                risk: Risk::Review,
                action: Action::Trash,
                manifest: m.into(),
                reason: "Usually regenerable, but it may hold packages you installed by hand.".into(),
                recovery: format!("Restore it from the {trash}, or recreate it from {m}."),
            })
        }
        "dist" | "build" | ".next" => {
            let m = MANIFESTS.iter().find(|m| exists(&project.join(m)))?;
            if git_ignored(ctx, project, name) != Some(true) {
                return None;
            }
            Some(Evidence {
                risk: Risk::Safe,
                action: Action::Delete,
                manifest: (*m).into(),
                reason: "Build output that Git ignores and the project can regenerate.".into(),
                recovery: "Run the project's build again.".into(),
            })
        }
        _ => None,
    }
}

fn title(name: &str, project: &str) -> String {
    match name {
        "node_modules" => format!("node_modules in {project}"),
        "target" => format!("Rust target folder: {project}"),
        ".venv" => format!(".venv in {project}"),
        n => format!("{n} folder in {project}"),
    }
}

/// Inspect one generated folder. None when it must not be suggested.
pub fn inspect(ctx: &ScanContext, folder: &Path, detector: &str) -> Option<Finding> {
    let name = folder.file_name()?.to_str()?.to_string();
    let project = folder.parent()?;
    if platform::is_link(folder) {
        return None;
    }
    let ev = evidence(ctx, project, &name)?;
    let newest = newest_source_mtime(project, SOURCE_SCAN_CAP)?;
    let idle = ctx.now - newest;
    if idle < ctx.settings.inactive_project_days as i64 * DAY {
        return None;
    }
    let mut risk = ev.risk;
    let mut reason = ev.reason.clone();
    match platform::processes_in_dir(project, ctx.runner) {
        Some(p) if !p.is_empty() => return None,
        Some(_) => {}
        None => {
            // Unknown means less safe.
            risk = risk.raised();
            reason.push_str(" Freeboard couldn't check for programs running in this project.");
        }
    }
    let ex = walk::exclusions(ctx);
    let st = path_state(folder, &ex)?;
    if st.is_link {
        return None;
    }
    let pname = project.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut s = base(detector, Category::Builds, &st.canonical.to_string_lossy());
    s.title = title(&name, &pname);
    s.location = display_path(ctx, folder);
    s.size_bytes = st.m.reclaimable();
    s.size_is_estimate = st.m.is_estimate();
    s.last_used_at = rfc3339(newest);
    s.last_used_label = format!("Project idle for {} days", days_since(ctx.now, newest));
    s.risk = risk;
    // A raised Safe item moves to the Trash, like other Review items.
    s.action = if risk != Risk::Safe && ev.action == Action::Delete { Action::Trash } else { ev.action };
    s.action_label = if s.action == Action::Trash { move_label(ctx) } else { "Delete the folder".into() };
    s.recovery = if s.action == Action::Trash && ev.action == Action::Delete {
        format!("Restore it from the {}, or: {}", trash_word(ctx), ev.recovery)
    } else {
        ev.recovery
    };
    s.reason = reason;
    s.recoverable = true;
    s.fingerprint = format!("{};m:{}", fingerprint_paths(&[st]), ev.manifest);
    Some(Finding { suggestion: s, target: Target::Paths { paths: vec![folder.to_string_lossy().to_string()] } })
}

fn run_for(ctx: &ScanContext, detector: &str, names: &[&str]) -> Result<Vec<Finding>, DetectorError> {
    let d = walk::discover(ctx);
    let mut out = vec![];
    for g in &d.generated {
        if !g.file_name().and_then(|n| n.to_str()).is_some_and(|n| names.contains(&n)) {
            continue;
        }
        if !walk::wait_if_paused(ctx) {
            break;
        }
        (ctx.progress)("projects", Some(detector), out.len() as u64);
        if let Some(f) = inspect(ctx, g, detector) {
            out.push(f);
        }
    }
    Ok(out)
}

impl Detector for Dependencies {
    fn id(&self) -> &'static str {
        DEPS
    }
    fn deep_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        run_for(ctx, DEPS, &["node_modules", ".venv"])
    }
}

impl Detector for BuildOutput {
    fn id(&self) -> &'static str {
        BUILD
    }
    fn deep_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        run_for(ctx, BUILD, &["target", "dist", "build", ".next"])
    }
}

/// Pre-cleanup check: path checks, manifest still present, no process.
pub fn probe(f: &Finding, ctx: &ScanContext, guard: &Guard) -> Probe {
    let r = super::files::probe_paths(f, ctx, guard);
    if r != Probe::Unchanged {
        return r;
    }
    let Target::Paths { paths } = &f.target else { return Probe::Unknown("Unexpected cleanup target.".into()) };
    let folder = PathBuf::from(&paths[0]);
    let Some(project) = folder.parent() else { return Probe::Unknown("No project folder.".into()) };
    let name = folder.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let manifest = f.suggestion.fingerprint.rsplit(";m:").next().unwrap_or_default();
    let still = match evidence(ctx, project, name) {
        Some(ev) => ev.manifest == manifest,
        None => false,
    };
    if !still {
        return Probe::Changed("The project's manifest or lockfile changed after the scan.".into());
    }
    match platform::processes_in_dir(project, ctx.runner) {
        Some(p) if !p.is_empty() => Probe::Changed("A program is now running in this project.".into()),
        Some(_) => Probe::Unchanged,
        None if f.suggestion.risk != Risk::Safe => Probe::Unchanged, // already accounted for as unknown
        None => Probe::Unknown("Freeboard couldn't check for programs running in this project.".into()),
    }
}
