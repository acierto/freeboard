//! Cleanup engine tests. Fixture-only: temporary directories, a fake `docker`
//! and a directory-backed Trash. No real cleanup ever runs.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use super::trash::DirTrash;
use super::*;
use crate::detectors::tests::{age, git, node_project, Fx, NOW};
use crate::model::MemorySink;
use crate::scan::run_scan;

struct Env {
    fx: Fx,
    store: Arc<Store>,
    engine: Arc<CleanupEngine>,
    events: Arc<MemorySink>,
    trash_dir: PathBuf,
}

fn env_with(fx: Fx, trash_available: bool) -> Env {
    let store = Arc::new(Store::open_in_memory().unwrap());
    let guard = Guard::for_system(&fx.home, &[]);
    let events = Arc::new(MemorySink::default());
    let home = fx.home.clone();
    // Free space falls as the fixture home grows; the Trash lives inside the
    // home so moving to it frees nothing, like a real same-volume Trash.
    let free: FreeSpaceFn = Arc::new(move || 1_000_000_000_000u64 - measure(&home, &MeasureOpts::default()).bytes);
    let trash_dir = fx.home.join(".Trash-fixture");
    std::fs::create_dir_all(&trash_dir).unwrap();
    let settings = fx.settings.clone();
    let engine = CleanupEngine::new(store.clone(), guard, fx.runner.clone(), events.clone(), free)
        .with_context(fx.home.clone(), Arc::new(move || settings.clone()))
        .with_trash(Arc::new(DirTrash { dir: trash_dir.clone(), available: trash_available }))
        .with_platform(fx.platform);
    Env { fx, store, engine: Arc::new(engine), events, trash_dir }
}

/// Docker fixture, a Node project, a .venv project and a repository.
fn world() -> Fx {
    let fx = Fx::new();
    fx.docker(true);
    node_project(&fx, "projects/web", 60);
    fx.write("projects/data-tools/requirements.txt", b"requests");
    fx.write("projects/data-tools/.venv/lib/site.py", &vec![1u8; 20_000]);
    let app = fx.mkdir("projects/repo");
    git(&app, &["init", "-q"]);
    std::fs::write(app.join("a.txt"), "a").unwrap();
    git(&app, &["add", "."]);
    git(&app, &["commit", "-q", "-m", "a"]);
    git(&app, &["worktree", "add", "-q", "-b", "feature", &app.join(".worktrees/feature").to_string_lossy()]);
    // A clean worktree pushed to a local bare origin (Review, removable).
    let origin = fx.root.join("origin.git");
    std::fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "--bare", "-q"]);
    git(&app, &["remote", "add", "origin", &origin.to_string_lossy()]);
    git(&app, &["push", "-q", "-u", "origin", "HEAD:main"]);
    // "feature" gets a commit that exists nowhere else (no upstream): Risky.
    let feature = app.join(".worktrees/feature");
    std::fs::write(feature.join("f.txt"), "f").unwrap();
    git(&feature, &["add", "."]);
    git(&feature, &["commit", "-q", "-m", "f"]);
    let done = app.join(".worktrees/done");
    git(&app, &["worktree", "add", "-q", "-b", "done", &done.to_string_lossy()]);
    std::fs::write(done.join("d.txt"), "d").unwrap();
    git(&done, &["add", "."]);
    git(&done, &["commit", "-q", "-m", "d"]);
    git(&done, &["push", "-q", "-u", "origin", "done"]);
    age(&fx.home.join("projects"), 60);
    fx
}

impl Env {
    fn new() -> Env {
        env_with(world(), true)
    }
    fn scan(&self) -> Vec<Finding> {
        let o = self.fx.with_ctx(ScanKind::Deep, |ctx| run_scan(ctx, &crate::detectors::all()));
        self.store.replace_findings("scan", &o.findings).unwrap();
        o.findings
    }
    fn id(&self, det: &str, title: &str) -> String {
        self.store
            .findings()
            .unwrap()
            .into_iter()
            .find(|f| f.suggestion.detector == det && f.suggestion.title.contains(title))
            .unwrap_or_else(|| panic!("no {det} {title}"))
            .suggestion
            .id
    }
    fn nm(&self) -> String {
        self.id("project.dependencies", "node_modules in web")
    }
    fn venv(&self) -> String {
        self.id("project.dependencies", ".venv")
    }
    fn image(&self) -> String {
        self.id("docker.unused_images", "postgres")
    }
    fn volume(&self) -> String {
        self.id("docker.volumes", "billing_pgdata")
    }
    fn actions(&self) -> String {
        std::fs::read_to_string(self.fx.root.join("docker-state/actions")).unwrap_or_default()
    }
    fn state(&self, f: &str) -> PathBuf {
        self.fx.root.join("docker-state").join(f)
    }
    fn plan(&self, ids: &[String]) -> CleanupPlan {
        self.engine.create_plan(ids, NOW).unwrap()
    }
    fn run(&self, plan: &CleanupPlan) -> RunDetail {
        let id = self.engine.run_plan_blocking(&plan.id, NOW + 60).unwrap();
        self.engine.get_run(&id).unwrap()
    }
    fn item<'a>(&self, r: &'a RunDetail, id: &str) -> &'a RunItem {
        r.items.iter().find(|i| i.item_id == id).unwrap()
    }
}

fn nm_path(e: &Env) -> PathBuf {
    e.fx.home.join("projects/web/node_modules")
}

// ----- plans -------------------------------------------------------------

#[test]
fn inv02_nothing_runs_without_a_saved_plan() {
    let e = Env::new();
    e.scan();
    assert_eq!(e.engine.run_plan_blocking("plan_unknown", NOW), Err(CleanupError::UnknownPlan));
    assert!(nm_path(&e).exists());
}

#[test]
fn inv02_altered_plan_is_rejected() {
    let e = Env::new();
    e.scan();
    let p = e.plan(&[e.nm()]);
    e.store.conn().execute("UPDATE cleanup_plans SET snapshot = snapshot || ' ' WHERE id = ?1", [&p.id]).unwrap();
    assert_eq!(e.engine.run_plan_blocking(&p.id, NOW), Err(CleanupError::Altered));
    assert!(nm_path(&e).exists());
}

#[test]
fn plan_expires_after_thirty_minutes() {
    let e = Env::new();
    e.scan();
    let p = e.plan(&[e.nm()]);
    assert_eq!(p.state, PlanState::Open);
    assert_eq!(e.engine.run_plan_blocking(&p.id, NOW + PLAN_TTL_SECS), Err(CleanupError::Expired));
    assert_eq!(e.engine.get_plan(&p.id, NOW).unwrap().state, PlanState::Expired);
    assert!(nm_path(&e).exists());
}

#[test]
fn new_scan_invalidates_plan() {
    let e = Env::new();
    e.scan();
    let p = e.plan(&[e.nm()]);
    e.scan();
    assert_eq!(e.engine.run_plan_blocking(&p.id, NOW), Err(CleanupError::Invalidated));
    assert_eq!(e.engine.get_plan(&p.id, NOW).unwrap().state, PlanState::Invalidated);
    assert!(nm_path(&e).exists());
}

#[test]
fn inv03_unknown_ids_and_paths_are_rejected() {
    let e = Env::new();
    e.scan();
    for bad in ["/etc/passwd", "../../", "docker system prune", ""] {
        assert!(matches!(e.engine.create_plan(&[bad.to_string()], NOW), Err(CleanupError::UnknownItem(_))), "{bad}");
    }
    assert_eq!(e.engine.create_plan(&[], NOW), Err(CleanupError::Empty));
    let p = e.plan(&[e.nm()]);
    assert!(matches!(e.engine.acknowledge_item(&p.id, "/etc/passwd", NOW), Err(CleanupError::UnknownItem(_))));
}

#[test]
fn inv09_unpushed_worktree_cannot_be_planned() {
    let e = Env::new();
    e.scan();
    // "feature" has no upstream: Risky and blocked.
    let wt = e.id("git.worktrees", "feature");
    assert!(matches!(e.engine.create_plan(&[wt], NOW), Err(CleanupError::Blocked(_))));
}

fn repo_path(e: &Env) -> PathBuf {
    std::fs::canonicalize(e.fx.home.join("projects/repo")).unwrap()
}

#[test]
fn clean_pushed_worktree_is_removed_without_force() {
    let e = Env::new();
    e.scan();
    let wt = e.id("git.worktrees", "done");
    let p = e.plan(&[wt.clone()]);
    let r = e.run(&p);
    assert_eq!(e.item(&r, &wt).state, ItemState::Done, "{:?}", e.item(&r, &wt).reason);
    let repo = repo_path(&e);
    assert!(!repo.join(".worktrees/done").exists());
    // The branch, the main worktree and the other worktree are untouched.
    assert!(repo.join(".git").is_dir());
    assert!(repo.join("a.txt").exists());
    assert!(repo.join(".worktrees/feature").exists());
    let branches = std::process::Command::new("git").args(["branch", "--list", "done"]).current_dir(&repo).output().unwrap();
    assert!(String::from_utf8_lossy(&branches.stdout).contains("done"), "branch kept for recovery");
}

#[test]
fn inv04_worktree_with_new_commit_after_scan_is_skipped() {
    let e = Env::new();
    e.scan();
    let wt = e.id("git.worktrees", "done");
    let p = e.plan(&[wt.clone()]);
    let done = repo_path(&e).join(".worktrees/done");
    std::fs::write(done.join("late.txt"), "late").unwrap();
    git(&done, &["add", "."]);
    git(&done, &["commit", "-q", "-m", "late"]);
    let r = e.run(&p);
    assert_eq!(e.item(&r, &wt).state, ItemState::Skipped);
    assert!(done.join("late.txt").exists());
}

#[test]
fn inv04_worktree_edited_after_scan_is_skipped() {
    let e = Env::new();
    e.scan();
    let wt = e.id("git.worktrees", "done");
    let p = e.plan(&[wt.clone()]);
    let done = repo_path(&e).join(".worktrees/done");
    std::fs::write(done.join("d.txt"), "edited").unwrap();
    let r = e.run(&p);
    assert_eq!(e.item(&r, &wt).state, ItemState::Skipped);
    assert_eq!(std::fs::read_to_string(done.join("d.txt")).unwrap(), "edited");
}

#[test]
fn inv06_worktree_swapped_for_symlink_is_skipped() {
    let e = Env::new();
    e.scan();
    let wt = e.id("git.worktrees", "done");
    let p = e.plan(&[wt.clone()]);
    let done = repo_path(&e).join(".worktrees/done");
    let moved = e.fx.root.join("moved-wt");
    std::fs::rename(&done, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &done).unwrap();
    let r = e.run(&p);
    assert_ne!(e.item(&r, &wt).state, ItemState::Done);
    assert!(moved.join("d.txt").exists(), "link target untouched");
}

#[test]
fn totals_acknowledgment_and_events() {
    let e = Env::new();
    e.scan();
    let (nm, venv, vol) = (e.nm(), e.venv(), e.volume());
    let p = e.plan(&[nm.clone(), venv.clone(), vol.clone()]);
    let size = |id: &str| p.items.iter().find(|s| s.id == id).unwrap().size_bytes;
    assert_eq!(p.bytes_freed_now, size(&nm) + size(&vol));
    assert_eq!(p.bytes_held_in_trash, size(&venv));
    assert_eq!(p.unrecoverable_ids, vec![vol.clone()]);
    assert_eq!(p.unrecoverable_bytes, 7_600_000_000);
    assert_eq!(p.needs_ack_ids, vec![vol.clone()]);
    assert!(!p.ready);
    assert_eq!(e.engine.run_plan_blocking(&p.id, NOW), Err(CleanupError::NeedsAcknowledgment));
    assert!(e.actions().is_empty());
    let p = e.engine.acknowledge_item(&p.id, &vol, NOW).unwrap();
    assert!(p.ready);
    let r = e.run(&p);
    assert_eq!(r.state, "finished");
    assert_eq!(e.item(&r, &nm).state, ItemState::Done);
    assert_eq!(e.item(&r, &venv).state, ItemState::MovedToTrash);
    assert_eq!(e.item(&r, &vol).state, ItemState::Done);
    assert!(!nm_path(&e).exists());
    assert!(e.fx.home.join("projects/web/package.json").exists());
    assert!(e.actions().contains("volume rm [billing_pgdata]"));
    assert!(r.trash_bytes > 0);
    // Cleaned items leave Suggestions; held Trash is reported.
    let left: Vec<String> = e.store.findings().unwrap().into_iter().map(|f| f.suggestion.id).collect();
    assert!(!left.contains(&nm) && !left.contains(&venv));
    assert_eq!(e.engine.trash_summary().bytes, r.trash_bytes);
    // Events.
    let upd = e.events.events(EV_CLEANUP_ITEM_UPDATED);
    assert!(upd.iter().any(|v| v["itemId"] == nm.as_str() && v["state"] == "done"));
    assert!(upd.iter().any(|v| v["state"] == "checking"));
    let fin = e.events.events(EV_CLEANUP_FINISHED);
    assert_eq!(fin.len(), 1);
    assert!(fin[0]["measuredBytesFreed"].as_i64().unwrap() > 0);
    // A plan runs once.
    assert_eq!(e.engine.run_plan_blocking(&p.id, NOW), Err(CleanupError::AlreadyUsed));
}

#[test]
fn inv10_volume_requires_individual_ack() {
    let e = Env::new();
    e.scan();
    let (vol, img) = (e.volume(), e.image());
    let p = e.plan(&[img.clone(), vol.clone()]);
    // Acknowledging a different item does not count.
    assert!(matches!(e.engine.acknowledge_item(&p.id, &img, NOW), Err(CleanupError::UnknownItem(_))));
    assert_eq!(e.engine.run_plan_blocking(&p.id, NOW), Err(CleanupError::NeedsAcknowledgment));
    // Acknowledgments are scoped to the plan.
    let other = e.plan(&[vol.clone()]);
    e.engine.acknowledge_item(&other.id, &vol, NOW).unwrap();
    assert_eq!(e.engine.run_plan_blocking(&p.id, NOW), Err(CleanupError::NeedsAcknowledgment));
    assert!(e.actions().is_empty());
}

// ----- revalidation ----------------------------------------------------------

#[test]
fn inv04_modified_file_is_skipped_and_explained() {
    let e = Env::new();
    e.scan();
    let nm = e.nm();
    let p = e.plan(&[nm.clone()]);
    std::fs::write(nm_path(&e).join("dep/index.js"), b"edited after the scan").unwrap();
    let r = e.run(&p);
    let it = e.item(&r, &nm);
    assert_eq!(it.state, ItemState::Skipped);
    assert!(it.reason.as_deref().unwrap().contains("changed"));
    assert!(nm_path(&e).join("dep/index.js").exists());
    // Still listed in Suggestions with the reason.
    let f = e.store.finding(&nm).unwrap().unwrap();
    assert_eq!(f.suggestion.skip_reason, it.reason);
    assert_eq!(r.skipped, 1);
}

#[test]
fn inv04_symlink_swap_is_skipped() {
    let e = Env::new();
    e.scan();
    let nm = e.nm();
    let p = e.plan(&[nm.clone()]);
    // Replace the project with a link to another tree that also has node_modules.
    let other = e.fx.root.join("other");
    node_project(&e.fx, "../other", 60);
    std::fs::rename(e.fx.home.join("projects/web"), e.fx.root.join("web-moved")).unwrap();
    std::os::unix::fs::symlink(&other, e.fx.home.join("projects/web")).unwrap();
    let r = e.run(&p);
    assert_eq!(e.item(&r, &nm).state, ItemState::Skipped);
    assert!(other.join("node_modules/dep/index.js").exists());
    assert!(e.fx.root.join("web-moved/node_modules/dep/index.js").exists());
}

#[test]
fn inv04_container_starts_using_image_or_volume() {
    let e = Env::new();
    e.scan();
    let (img, vol) = (e.image(), e.volume());
    let p = e.plan(&[img.clone(), vol.clone()]);
    e.engine.acknowledge_item(&p.id, &vol, NOW).unwrap();
    let c = std::fs::read_to_string(e.state("containers.json")).unwrap().replacen(
        "[\n",
        "[\n  {\"Id\":\"c9\",\"Image\":\"sha256:ccc3\",\"State\":{\"Status\":\"running\",\"Running\":true},\"Mounts\":[{\"Type\":\"volume\",\"Name\":\"billing_pgdata\"}]},\n",
        1,
    );
    std::fs::write(e.state("containers.json"), c).unwrap();
    let r = e.run(&p);
    assert_eq!(e.item(&r, &img).state, ItemState::Skipped);
    assert_eq!(e.item(&r, &vol).state, ItemState::Skipped);
    assert!(e.actions().is_empty(), "{}", e.actions());
}

#[test]
fn inv04_removed_manifest_is_skipped() {
    let e = Env::new();
    e.scan();
    let nm = e.nm();
    let p = e.plan(&[nm.clone()]);
    std::fs::remove_file(e.fx.home.join("projects/web/package-lock.json")).unwrap();
    let r = e.run(&p);
    assert_eq!(e.item(&r, &nm).state, ItemState::Skipped);
    assert!(nm_path(&e).exists());
}

#[test]
fn inv11_unknown_status_skips() {
    let e = Env::new();
    e.scan();
    let img = e.image();
    let p = e.plan(&[img.clone()]);
    std::fs::write(e.state("down"), "").unwrap();
    let r = e.run(&p);
    let it = e.item(&r, &img);
    assert_eq!(it.state, ItemState::Skipped);
    assert!(it.reason.as_deref().unwrap().contains("could not be checked"));
    assert!(e.actions().is_empty());
}

// ----- guarded execution -----------------------------------------------------

fn inject(e: &Env, f: Finding) -> String {
    let mut all = e.store.findings().unwrap();
    let id = f.suggestion.id.clone();
    all.push(f);
    e.store.replace_findings("scan", &all).unwrap();
    id
}

fn template(e: &Env) -> Finding {
    e.store.finding(&e.nm()).unwrap().unwrap()
}

#[test]
fn inv05_protected_paths_are_refused_at_execution() {
    let e = Env::new();
    e.scan();
    let repo = e.fx.home.join("projects/repo");
    let mut ids = vec![];
    for (n, p) in [("home", e.fx.home.clone()), ("repo", repo.clone()), ("git", repo.join(".git"))] {
        let mut f = template(&e);
        f.suggestion.id = format!("bad-{n}");
        f.target = Target::Paths { paths: vec![p.to_string_lossy().to_string()] };
        ids.push(inject(&e, f));
    }
    let p = e.plan(&ids);
    let r = e.run(&p);
    for i in &r.items {
        assert_eq!(i.state, ItemState::Skipped, "{i:?}");
        assert!(i.reason.as_deref().unwrap().contains("protected"), "{i:?}");
    }
    assert!(repo.join(".git").exists() && repo.join("a.txt").exists());
}

#[test]
fn inv06_delete_removes_links_not_targets() {
    let e = env_with(
        {
            let fx = world();
            let precious = fx.root.join("precious");
            std::fs::create_dir_all(&precious).unwrap();
            std::fs::write(precious.join("data"), "keep").unwrap();
            std::os::unix::fs::symlink(&precious, fx.home.join("projects/web/node_modules/escape")).unwrap();
            fx
        },
        true,
    );
    e.scan();
    let p = e.plan(&[e.nm()]);
    let r = e.run(&p);
    assert_eq!(r.items[0].state, ItemState::Done);
    assert!(!nm_path(&e).exists());
    assert_eq!(std::fs::read_to_string(e.fx.root.join("precious/data")).unwrap(), "keep");
}

#[test]
fn inv07_banned_and_forced_commands_are_refused() {
    let e = Env::new();
    e.scan();
    let bc = e.store.findings().unwrap().into_iter().find(|f| f.suggestion.detector == "docker.build_cache").unwrap();
    let mut ids = vec![];
    for (n, args) in [
        ("system-prune", vec!["system", "prune", "--all", "--force"]),
        ("volume-prune", vec!["volume", "prune"]),
        ("forced-rm", vec!["image", "rm", "--force", "sha256:ccc3"]),
    ] {
        let mut f = bc.clone();
        f.suggestion.id = format!("bad-{n}");
        let Target::Native { resources, .. } = &bc.target else { panic!() };
        f.target = Target::Native { program: "docker".into(), args: args.iter().map(|s| s.to_string()).collect(), resources: resources.clone(), cwd: None };
        ids.push(inject(&e, f));
    }
    let p = e.plan(&ids);
    let r = e.run(&p);
    for i in &r.items {
        assert_eq!(i.state, ItemState::Failed, "{i:?}");
        assert!(i.reason.as_deref().unwrap().contains("refused"), "{i:?}");
    }
    assert!(e.actions().is_empty(), "{}", e.actions());
}

#[test]
fn inv08_arguments_are_passed_without_a_shell() {
    let e = env_with(world(), true);
    let evil = "evil;$(touch PWNED)`id`";
    let vols = e.state("volumes.jsonl");
    let mut s = std::fs::read_to_string(&vols).unwrap();
    s.push_str(&format!("{{\"Name\":\"{evil}\"}}\n"));
    std::fs::write(&vols, s).unwrap();
    e.scan();
    let id = e.id("docker.volumes", "evil");
    let p = e.plan(&[id.clone()]);
    e.engine.acknowledge_item(&p.id, &id, NOW).unwrap();
    let r = e.run(&p);
    assert_eq!(r.items[0].state, ItemState::Done);
    assert!(e.actions().contains(&format!("volume rm [{evil}]")));
    assert!(!e.fx.root.join("docker-state/PWNED").exists() && !Path::new("PWNED").exists());
}

#[test]
fn aggregate_commands_stay_within_the_plan() {
    let e = Env::new();
    e.scan();
    let dang = e.id("docker.dangling_images", "Untagged");
    let bc = e.id("docker.build_cache", "build cache");
    let p = e.plan(&[dang.clone(), bc.clone()]);
    // A new dangling image appears after the plan was saved.
    let mut imgs = std::fs::read_to_string(e.state("images.json")).unwrap();
    imgs = imgs.replacen("[\n", "[\n  {\"Id\":\"sha256:dd9\",\"RepoTags\":[],\"Created\":\"2026-11-03T00:00:00Z\",\"Size\":5000000000},\n", 1);
    std::fs::write(e.state("images.json"), imgs).unwrap();
    let r = e.run(&p);
    assert_eq!(e.item(&r, &dang).state, ItemState::Done);
    assert_eq!(e.item(&r, &bc).state, ItemState::Done);
    let a = e.actions();
    assert!(a.contains("image rm sha256:dd1 sha256:dd2\n"), "{a}");
    assert!(!a.contains("dd9"));
    assert!(a.contains("builder prune --force --filter until="), "{a}");
    assert!(!a.contains("{scan_age}"));
}

// ----- runs ------------------------------------------------------------------

#[test]
fn cancel_takes_effect_between_items_and_single_run_lock() {
    let e = Env::new();
    e.scan();
    let (img, nm) = (e.image(), e.nm());
    let p = e.plan(&[img.clone(), nm.clone()]);
    let other = e.plan(&[e.venv()]);
    std::fs::write(e.state("rm_sleep"), "1").unwrap();
    let run = e.engine.run_plan(&p.id, NOW).unwrap();
    // Wait until the first item is executing.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while e.engine.get_run(&run).unwrap().items[0].state != ItemState::Running {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(e.engine.run_plan_blocking(&other.id, NOW), Err(CleanupError::Busy));
    assert!(e.engine.cancel_run(&run));
    let r = e.engine.wait_run(&run, Duration::from_secs(20)).unwrap();
    assert_eq!(r.state, "canceled");
    assert_eq!(e.item(&r, &img).state, ItemState::Done, "the running item completes");
    assert_eq!(e.item(&r, &nm).state, ItemState::Canceled);
    assert!(nm_path(&e).exists());
    assert!(!e.engine.cancel_run("run_unknown"));
    // The lock is released afterwards.
    let again = e.plan(&[e.venv()]);
    assert!(e.engine.run_plan_blocking(&again.id, NOW).is_ok());
}

#[test]
fn interrupted_run_is_recorded_and_never_replayed() {
    let e = Env::new();
    e.scan();
    let (img, nm) = (e.image(), e.nm());
    let p = e.plan(&[img.clone(), nm.clone()]);
    // Simulate a crash: a run row left "running" with one item in flight.
    let snap = e.store.findings().unwrap();
    let items: Vec<(Suggestion, String)> =
        snap.iter().filter(|f| f.suggestion.id == img || f.suggestion.id == nm).map(|f| (f.suggestion.clone(), "x".into())).collect();
    persist::insert_run(&mut e.store.conn(), "run_crashed", &p.id, "{}", NOW, 1, 1, true, &items).unwrap();
    persist::set_item(&e.store.conn(), "run_crashed", &img, ItemState::Running, None, NOW).unwrap();
    let fresh = e.plan(&[e.venv()]);
    assert_eq!(e.engine.run_plan_blocking(&fresh.id, NOW), Err(CleanupError::Busy));
    e.engine.recover_interrupted();
    let r = e.engine.get_run("run_crashed").unwrap();
    assert_eq!(r.state, "interrupted");
    assert_eq!(e.item(&r, &img).state, ItemState::Interrupted);
    assert_eq!(e.item(&r, &nm).state, ItemState::Canceled);
    assert!(e.actions().is_empty(), "nothing replayed");
    assert!(nm_path(&e).exists());
    assert_eq!(e.engine.run_plan_blocking(&p.id, NOW), Err(CleanupError::AlreadyUsed));
    assert!(e.engine.run_plan_blocking(&fresh.id, NOW).is_ok());
}

#[test]
fn inv13_failures_are_independent_and_recorded() {
    let e = Env::new();
    e.scan();
    let (img, nm) = (e.image(), e.nm());
    let p = e.plan(&[img.clone(), nm.clone()]);
    std::fs::write(e.state("fail_rm"), "").unwrap();
    let r = e.run(&p);
    assert_eq!(r.state, "finished");
    assert_eq!(e.item(&r, &img).state, ItemState::Failed);
    assert!(e.item(&r, &img).reason.as_deref().unwrap().contains("conflict"));
    assert_eq!(e.item(&r, &nm).state, ItemState::Done);
    assert_eq!(r.failed, 1);
    assert!(r.free_after.is_some());
    assert!(r.measured_bytes_freed.unwrap() > 0);
    assert_eq!(r.expected_bytes, p.bytes_freed_now);
    // Durable record: plan, item snapshots, before/after.
    let (plan_json, before, after): (String, i64, Option<i64>) = e
        .store
        .conn()
        .query_row("SELECT plan_json, free_before, free_after FROM cleanup_runs WHERE id = ?1", [&r.id], |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?)))
        .unwrap();
    assert!(plan_json.contains(&img) && before > 0 && after.is_some());
    let h = e.engine.list_history(0).unwrap();
    assert_eq!(h.runs.len(), 1);
    assert_eq!(h.runs[0].items, 2);
    assert_eq!(h.runs[0].measured_bytes_freed, r.measured_bytes_freed);
    assert!(!h.has_more);
}

#[test]
fn history_shows_mixed_results_and_paginates() {
    let e = Env::new();
    e.scan();
    let (nm, img) = (e.nm(), e.image());
    let p = e.plan(&[nm.clone(), img.clone()]);
    std::fs::write(nm_path(&e).join("dep/new.js"), b"changed").unwrap();
    let r = e.run(&p);
    assert_eq!(r.skipped, 1);
    let h = e.engine.list_history(0).unwrap();
    assert_eq!(h.runs[0].skipped, 1);
    // Fill more than one page.
    for i in 0..HISTORY_PAGE_SIZE {
        e.store
            .conn()
            .execute(
                "INSERT INTO cleanup_runs(id, plan_id, plan_json, started_at, state, volume, free_before, free_after, expected_bytes) VALUES (?1,'p','{}',?2,'finished','home',1,1,0)",
                rusqlite::params![format!("old{i}"), NOW - 1000 - i as i64],
            )
            .unwrap();
    }
    let p0 = e.engine.list_history(0).unwrap();
    assert_eq!(p0.runs.len(), HISTORY_PAGE_SIZE as usize);
    assert!(p0.has_more);
    let p1 = e.engine.list_history(1).unwrap();
    assert_eq!(p1.runs.len(), 1);
    assert!(!p1.has_more);
    assert_eq!(e.engine.get_run("nope"), Err(CleanupError::UnknownRun));
}

#[test]
fn docker_desktop_reports_measured_not_expected() {
    let e = Env::new();
    e.scan();
    let bc = e.id("docker.build_cache", "build cache");
    let p = e.plan(&[bc]);
    let r = e.run(&p);
    assert_eq!(r.items[0].state, ItemState::Done);
    assert!(r.involves_docker);
    assert_eq!(r.expected_bytes, 14_800_000_000);
    assert_eq!(r.measured_bytes_freed, Some(0), "space stays in Docker's virtual disk in this fixture");
}

// ----- Trash -----------------------------------------------------------------

#[test]
fn scoped_trash_empties_only_freeboard_items() {
    let e = Env::new();
    e.scan();
    let venv = e.venv();
    let p = e.plan(&[venv.clone()]);
    let r = e.run(&p);
    assert_eq!(r.items[0].state, ItemState::MovedToTrash);
    assert!(!e.fx.home.join("projects/data-tools/.venv").exists());
    std::fs::write(e.trash_dir.join("unrelated.txt"), "user's own trash").unwrap();
    let t = e.engine.trash_summary();
    assert_eq!(t.items, 1);
    assert!(t.bytes > 0);
    assert_eq!(e.engine.empty_freeboard_trash("wrong"), Err(CleanupError::BadToken));
    let res = e.engine.empty_freeboard_trash(t.confirmation_token.as_deref().unwrap()).unwrap();
    assert_eq!((res.emptied, res.failed), (1, 0));
    assert!(e.trash_dir.join("unrelated.txt").exists(), "unrelated Trash content is untouched");
    assert!(!e.trash_dir.join(".venv").exists());
    assert_eq!(e.engine.trash_summary().items, 0);
    // The token is single-use for that set.
    assert_eq!(e.engine.empty_freeboard_trash(t.confirmation_token.as_deref().unwrap()), Err(CleanupError::BadToken));
}

#[test]
fn trash_item_removed_outside_freeboard_is_gone() {
    let e = Env::new();
    e.scan();
    let p = e.plan(&[e.venv()]);
    e.run(&p);
    std::fs::remove_dir_all(e.trash_dir.join(".venv")).unwrap();
    std::fs::write(e.trash_dir.join(".venv"), "a different item with the same name").unwrap();
    let t = e.engine.trash_summary();
    let res = e.engine.empty_freeboard_trash(t.confirmation_token.as_deref().unwrap()).unwrap();
    assert_eq!(res.emptied, 0);
    assert!(e.trash_dir.join(".venv").exists(), "matched by identity, not name");
    assert_eq!(e.engine.trash_summary().items, 0);
}

#[test]
fn unavailable_trash_needs_a_separate_permanent_plan() {
    let e = env_with(world(), false);
    e.scan();
    let (venv, nm) = (e.venv(), e.nm());
    let p = e.plan(&[venv.clone()]);
    let r = e.run(&p);
    let it = &r.items[0];
    assert_eq!(it.state, ItemState::Failed);
    assert!(it.reason.as_deref().unwrap().starts_with(TRASH_UNAVAILABLE));
    let venv_path = e.fx.home.join("projects/data-tools/.venv");
    assert!(venv_path.exists(), "nothing deleted as a fallback");
    assert!(matches!(e.engine.create_permanent_plan(&[nm], NOW), Err(CleanupError::NotEligible(_))));
    let pp = e.engine.create_permanent_plan(&[venv.clone()], NOW).unwrap();
    assert_eq!(pp.unrecoverable_ids, vec![venv.clone()]);
    assert_eq!(pp.needs_ack_ids, vec![venv.clone()]);
    assert_eq!(pp.bytes_held_in_trash, 0);
    assert_eq!(e.engine.run_plan_blocking(&pp.id, NOW), Err(CleanupError::NeedsAcknowledgment));
    e.engine.acknowledge_item(&pp.id, &venv, NOW).unwrap();
    let r = e.run(&pp);
    assert_eq!(r.items[0].state, ItemState::Done);
    assert!(!venv_path.exists());
}

/// Uses the real platform Trash with a dedicated disposable file. Run
/// manually: `cargo test --no-default-features -- --ignored native_trash`.
#[test]
#[ignore]
fn native_trash_round_trip_with_disposable_file() {
    // Two disposable files: Freeboard trashes and records one; the other is
    // trashed outside Freeboard and must survive emptying.
    let t = tempfile::tempdir_in(crate::platform::home_dir().unwrap()).unwrap();
    let pid = std::process::id();
    let mine = t.path().join(format!("freeboard-disposable-{pid}.txt"));
    let other = t.path().join(format!("freeboard-unrelated-{pid}.txt"));
    std::fs::write(&mine, "disposable").unwrap();
    std::fs::write(&other, "unrelated").unwrap();
    let guard = Guard::for_system(&crate::platform::home_dir().unwrap(), &[]);
    let checked = guard.check(&mine).unwrap();
    let identity = trash::trash_identity(&checked.canonical);
    let backend = trash::NativeTrash;
    backend.trash(&guard, &checked).unwrap();
    assert!(!mine.exists());
    ::trash::delete(&other).unwrap();
    let rec = trash::TrashRecord {
        id: 1,
        run_id: "r".into(),
        item_id: "i".into(),
        original_path: checked.canonical.to_string_lossy().to_string(),
        trashed_at: now_secs(),
        bytes: 0,
        identity,
    };
    let first = backend.purge(&guard, &rec);
    #[cfg(target_os = "macos")]
    if first == Err(trash::TRASH_NOT_READABLE.to_string()) {
        // Without Full Disk Access macOS hides ~/.Trash. Freeboard must say
        // so (never report the item as already gone). The two disposable
        // files stay in the Trash until the user empties it.
        eprintln!("~/.Trash is not readable without Full Disk Access; verified the explicit error path");
        return;
    }
    assert_eq!(first, Ok(trash::PurgeOutcome::Purged));
    // Purging again finds nothing: the item is gone, not re-matched.
    assert_eq!(backend.purge(&guard, &rec).unwrap(), trash::PurgeOutcome::Gone);
    // The unrelated item is still in the Trash; restore-free cleanup of the
    // test leftovers goes through the platform API, matching the exact path.
    #[cfg(not(target_os = "macos"))]
    {
        let items = ::trash::os_limited::list().unwrap();
        let left: Vec<_> = items.into_iter().filter(|i| i.original_path() == other).collect();
        assert_eq!(left.len(), 1, "unrelated Trash item must survive");
        ::trash::os_limited::purge_all(left).unwrap();
    }
    #[cfg(target_os = "macos")]
    {
        let home = crate::platform::home_dir().unwrap();
        let survivor = std::fs::read_dir(home.join(".Trash"))
            .unwrap()
            .flatten()
            .find(|e| e.file_name().to_string_lossy().starts_with(&format!("freeboard-unrelated-{pid}")))
            .expect("unrelated Trash item must survive");
        std::fs::remove_file(survivor.path()).unwrap();
    }
}
