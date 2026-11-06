use super::*;
use crate::detectors::git::*;
use crate::detectors::{probe, Probe};

/// Builds `projects/app` with a bare `origin` and one linked worktree per
/// source state. Returns the app path.
fn repo(fx: &Fx) -> PathBuf {
    let origin = fx.root.join("origin.git");
    std::fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "--bare", "-q"]);
    let app = fx.mkdir("projects/app");
    git(&app, &["init", "-q"]);
    std::fs::write(app.join("README.md"), "hello\n").unwrap();
    git(&app, &["add", "."]);
    git(&app, &["commit", "-q", "-m", "init"]);
    git(&app, &["remote", "add", "origin", &origin.to_string_lossy()]);
    git(&app, &["push", "-q", "-u", "origin", "main"]);

    let wt = |name: &str| {
        let p = app.join(".worktrees").join(name);
        git(&app, &["worktree", "add", "-q", "-b", name, &p.to_string_lossy()]);
        p
    };
    let commit = |p: &Path, file: &str| {
        std::fs::write(p.join(file), file).unwrap();
        git(p, &["add", "."]);
        git(p, &["commit", "-q", "-m", file]);
    };

    // Clean and merged into the default branch.
    let merged = wt("merged");
    commit(&merged, "m.txt");
    git(&merged, &["push", "-q", "-u", "origin", "merged"]);
    git(&app, &["merge", "-q", "--no-edit", "merged"]);
    git(&app, &["push", "-q", "origin", "main"]);

    // Clean, pushed, not merged.
    let pushed = wt("pushed");
    commit(&pushed, "p.txt");
    git(&pushed, &["push", "-q", "-u", "origin", "pushed"]);

    // Squash-merged: content is in main, but Git sees it as not merged.
    let squash = wt("squash");
    commit(&squash, "s.txt");
    git(&squash, &["push", "-q", "-u", "origin", "squash"]);
    git(&app, &["merge", "-q", "--squash", "squash"]);
    git(&app, &["commit", "-q", "-m", "squash"]);
    git(&app, &["push", "-q", "origin", "main"]);

    // Uncommitted changes.
    let dirty = wt("dirty");
    commit(&dirty, "d.txt");
    git(&dirty, &["push", "-q", "-u", "origin", "dirty"]);
    std::fs::write(dirty.join("d.txt"), "changed").unwrap();

    // Untracked file only.
    let untracked = wt("untracked");
    commit(&untracked, "u.txt");
    git(&untracked, &["push", "-q", "-u", "origin", "untracked"]);
    std::fs::write(untracked.join("new.txt"), "new").unwrap();

    // Unpushed commit.
    let unpushed = wt("unpushed");
    commit(&unpushed, "a.txt");
    git(&unpushed, &["push", "-q", "-u", "origin", "unpushed"]);
    commit(&unpushed, "b.txt");

    // No upstream.
    let noup = wt("noupstream");
    commit(&noup, "n.txt");

    // Locked.
    let locked = wt("locked");
    commit(&locked, "l.txt");
    git(&locked, &["push", "-q", "-u", "origin", "locked"]);
    git(&app, &["worktree", "lock", &locked.to_string_lossy()]);

    // In the middle of a merge.
    let merging = wt("merging");
    commit(&merging, "g.txt");
    git(&merging, &["push", "-q", "-u", "origin", "merging"]);
    let gd = git(&merging, &["rev-parse", "--absolute-git-dir"]);
    std::fs::write(Path::new(&gd).join("MERGE_HEAD"), git(&merging, &["rev-parse", "HEAD"])).unwrap();

    // Status can't be determined: the worktree's link to its repo is broken.
    let broken = wt("broken");
    commit(&broken, "x.txt");
    git(&broken, &["push", "-q", "-u", "origin", "broken"]);
    std::fs::write(broken.join(".git"), "gitdir: /nonexistent/freeboard\n").unwrap();

    app
}

/// Every worktree finding, including those inside the merged
/// "Git worktrees with no local changes" item.
fn leaves(o: &ScanOutcome) -> Vec<&Finding> {
    o.findings.iter().flat_map(|f| if f.parts.is_empty() { vec![f] } else { f.parts.iter().collect() }).collect()
}

fn by_branch<'a>(o: &'a ScanOutcome, b: &str) -> &'a Suggestion {
    leaves(o)
        .into_iter()
        .map(|f| &f.suggestion)
        .find(|s| s.detector == ID && s.title.ends_with(&format!(": {b}")))
        .unwrap_or_else(|| panic!("no worktree {b}: {:#?}", o.findings.iter().map(|f| &f.suggestion.title).collect::<Vec<_>>()))
}

#[test]
fn worktree_state_matrix() {
    let fx = Fx::new();
    repo(&fx);
    let o = fx.scan(ScanKind::Deep, &[Box::new(Worktrees)]);
    assert!(o.errors.is_empty(), "{:?}", o.errors);
    // The main worktree is never suggested.
    assert!(o.findings.iter().all(|f| !f.suggestion.title.ends_with(": main")));
    assert_eq!(leaves(&o).len(), 10);
    // Removable worktrees are one item with a part per worktree.
    let merged: Vec<_> = o.findings.iter().filter(|f| !f.parts.is_empty()).collect();
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].suggestion.title, "Git worktrees with no local changes");
    assert_eq!(merged[0].suggestion.parts.len(), merged[0].parts.len());
    assert!(merged[0].parts.iter().all(|f| f.suggestion.risk == Risk::Review));
    assert!(o.findings.iter().filter(|f| f.parts.is_empty()).all(|f| f.suggestion.blocked_reason.is_some()), "only blocked worktrees stay separate");

    let m = by_branch(&o, "merged");
    assert_eq!(m.risk, Risk::Review);
    assert!(m.reason.contains("merged"));
    let p = by_branch(&o, "pushed");
    assert_eq!(p.risk, Risk::Review);
    assert!(p.reason.contains("pushed but not merged"));
    let s = by_branch(&o, "squash");
    assert_eq!(s.risk, Risk::Review);
    assert!(s.reason.contains("pushed but not merged"), "squash merges are reported conservatively");

    for (b, needle) in [
        ("dirty", "uncommitted"),
        ("untracked", "untracked"),
        ("unpushed", "unpushed"),
        ("noupstream", "no upstream"),
        ("locked", "locked"),
        ("merging", "merge"),
        ("broken", "couldn't determine"),
    ] {
        let w = by_branch(&o, b);
        assert_eq!(w.risk, Risk::Risky, "{b}");
        assert!(w.reason.contains(needle), "{b}: {}", w.reason);
        assert!(w.blocked_reason.is_some(), "{b} must be blocked");
        assert!(!w.recoverable);
    }
}

#[test]
fn inv09_git_work_never_preselected_only_clean_review_selectable() {
    let fx = Fx::new();
    repo(&fx);
    let o = fx.scan(ScanKind::Deep, &[Box::new(Worktrees)]);
    for f in o.findings.iter().chain(leaves(&o)) {
        assert!(!f.suggestion.preselected(), "{}", f.suggestion.title);
        let clean = f.suggestion.risk == Risk::Review;
        assert_eq!(f.suggestion.selectable(), clean, "{}", f.suggestion.title);
    }
    for b in ["merged", "pushed", "squash"] {
        let w = by_branch(&o, b);
        assert_eq!(w.blocked_reason, None, "{b}");
        assert!(w.action_label.starts_with("git worktree remove "), "{b}");
        assert!(!w.action_label.contains("--force") && !w.action_label.contains(" -f"));
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
