//! `git.worktrees`: linked worktrees of repositories under the scan folders,
//! one item each, for the Worktrees view (spec "Git worktrees", Version 19).
//!
//! `git worktree remove` deletes the folder only; the branch and its commits
//! stay. So a clean worktree idle 7+ days is Ready whether or not its branch
//! is pushed or merged, and its branch is deleted too only when its content
//! is already in the default branch. Recently used or locked worktrees need a
//! look (selectable by hand). Uncommitted files, a merge in progress, a
//! program using it, commits on no branch, or an unreadable state are Work
//! and are not removed. The main checkout is never an item.

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

/// Days without use before a clean worktree is Ready (spec "Git worktrees").
pub const IDLE_DAYS: u32 = 7;

#[derive(Debug, Clone, PartialEq)]
pub struct Classified {
    pub status: WtStatus,
    /// One plain sentence: why it has this status.
    pub reason: String,
    /// Work items are not removed; this says what would make them removable.
    pub blocked: Option<String>,
    pub fingerprint: String,
    pub last_commit: Option<i64>,
    /// The latest of the last commit, checkout, and change at its top level.
    pub last_used: Option<i64>,
    pub merged: WtMerged,
    pub ahead: u32,
    pub pushed: bool,
    pub dirty: u32,
    pub dirty_files: Vec<String>,
    pub hold: Option<&'static str>,
}

fn unknown(why: &str) -> Classified {
    Classified {
        status: WtStatus::Work,
        reason: format!("Freeboard couldn't determine this worktree's state ({why})."),
        blocked: Some("Blocked because its status can't be determined.".into()),
        fingerprint: fingerprint_of(&["unknown", why]),
        last_commit: None,
        last_used: None,
        merged: WtMerged::No,
        ahead: 0,
        pushed: false,
        dirty: 0,
        dirty_files: vec![],
        hold: Some("unknown"),
    }
}

fn mtime(p: &Path) -> Option<i64> {
    let m = std::fs::metadata(p).ok()?.modified().ok()?;
    m.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

/// The time of a reflog's last entry ("<old> <new> Name <email> <time> <tz>\t<message>").
fn reflog_time(p: &Path) -> Option<i64> {
    let text = std::fs::read_to_string(p).ok()?;
    let line = text.lines().rev().find(|l| !l.trim().is_empty())?;
    let head = line.split('\t').next()?;
    let mut parts = head.rsplitn(3, ' ');
    let _tz = parts.next()?;
    parts.next()?.parse().ok()
}

/// "Idle 30 days", "Used today": the idle part of a reason.
fn used(now: i64, last: Option<i64>) -> String {
    match last.map(|t| ((now - t).max(0) / 86_400) as u32) {
        None => "Last use unknown".into(),
        Some(0) => "Used today".into(),
        Some(1) => "Used yesterday".into(),
        Some(d) => format!("Used {d} days ago"),
    }
}

/// Is the branch's content already in the default branch although it is not
/// an ancestor (a squash merge)? Every file the branch changed since it left
/// the default branch is then the same in both. Read-only, unlike the spec's
/// `git merge-tree --write-tree`, which writes objects into the repository
/// (decision 2026-11-07). If the default branch changed those files again
/// later, this says no and the branch is kept: never a wrong yes.
fn squash_merged(ctx: &ScanContext, p: &Path, default: &str) -> bool {
    let Ok(base) = git_ok(ctx, p, &["merge-base", default, "HEAD"]) else { return false };
    let Ok(files) = git_ok(ctx, p, &["diff", "--name-only", "-z", &base, "HEAD"]) else { return false };
    let files: Vec<&str> = files.split('\0').filter(|f| !f.is_empty()).collect();
    if files.is_empty() {
        return false;
    }
    let mut args = vec!["diff", "--quiet", default, "HEAD", "--"];
    args.extend(files.iter().copied());
    matches!(git(ctx, p, &args), Ok((0, _)))
}

/// Classify one linked worktree (spec "Git worktrees"). Removing a worktree
/// deletes its folder, never its branch, so only uncommitted files, commits on
/// no branch, or work in progress can be lost; those are Work. Any failure to
/// read the state is Work too (invariant 11).
pub fn classify(ctx: &ScanContext, main: &Path, wt: &WtEntry, default: Option<&str>) -> Classified {
    let p = &wt.path;
    let status = match git(ctx, p, &["status", "--porcelain=v2", "--branch", "--untracked-files=normal"]) {
        Ok((0, s)) => s,
        Ok(_) => return unknown("git status failed"),
        Err(e) => return unknown(&e),
    };
    let mut upstream = None;
    let mut dirty_files = vec![];
    let mut dirty = 0u32;
    for l in status.lines() {
        if let Some(u) = l.strip_prefix("# branch.upstream ") {
            upstream = Some(u.to_string());
        } else if l.starts_with("? ") || l.starts_with("1 ") || l.starts_with("2 ") || l.starts_with("u ") {
            dirty += 1;
            // The path is the last field ("1 .M N... 100644 100644 100644 h h path").
            let name = if let Some(n) = l.strip_prefix("? ") {
                n.to_string()
            } else {
                let fields = if l.starts_with("1 ") { 8 } else if l.starts_with("2 ") { 9 } else { 10 };
                l.splitn(fields + 1, ' ').nth(fields).unwrap_or("").split('\t').next().unwrap_or("").to_string()
            };
            if dirty_files.len() < 8 && !name.is_empty() {
                dirty_files.push(name);
            }
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
    // Last use: commits, checkouts (the time in the reflog's last entry and
    // HEAD's date) and files coming or going at the top level. Not the
    // reflog's own file date, which `git gc` rewrites in every worktree
    // (found on a real Mac, 2026-11-07), nor the index, which `git status`
    // rewrites.
    let last_used = [last_commit, reflog_time(&gitdir.join("logs/HEAD")), mtime(&gitdir.join("HEAD")), mtime(p)].into_iter().flatten().max();
    let idle = last_used.map(|t| ((ctx.now - t).max(0) / 86_400) as u32);
    let (merged, ahead) = match (default, wt.head.as_deref()) {
        (Some(d), Some(_)) => {
            let anc = match git(ctx, p, &["merge-base", "--is-ancestor", "HEAD", d]) {
                Ok((0, _)) => Some(true),
                Ok((1, _)) => Some(false),
                _ => None,
            };
            let Some(anc) = anc else { return unknown("merge check failed") };
            let ahead = git_ok(ctx, p, &["rev-list", "--count", &format!("{d}..HEAD")]).ok().and_then(|n| n.parse().ok()).unwrap_or(0);
            let merged = if anc {
                WtMerged::Yes
            } else if squash_merged(ctx, p, d) {
                WtMerged::Squash
            } else {
                WtMerged::No
            };
            (merged, ahead)
        }
        _ => (WtMerged::No, 0),
    };
    let procs = crate::scan::processes_in(ctx, p);
    let fp = fingerprint_of(&[
        wt.head.as_deref().unwrap_or(""),
        &status,
        &format!("{}|{in_progress}", wt.locked),
        &format!("{merged:?}"),
        &last_used.unwrap_or(0).to_string(),
    ]);
    let base = Classified {
        status: WtStatus::Ready,
        reason: String::new(),
        blocked: None,
        fingerprint: fp,
        last_commit,
        last_used,
        merged,
        ahead,
        pushed: upstream.is_some(),
        dirty,
        dirty_files,
        hold: None,
    };
    let work = |reason: String, block: &str, hold: Option<&'static str>| Classified { status: WtStatus::Work, reason, blocked: Some(block.to_string()), hold, ..base.clone() };
    let _ = main;
    // Held: never removed, not even after the user's acknowledgment.
    if in_progress {
        return work("A merge, rebase or cherry-pick is in progress.".into(), "Blocked until the merge or rebase is finished.", Some("merging"));
    }
    match procs {
        None => return work("Freeboard couldn't check whether a program has this worktree open.".into(), "Blocked because its status can't be determined.", Some("unknown")),
        Some(v) if !v.is_empty() => return work("A running program has this worktree open.".into(), "Blocked while a program is using it.", Some("in_use")),
        _ => {}
    }
    // Commits on no branch would be lost with the folder.
    if wt.branch.is_none() && ahead > 0 {
        return work("Its commits are on no branch, so they would be lost.".into(), "Create a branch for them first, then scan again.", Some("no_branch"));
    }
    // Uncommitted files: removable only after the user acknowledges that
    // these files go (spec "Git worktrees", decision 2026-11-07).
    if dirty > 0 {
        let reason = if dirty == 1 { "1 uncommitted file would be deleted.".to_string() } else { format!("{dirty} uncommitted files would be deleted.") };
        return Classified { status: WtStatus::Work, reason, ..base };
    }
    if wt.locked {
        return Classified { status: WtStatus::Look, reason: format!("Locked, maybe by an agent that is still running. {}.", used(ctx.now, last_used)), ..base };
    }
    if idle.is_none_or(|d| d < IDLE_DAYS) {
        return Classified { status: WtStatus::Look, reason: format!("{}. It may still be in use.", used(ctx.now, last_used)), ..base };
    }
    let d = idle.unwrap_or(0);
    let reason = match merged {
        WtMerged::Squash => format!("Idle {d} days, already in main (squash-merged)."),
        WtMerged::Yes => format!("Idle {d} days, merged into main."),
        WtMerged::No => format!("Idle {d} days, nothing uncommitted. The branch stays, so you can reopen it anytime."),
    };
    Classified { reason, ..base }
}

/// "agent", "claude" or "codex" for branches coding agents create.
pub fn agent_of(branch: Option<&str>) -> Option<String> {
    let b = branch?;
    if b.starts_with("worktree-agent-") {
        Some("agent".into())
    } else if b.starts_with("claude/") {
        Some("claude".into())
    } else if b.starts_with("codex/") {
        Some("codex".into())
    } else {
        None
    }
}

/// Does removal delete the branch too? Only when its content is in the
/// default branch, and never the default branch itself.
fn branch_goes(c: &Classified, branch: Option<&str>, default: Option<&str>) -> bool {
    let Some(b) = branch else { return false };
    let default_short = default.map(|d| d.rsplit('/').next().unwrap_or(d));
    c.merged != WtMerged::No && Some(b) != default_short && !matches!(b, "main" | "master")
}

fn finding(ctx: &ScanContext, main: &Path, wt: &WtEntry, c: Classified, default: Option<&str>) -> Finding {
    let canon = std::fs::canonicalize(&wt.path).unwrap_or_else(|_| wt.path.clone());
    let mut s = base(ID, Group::Projects, &canon.to_string_lossy());
    let repo = main.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let label = wt.branch.clone().unwrap_or_else(|| wt.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    s.title = format!("{repo}: {label}");
    s.location = display_path(ctx, &wt.path);
    let ex = walk::exclusions(ctx);
    s.size_bytes = measure(&wt.path, &MeasureOpts { exclusions: ex, serial: false }).bytes;
    s.last_used_at = c.last_used.and_then(rfc3339);
    s.last_used_label = match (c.merged, c.last_commit) {
        (WtMerged::Yes, Some(t)) => format!("Merged, last commit {}", ago(ctx.now, t)),
        (_, Some(t)) => format!("Last commit {}", ago(ctx.now, t)),
        _ => "Last commit unknown".into(),
    };
    s.action = Action::NativeCommand;
    s.reason = c.reason.clone();
    s.fingerprint = c.fingerprint.clone();
    let rel = wt.path.strip_prefix(main).map(|r| r.to_string_lossy().to_string()).unwrap_or_else(|_| display_path(ctx, &wt.path));
    let root = display_path(ctx, main);
    let goes = branch_goes(&c, wt.branch.as_deref(), default);
    let mut commands = vec![];
    if wt.locked {
        commands.push(format!("git -C {root} worktree unlock {rel}"));
    }
    let forced = c.status == WtStatus::Work && c.hold.is_none() && c.dirty > 0;
    commands.push(format!("git -C {root} worktree remove {}{rel}", if forced { "--force " } else { "" }));
    if goes {
        let flag = if c.merged == WtMerged::Yes { "-d" } else { "-D" };
        commands.push(format!("git -C {root} branch {flag} {}", wt.branch.as_deref().unwrap_or("")));
    }
    let reopen = match (&wt.branch, goes) {
        (Some(b), false) => Some(format!("git worktree add {rel} {b}")),
        _ => None,
    };
    if forced {
        // Uncommitted files: the user may remove it after acknowledging that
        // these files are deleted for good. The branch and its commits stay.
        s.risk = Risk::Risky;
        s.action_label = format!("git worktree remove --force {}", wt.path.display());
        s.recovery = "Not recoverable: the uncommitted files are deleted. The branch and its commits stay.".into();
        s.recoverable = false;
        s.blocked_reason = None;
        s.needs_confirmation = true;
        s.ack_name = Some(if c.dirty == 1 { "1 uncommitted file".into() } else { format!("{} uncommitted files", c.dirty) });
    } else if c.status == WtStatus::Work {
        s.risk = Risk::Risky;
        s.action_label = "None".into();
        s.recovery = "Your work exists only here.".into();
        s.recoverable = false;
        s.blocked_reason = c.blocked.clone();
    } else {
        // Nothing is lost: the branch keeps every commit, or its content is
        // already in the default branch. Never automatic (Review).
        s.risk = Risk::Review;
        s.action_label = format!("git worktree remove {}", wt.path.display());
        s.recovery = match &reopen {
            Some(r) => format!("The branch stays. Reopen it with {r}."),
            None => "Everything is already in main.".into(),
        };
        s.recoverable = true;
        s.blocked_reason = None;
    }
    s.worktree = Some(WorktreeInfo {
        repo,
        root,
        branch: wt.branch.clone(),
        rel_path: rel,
        status: c.status,
        agent: agent_of(wt.branch.as_deref()),
        locked: wt.locked,
        idle_days: c.last_used.map(|t| ((ctx.now - t).max(0) / 86_400) as u32),
        dirty: c.dirty,
        dirty_files: c.dirty_files.clone(),
        merged: c.merged,
        ahead: c.ahead,
        pushed: c.pushed,
        hold: c.hold.map(str::to_string),
        commands,
        reopen,
    });
    Finding {
        parts: vec![],
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
        let mut failures = 0;
        // List every repository's linked worktrees first (one quick git call each).
        let mut jobs: Vec<(PathBuf, WtEntry, Option<String>)> = vec![];
        for (checked, main) in main_roots(ctx, &d.repos).into_iter().enumerate() {
            if !walk::wait_if_paused(ctx) {
                break;
            }
            // Progress: repositories checked so far.
            (ctx.progress)("worktrees", Some(ID), checked as u64);
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
                jobs.push((main.clone(), wt, default.clone()));
            }
        }
        // Then classify and measure the worktrees in parallel: each one waits
        // on git and the disk, so a small pool cuts a scan of hundreds of
        // worktrees from minutes to seconds. Results keep the listing order.
        let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8);
        let next = std::sync::atomic::AtomicUsize::new(0);
        let results: Vec<std::sync::Mutex<Option<Finding>>> = jobs.iter().map(|_| std::sync::Mutex::new(None)).collect();
        std::thread::scope(|sc| {
            for _ in 0..workers {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((main, wt, default)) = jobs.get(i) else { break };
                    if !walk::wait_if_paused(ctx) {
                        break;
                    }
                    let c = classify(ctx, main, wt, default.as_deref());
                    *results[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(finding(ctx, main, wt, c, default.as_deref()));
                });
            }
        });
        let out: Vec<Finding> = results.into_iter().filter_map(|m| m.into_inner().unwrap_or_else(|e| e.into_inner())).collect();
        if out.is_empty() && failures > 0 {
            return Err(err(ID, "error", format!("Git could not list worktrees in {failures} repositories.")));
        }
        // One item per worktree: the Worktrees view lists and removes them
        // one by one (spec "Worktrees view"), so they are no longer grouped.
        Ok(out)
    }
}

/// Pre-cleanup check (spec "Pre-cleanup check", Worktree): still no
/// uncommitted or untracked files, not locked unless the plan unlocks it (the
/// lock is part of the fingerprint), not used since the scan.
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
    // Held work never goes. Uncommitted files go only for an item that asked
    // for the user's acknowledgment (the plan requires it), and only while
    // they are exactly the ones found at the scan (the fingerprint).
    let acknowledged = f.suggestion.needs_confirmation && c.hold.is_none() && c.dirty > 0;
    if c.status == WtStatus::Work && !acknowledged {
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
