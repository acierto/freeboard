//! `git.worktrees`: linked worktrees of repositories under the scan folders.
//!
//! The main worktree is never suggested. Clean worktrees whose branch is
//! merged, or pushed but not merged, are Review. Dirty, untracked, unpushed,
//! no-upstream, locked, merging/rebasing, process-open and unknown-state
//! worktrees are Risky and blocked.
//!
//! Removal itself is disabled (task 4.1): the source protects repository roots
//! but also lists `git worktree remove` for linked worktrees, whose roots are
//! repository roots. Until the user confirms the rule, every worktree carries
//! a blocked reason; discovery and risk presentation still work.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::common::*;
use super::Probe;
use crate::model::*;
use crate::platform;
use crate::runner::Cmd;
use crate::scan::size::{measure, MeasureOpts};
use crate::scan::{walk, Detector, ScanContext};

pub const ID: &str = "git.worktrees";

pub struct Worktrees;

#[derive(Debug, Clone, PartialEq)]
pub struct WtEntry {
    pub path: PathBuf,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub locked: bool,
    pub prunable: bool,
    pub bare: bool,
}

/// Parse `git worktree list --porcelain`. The first entry is the main worktree.
pub fn parse_porcelain(s: &str) -> Vec<WtEntry> {
    let mut out = vec![];
    let mut cur: Option<WtEntry> = None;
    for line in s.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(c) = cur.take() {
                out.push(c);
            }
            cur = Some(WtEntry { path: PathBuf::from(p), head: None, branch: None, locked: false, prunable: false, bare: false });
        } else if let Some(c) = cur.as_mut() {
            if let Some(h) = line.strip_prefix("HEAD ") {
                c.head = Some(h.to_string());
            } else if let Some(b) = line.strip_prefix("branch ") {
                c.branch = Some(b.trim_start_matches("refs/heads/").to_string());
            } else if line == "locked" || line.starts_with("locked ") {
                c.locked = true;
            } else if line == "prunable" || line.starts_with("prunable ") {
                c.prunable = true;
            } else if line == "bare" {
                c.bare = true;
            }
        }
    }
    if let Some(c) = cur {
        out.push(c);
    }
    out
}

fn git(ctx: &ScanContext, dir: &Path, args: &[&str]) -> Result<(i32, String), String> {
    let d = dir.to_string_lossy().to_string();
    let mut a = vec!["-C", d.as_str()];
    a.extend_from_slice(args);
    let out = ctx.runner.run_read(&Cmd::new("git", a)).map_err(|e| e.to_string())?;
    Ok((out.code.unwrap_or(-1), out.stdout))
}

fn git_ok(ctx: &ScanContext, dir: &Path, args: &[&str]) -> Result<String, String> {
    match git(ctx, dir, args)? {
        (0, s) => Ok(s.trim().to_string()),
        (c, _) => Err(format!("git exited with {c}")),
    }
}

/// Main repository roots for the discovered `.git` entries.
fn main_roots(ctx: &ScanContext, repos: &[PathBuf]) -> Vec<PathBuf> {
    let mut set = BTreeSet::new();
    for r in repos {
        let dotgit = r.join(".git");
        let Ok(m) = std::fs::symlink_metadata(&dotgit) else { continue };
        if m.is_dir() {
            set.insert(r.clone());
        } else if let Ok(common) = git_ok(ctx, r, &["rev-parse", "--path-format=absolute", "--git-common-dir"]) {
            let common = PathBuf::from(common);
            if common.file_name().is_some_and(|n| n == ".git") {
                if let Some(p) = common.parent() {
                    set.insert(p.to_path_buf());
                }
            }
        }
    }
    set.into_iter().collect()
}

fn default_branch(ctx: &ScanContext, main: &Path) -> Option<String> {
    if let Ok(r) = git_ok(ctx, main, &["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"]) {
        if !r.is_empty() {
            return Some(r);
        }
    }
    for cand in ["refs/remotes/origin/main", "refs/remotes/origin/master", "refs/heads/main", "refs/heads/master"] {
        if git(ctx, main, &["rev-parse", "--verify", "--quiet", cand]).is_ok_and(|(c, _)| c == 0) {
            return Some(cand.trim_start_matches("refs/remotes/").trim_start_matches("refs/heads/").to_string());
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub struct Classified {
    pub risk: Risk,
    /// Reason for the risk (shown as "Why it is listed").
    pub reason: String,
    /// Risky states are blocked with their own reason.
    pub unsafe_reason: Option<String>,
    pub fingerprint: String,
    pub last_commit: Option<i64>,
    pub merged: bool,
}

fn unknown(why: &str) -> Classified {
    Classified {
        risk: Risk::Risky,
        reason: format!("Freeboard couldn't determine this worktree's state ({why})."),
        unsafe_reason: Some("Blocked because its status can't be determined.".into()),
        fingerprint: fingerprint_of(&["unknown", why]),
        last_commit: None,
        merged: false,
    }
}

/// Classify one linked worktree. Any failure is Risky (invariant 11).
pub fn classify(ctx: &ScanContext, main: &Path, wt: &WtEntry, default: Option<&str>) -> Classified {
    let p = &wt.path;
    let status = match git(ctx, p, &["status", "--porcelain=v2", "--branch", "--untracked-files=normal"]) {
        Ok((0, s)) => s,
        Ok(_) => return unknown("git status failed"),
        Err(e) => return unknown(&e),
    };
    let mut upstream = None;
    let mut ahead: Option<u64> = None;
    let (mut changed, mut untracked) = (0u32, 0u32);
    for l in status.lines() {
        if let Some(u) = l.strip_prefix("# branch.upstream ") {
            upstream = Some(u.to_string());
        } else if let Some(ab) = l.strip_prefix("# branch.ab ") {
            ahead = ab.split_whitespace().next().and_then(|a| a.trim_start_matches('+').parse().ok());
        } else if l.starts_with("? ") {
            untracked += 1;
        } else if l.starts_with("1 ") || l.starts_with("2 ") || l.starts_with("u ") {
            changed += 1;
        }
    }
    let gitdir = match git_ok(ctx, p, &["rev-parse", "--absolute-git-dir"]) {
        Ok(d) => PathBuf::from(d),
        Err(e) => return unknown(&e),
    };
    let in_progress = ["MERGE_HEAD", "rebase-merge", "rebase-apply", "CHERRY_PICK_HEAD", "REVERT_HEAD"]
        .iter()
        .any(|f| std::fs::symlink_metadata(gitdir.join(f)).is_ok());
    let last_commit = git_ok(ctx, p, &["log", "-1", "--format=%ct"]).ok().and_then(|s| s.parse().ok());
    let merged = match (default, wt.head.as_deref()) {
        (Some(d), Some(_)) => match git(ctx, p, &["merge-base", "--is-ancestor", "HEAD", d]) {
            Ok((0, _)) => Some(true),
            Ok((1, _)) => Some(false),
            _ => None,
        },
        _ => Some(false),
    };
    let procs = platform::processes_in_dir(p, ctx.runner);
    let fp = fingerprint_of(&[
        wt.head.as_deref().unwrap_or(""),
        &status,
        &format!("{}|{in_progress}", wt.locked),
        &format!("{merged:?}"),
    ]);
    let risky = |reason: String, block: &str| Classified {
        risk: Risk::Risky,
        reason,
        unsafe_reason: Some(block.to_string()),
        fingerprint: fp.clone(),
        last_commit,
        merged: merged == Some(true),
    };
    let _ = main;
    if wt.locked {
        return risky("The worktree is locked.".into(), "Blocked because the worktree is locked.");
    }
    if in_progress {
        return risky("A merge, rebase or cherry-pick is in progress.".into(), "Blocked until the merge or rebase is finished.");
    }
    if changed > 0 || untracked > 0 {
        let mut parts = vec![];
        if changed > 0 {
            parts.push(format!("{changed} uncommitted file{}", if changed == 1 { "" } else { "s" }));
        }
        if untracked > 0 {
            parts.push(format!("{untracked} untracked file{}", if untracked == 1 { "" } else { "s" }));
        }
        return risky(format!("{}.", parts.join(" and ")), "Blocked in v1. Commit and push this work first.");
    }
    match procs {
        None => return risky("Freeboard couldn't check whether a program has this worktree open.".into(), "Blocked because its status can't be determined."),
        Some(v) if !v.is_empty() => return risky("A running program has this worktree open.".into(), "Blocked while a program is using it."),
        _ => {}
    }
    let Some(merged) = merged else { return unknown("merge check failed") };
    if merged {
        return Classified {
            risk: Risk::Review,
            reason: "The branch is merged and the worktree has no local changes.".into(),
            unsafe_reason: None,
            fingerprint: fp,
            last_commit,
            merged: true,
        };
    }
    match (upstream, ahead) {
        (None, _) | (Some(_), None) => risky("The branch has no upstream, so its commits may exist only here.".into(), "Blocked in v1. Push this branch first."),
        (Some(_), Some(a)) if a > 0 => risky(
            format!("{a} unpushed commit{}.", if a == 1 { "" } else { "s" }),
            "Blocked in v1. Push this work first.",
        ),
        _ => Classified {
            risk: Risk::Review,
            // Git can't detect squash merges reliably; they show up here.
            reason: "No local changes, and the branch is pushed but not merged.".into(),
            unsafe_reason: None,
            fingerprint: fp,
            last_commit,
            merged: false,
        },
    }
}

fn finding(ctx: &ScanContext, main: &Path, wt: &WtEntry, c: Classified) -> Finding {
    let canon = std::fs::canonicalize(&wt.path).unwrap_or_else(|_| wt.path.clone());
    let mut s = base(ID, Category::Worktrees, &canon.to_string_lossy());
    let repo = main.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let label = wt.branch.clone().unwrap_or_else(|| wt.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    s.title = format!("{repo}: {label}");
    s.location = display_path(ctx, &wt.path);
    let ex = walk::exclusions(ctx);
    s.size_bytes = measure(&wt.path, &MeasureOpts { exclusions: ex, serial: false }).bytes;
    s.last_used_at = c.last_commit.and_then(rfc3339);
    s.last_used_label = match (c.merged, c.last_commit) {
        (true, Some(t)) => format!("Merged, last commit {}", ago(ctx.now, t)),
        (_, Some(t)) => format!("Last commit {}", ago(ctx.now, t)),
        _ => "Last commit unknown".into(),
    };
    s.risk = c.risk;
    s.action = Action::NativeCommand;
    s.reason = c.reason;
    s.fingerprint = c.fingerprint;
    if c.risk == Risk::Risky {
        s.action_label = "None".into();
        s.recovery = "Your work exists only here.".into();
        s.recoverable = false;
        s.blocked_reason = c.unsafe_reason;
    } else {
        s.action_label = format!("git worktree remove {}", wt.path.display());
        s.recovery = "Run git worktree add with the same branch.".into();
        s.recoverable = true;
        s.blocked_reason = None;
    }
    Finding {
        suggestion: s,
        target: Target::Native {
            program: "git".into(),
            args: vec!["worktree".into(), "remove".into(), wt.path.to_string_lossy().to_string()],
            resources: vec![canon.to_string_lossy().to_string()],
            cwd: Some(main.to_string_lossy().to_string()),
        },
    }
}

fn linked(ctx: &ScanContext, main: &Path) -> Result<Vec<WtEntry>, String> {
    let out = git_ok(ctx, main, &["worktree", "list", "--porcelain"])?;
    let mut all = parse_porcelain(&out);
    if all.is_empty() {
        return Err("no worktrees listed".into());
    }
    all.remove(0); // main worktree: never suggested
    Ok(all.into_iter().filter(|w| !w.bare && !w.prunable).collect())
}

impl Detector for Worktrees {
    fn id(&self) -> &'static str {
        ID
    }
    fn deep_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let d = walk::discover(ctx);
        if d.repos.is_empty() {
            return Ok(vec![]);
        }
        if !ctx.runner.available("git") {
            return Err(err(ID, "unavailable", "Git is not installed."));
        }
        let ex = walk::exclusions(ctx);
        let mut out = vec![];
        let mut failures = 0;
        for main in main_roots(ctx, &d.repos) {
            if !walk::wait_if_paused(ctx) {
                break;
            }
            (ctx.progress)("worktrees", Some(ID), out.len() as u64);
            let wts = match linked(ctx, &main) {
                Ok(w) => w,
                Err(_) => {
                    failures += 1;
                    continue;
                }
            };
            let default = default_branch(ctx, &main);
            for wt in wts {
                if ex.iter().any(|x| wt.path.starts_with(x)) || platform::is_wsl_path(&wt.path) {
                    continue;
                }
                let c = classify(ctx, &main, &wt, default.as_deref());
                out.push(finding(ctx, &main, &wt, c));
            }
        }
        if out.is_empty() && failures > 0 {
            return Err(err(ID, "error", format!("Git could not list worktrees in {failures} repositories.")));
        }
        Ok(out)
    }
}

/// Pre-cleanup check: still clean, still pushed or merged, not locked, no new
/// commits. (Removal stays disabled; this is used for the record.)
pub fn probe(f: &Finding, ctx: &ScanContext) -> Probe {
    let Target::Native { cwd: Some(main), resources, .. } = &f.target else {
        return Probe::Unknown("Unexpected cleanup target.".into());
    };
    let main = PathBuf::from(main);
    let wts = match linked(ctx, &main) {
        Ok(w) => w,
        Err(e) => return Probe::Unknown(format!("Git could not list worktrees: {e}")),
    };
    let Some(wt) = wts.into_iter().find(|w| std::fs::canonicalize(&w.path).ok().is_some_and(|c| c.to_string_lossy() == resources[0])) else {
        return Probe::Changed("The worktree no longer exists.".into());
    };
    let c = classify(ctx, &main, &wt, default_branch(ctx, &main).as_deref());
    if c.risk != Risk::Review {
        return Probe::Changed(c.reason);
    }
    if c.fingerprint != f.suggestion.fingerprint {
        return Probe::Changed("A new commit or change appeared after the scan.".into());
    }
    Probe::Unchanged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain() {
        let s = "worktree /r\nHEAD aaa\nbranch refs/heads/main\n\nworktree /r/.wt/a\nHEAD bbb\nbranch refs/heads/feature\nlocked reason\n\nworktree /gone\nHEAD ccc\ndetached\nprunable gitdir file points to non-existent location\n";
        let v = parse_porcelain(s);
        assert_eq!(v.len(), 3);
        assert_eq!(v[1].branch.as_deref(), Some("feature"));
        assert!(v[1].locked);
        assert!(v[2].prunable);
        assert_eq!(v[2].branch, None);
    }
}
