use super::*;
use crate::detectors::git::*;
use crate::detectors::{probe, Probe};
use crate::model::{WorktreeInfo, WtMerged, WtStatus};

/// Builds `projects/app` with a bare `origin` and one linked worktree per
/// state. Commits are dated 30 days before `NOW` and everything is aged, so
/// clean worktrees are idle; `fresh` is made last and used today. Returns the
/// app path.
fn repo(fx: &Fx) -> PathBuf {
    let origin = fx.root.join("origin.git");
    std::fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "--bare", "-q"]);
    let app = fx.mkdir("projects/app");
    let g = |dir: &Path, args: &[&str]| git_old(dir, args, 30);
    g(&app, &["init", "-q"]);
    std::fs::write(app.join("README.md"), "hello\n").unwrap();
    g(&app, &["add", "."]);
    g(&app, &["commit", "-q", "-m", "init"]);
    g(&app, &["remote", "add", "origin", &origin.to_string_lossy()]);
    g(&app, &["push", "-q", "-u", "origin", "main"]);

    let wt = |name: &str| {
        let p = app.join(".worktrees").join(name.replace('/', "-"));
        g(&app, &["worktree", "add", "-q", "-b", name, &p.to_string_lossy()]);
        p
    };
    let commit = |p: &Path, file: &str| {
        std::fs::write(p.join(file), file).unwrap();
        g(p, &["add", "."]);
        g(p, &["commit", "-q", "-m", file]);
    };

    // Merged into the default branch: Ready, the branch goes too (-d).
    let merged = wt("merged");
    commit(&merged, "m.txt");
    g(&app, &["merge", "-q", "--no-edit", "merged"]);
    // Pushed, not merged: Ready, the branch stays.
    let pushed = wt("pushed");
    commit(&pushed, "p.txt");
    g(&pushed, &["push", "-q", "-u", "origin", "pushed"]);
    // Squash-merged: content in main, not an ancestor: Ready, branch goes (-D).
    let squash = wt("squash");
    commit(&squash, "s.txt");
    g(&app, &["merge", "-q", "--squash", "squash"]);
    g(&app, &["commit", "-q", "-m", "squash"]);
    g(&app, &["push", "-q", "origin", "main"]);
    // Unpushed commits and no upstream: Ready, the branch keeps them.
    let unpushed = wt("unpushed");
    commit(&unpushed, "a.txt");
    commit(&unpushed, "b.txt");
    // A coding agent's branch, never pushed: Ready, with its badge.
    let agent = wt("worktree-agent-a1b2");
    commit(&agent, "agent.txt");
    // Uncommitted change and untracked file: Work.
    let dirty = wt("dirty");
    commit(&dirty, "d.txt");
    std::fs::write(dirty.join("d.txt"), "changed").unwrap();
    let untracked = wt("untracked");
    std::fs::write(untracked.join("new.txt"), "new").unwrap();
    // Locked: Needs a look; removal unlocks it first.
    let locked = wt("locked");
    commit(&locked, "l.txt");
    g(&app, &["worktree", "lock", &locked.to_string_lossy()]);
    // In the middle of a merge: Work.
    let merging = wt("merging");
    commit(&merging, "g.txt");
    let gd = g(&merging, &["rev-parse", "--absolute-git-dir"]);
    std::fs::write(Path::new(&gd).join("MERGE_HEAD"), g(&merging, &["rev-parse", "HEAD"])).unwrap();
    // Status can't be determined: the link to its repository is broken.
    let broken = wt("broken");
    commit(&broken, "x.txt");
    std::fs::write(broken.join(".git"), "gitdir: /nonexistent/freeboard\n").unwrap();
    age(&app, 30);
    // Used today: Needs a look.
    let fresh = app.join(".worktrees/fresh");
    git(&app, &["worktree", "add", "-q", "-b", "fresh", &fresh.to_string_lossy()]);
    app
}

/// Every worktree finding.
fn leaves(o: &ScanOutcome) -> Vec<&Finding> {
    o.findings.iter().collect()
}

fn by_branch<'a>(o: &'a ScanOutcome, b: &str) -> &'a Suggestion {
    leaves(o)
        .into_iter()
        .map(|f| &f.suggestion)
        .find(|s| s.detector == ID && s.title.ends_with(&format!(": {b}")))
        .unwrap_or_else(|| panic!("no worktree {b}: {:#?}", o.findings.iter().map(|f| &f.suggestion.title).collect::<Vec<_>>()))
}

fn info<'a>(o: &'a ScanOutcome, b: &str) -> &'a WorktreeInfo {
    by_branch(o, b).worktree.as_ref().expect("worktree facts")
}

#[test]
fn worktree_state_matrix() {
    let fx = Fx::new();
    repo(&fx);
    let o = fx.scan(ScanKind::Deep, &[Box::new(Worktrees)]);
    assert!(o.errors.is_empty(), "{:?}", o.errors);
    // The main worktree is never an item; every linked worktree is its own.
    assert!(o.findings.iter().all(|f| !f.suggestion.title.ends_with(": main")));
    assert_eq!(o.findings.len(), 11);
    assert!(o.findings.iter().all(|f| f.parts.is_empty() && f.suggestion.worktree.is_some()));

    // Ready: clean and idle, whether merged, pushed or not. Nothing is lost.
    for b in ["merged", "pushed", "squash", "unpushed", "worktree-agent-a1b2"] {
        let s = by_branch(&o, b);
        assert_eq!(s.worktree.as_ref().unwrap().status, WtStatus::Ready, "{b}: {}", s.reason);
        assert_eq!(s.risk, Risk::Review, "{b}: never automatic");
        assert!(s.blocked_reason.is_none() && s.recoverable, "{b}");
        assert!(s.reason.starts_with("Idle 30 days"), "{b}: {}", s.reason);
    }
    assert_eq!(info(&o, "merged").merged, WtMerged::Yes);
    assert!(info(&o, "merged").commands.last().unwrap().ends_with("branch -d merged"));
    assert_eq!(info(&o, "merged").reopen, None);
    assert_eq!(info(&o, "pushed").merged, WtMerged::No);
    assert!(info(&o, "pushed").pushed);
    assert!(info(&o, "pushed").reopen.as_deref().unwrap().starts_with("git worktree add .worktrees/pushed pushed"));
    assert!(!info(&o, "pushed").commands.iter().any(|c| c.contains(" branch ")), "the branch stays");
    let un = info(&o, "unpushed");
    assert_eq!((un.merged, un.ahead, un.pushed), (WtMerged::No, 2, false));
    assert!(by_branch(&o, "unpushed").reason.contains("The branch stays"));
    assert_eq!(info(&o, "worktree-agent-a1b2").agent.as_deref(), Some("agent"));
    // Squash merges need Git 2.38's diff behaviour only; checked read-only.
    assert_eq!(info(&o, "squash").merged, WtMerged::Squash, "{}", by_branch(&o, "squash").reason);
    assert!(by_branch(&o, "squash").reason.contains("squash-merged"));
    assert!(info(&o, "squash").commands.last().unwrap().ends_with("branch -D squash"));

    // Needs a look: used today, or locked. Selectable, never automatic.
    let f = info(&o, "fresh");
    assert_eq!((f.status, f.idle_days), (WtStatus::Look, Some(0)));
    assert!(by_branch(&o, "fresh").reason.starts_with("Used today"));
    let l = info(&o, "locked");
    assert_eq!(l.status, WtStatus::Look);
    assert!(l.locked && l.commands[0].contains("worktree unlock"));
    assert!(by_branch(&o, "locked").blocked_reason.is_none());

    // Has your work: never removed.
    for (b, needle, files) in [("dirty", "1 uncommitted file would be deleted", 1), ("untracked", "1 uncommitted file would be deleted", 1), ("merging", "merge", 0), ("broken", "couldn't determine", 0)] {
        let s = by_branch(&o, b);
        assert_eq!(s.worktree.as_ref().unwrap().status, WtStatus::Work, "{b}");
        assert_eq!(s.risk, Risk::Risky, "{b}");
        assert!(s.reason.contains(needle), "{b}: {}", s.reason);
        assert!(s.blocked_reason.is_some() && !s.recoverable, "{b}");
        assert_eq!(s.worktree.as_ref().unwrap().dirty, files, "{b}");
    }
    assert_eq!(info(&o, "untracked").dirty_files, vec!["new.txt".to_string()]);
    assert_eq!(info(&o, "dirty").dirty_files, vec!["d.txt".to_string()]);
}

#[test]
fn inv09_git_work_never_preselected_only_ready_and_look_selectable() {
    let fx = Fx::new();
    repo(&fx);
    let o = fx.scan(ScanKind::Deep, &[Box::new(Worktrees)]);
    for f in &o.findings {
        let s = &f.suggestion;
        assert!(!s.preselected(), "{}", s.title);
        let removable = s.worktree.as_ref().unwrap().status != WtStatus::Work;
        assert_eq!(s.selectable(), removable, "{}", s.title);
        if removable {
            assert!(s.action_label.starts_with("git worktree remove "), "{}", s.title);
            assert!(!s.action_label.contains("--force") && !s.action_label.contains(" -f"));
            assert!(s.worktree.as_ref().unwrap().commands.iter().all(|c| !c.contains("--force")));
        }
    }
}

#[test]
fn process_open_worktree_is_blocked() {
    let fx = Fx::new();
    let app = repo(&fx);
    let target = app.join(".worktrees/pushed");
    // Fake lsof reports a process for this worktree (macOS path); on Linux a
    // real process with its working directory there is used.
    fx.tool("lsof", &format!("printf 'p1\\nfcwd\\nn/\\np4242\\nfcwd\\nn{}\\n'; exit 0", std::fs::canonicalize(&target).unwrap().display()));
    let mut child = std::process::Command::new("sleep").arg("30").current_dir(&target).spawn().unwrap();
    let o = fx.scan(ScanKind::Deep, &[Box::new(Worktrees)]);
    let _ = child.kill();
    let _ = child.wait();
    let w = by_branch(&o, "pushed");
    assert_eq!(w.risk, Risk::Risky);
    assert!(w.reason.contains("running program"), "{}", w.reason);
}

#[test]
fn inv11_unknown_process_state_is_risky() {
    let fx = Fx::new();
    repo(&fx);
    if cfg!(target_os = "linux") {
        return; // /proc is always readable for the user's own processes.
    }
    fx.tool("lsof", "exit 3");
    let o = fx.scan(ScanKind::Deep, &[Box::new(Worktrees)]);
    assert_eq!(by_branch(&o, "merged").risk, Risk::Risky);
}

#[test]
fn probe_detects_new_commit() {
    let fx = Fx::new();
    let app = repo(&fx);
    let o = fx.scan(ScanKind::Deep, &[Box::new(Worktrees)]);
    let f = leaves(&o).into_iter().find(|f| f.suggestion.title.ends_with(": pushed")).unwrap().clone();
    fx.with_ctx(ScanKind::Deep, |ctx| assert_eq!(probe(&f, ctx), Probe::Unchanged));
    let p = app.join(".worktrees/pushed");
    std::fs::write(p.join("late.txt"), "x").unwrap();
    git(&p, &["add", "."]);
    git(&p, &["commit", "-q", "-m", "late"]);
    fx.with_ctx(ScanKind::Deep, |ctx| assert!(matches!(probe(&f, ctx), Probe::Changed(_))));
}

#[test]
fn git_missing_is_unavailable() {
    let mut fx = Fx::new();
    repo(&fx);
    fx.runner = Runner::with_path(&fx.bin);
    let o = fx.scan(ScanKind::Deep, &[Box::new(Worktrees)]);
    assert_eq!(o.errors.len(), 1);
    assert_eq!(o.errors[0].kind, "unavailable");
}

#[test]
fn quick_scan_skips_deep_only_worktrees() {
    let fx = Fx::new();
    repo(&fx);
    assert!(fx.scan(ScanKind::Quick, &[Box::new(Worktrees)]).findings.is_empty());
}

#[test]
fn a_rewritten_reflog_file_is_not_use() {
    // `git gc` rewrites every worktree's reflog file; its date says nothing
    // about use, the time in its last entry does (found on a real Mac).
    let fx = Fx::new();
    let app = repo(&fx);
    let wt = app.join(".worktrees/pushed");
    let gd = git(&wt, &["rev-parse", "--absolute-git-dir"]);
    let log = Path::new(&gd).join("logs/HEAD");
    let text = std::fs::read_to_string(&log).unwrap();
    std::fs::write(&log, text).unwrap(); // now has today's date
    let o = fx.scan(ScanKind::Deep, &[Box::new(Worktrees)]);
    assert_eq!(info(&o, "pushed").status, WtStatus::Ready, "{}", by_branch(&o, "pushed").reason);
    assert_eq!(info(&o, "pushed").idle_days, Some(30));
}
