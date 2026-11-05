use super::*;
use crate::detectors::caches::*;
use crate::detectors::{probe, Probe};

#[test]
fn pnpm_reports_only_unreferenced_packages() {
    let fx = Fx::new();
    let store = fx.mkdir("pnpm-store/v10/files/00");
    std::fs::write(store.join("unreferenced"), vec![1u8; 300_000]).unwrap();
    std::fs::write(store.join("linked"), vec![2u8; 300_000]).unwrap();
    let proj = fx.mkdir("projects/a/node_modules");
    std::fs::hard_link(store.join("linked"), proj.join("dep.js")).unwrap();
    fx.tool("pnpm", &format!("echo '{}'", fx.home.join("pnpm-store/v10").display()));
    let o = fx.scan(ScanKind::Quick, &[Box::new(Pnpm)]);
    let f = find(&o, PNPM);
    let s = &f.suggestion;
    assert!(s.size_bytes >= 300_000 && s.size_bytes < 500_000, "{}", s.size_bytes);
    assert_eq!((s.risk, s.action), (Risk::Safe, Action::NativeCommand));
    assert_eq!(s.action_label, "pnpm store prune");
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(f, ctx), Probe::Unchanged));
    // More packages become unreferenced: the prune would exceed the plan.
    std::fs::write(store.join("more"), vec![3u8; 80_000_000]).unwrap();
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(f, ctx), Probe::Changed(_))));
}

#[test]
fn pip_cache_and_missing_tool() {
    let fx = Fx::new();
    let dir = fx.mkdir("Library/Caches/pip/http");
    std::fs::write(dir.join("w"), vec![1u8; 10_000]).unwrap();
    // No pip installed yet.
    let o = fx.scan(ScanKind::Quick, &[Box::new(Pip)]);
    assert_eq!(o.errors[0].kind, "unavailable");
    fx.tool("pip3", &format!("echo '{}'", fx.home.join("Library/Caches/pip").display()));
    let o = fx.scan(ScanKind::Quick, &[Box::new(Pip)]);
    let s = &find(&o, PIP).suggestion;
    assert_eq!((s.risk, s.action_label.as_str()), (Risk::Safe, "pip3 cache purge"));
}

#[test]
fn pip_timeout_reports_timeout() {
    let mut fx = Fx::new();
    fx.tool("pip3", "sleep 3");
    fx.runner = fx.runner.clone().with_timeout(Duration::from_millis(300));
    let o = fx.scan(ScanKind::Quick, &[Box::new(Pip)]);
    assert_eq!(o.errors[0].kind, "timeout");
}

#[test]
fn npm_cache_is_deleted_as_a_folder_never_with_force() {
    let fx = Fx::new();
    let d = fx.mkdir(".npm/_cacache/content-v2");
    std::fs::write(d.join("x"), vec![1u8; 10_000]).unwrap();
    fx.tool("npm", &format!("echo '{}'", fx.home.join(".npm").display()));
    let o = fx.scan(ScanKind::Quick, &[Box::new(Npm)]);
    let f = find(&o, NPM);
    assert_eq!((f.suggestion.risk, f.suggestion.action), (Risk::Safe, Action::Delete));
    let Target::Paths { paths } = &f.target else { panic!() };
    assert!(paths[0].ends_with("_cacache"));
    // Without npm installed the default location is still found.
    std::fs::remove_file(fx.bin.join("npm")).unwrap();
    assert_eq!(fx.scan(ScanKind::Quick, &[Box::new(Npm)]).findings.len(), 1);
}

#[test]
fn homebrew_dry_run_and_growth() {
    let fx = Fx::new();
    let out = fx.root.join("brew.out");
    std::fs::write(&out, "Would remove: /opt/homebrew/old-1.0 (1,707 files, 34.6MB)\n==> This operation would free approximately 1GB of disk space.\n").unwrap();
    fx.tool("brew", &format!("[ \"$1 $2\" = 'cleanup --dry-run' ] || exit 9; cat '{}'", out.display()));
    let o = fx.scan(ScanKind::Quick, &[Box::new(Homebrew)]);
    let f = find(&o, HOMEBREW);
    assert_eq!(f.suggestion.size_bytes, 1_000_000_000);
    assert!(f.suggestion.size_is_estimate);
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(f, ctx), Probe::Unchanged));
    std::fs::write(&out, "Would remove: /opt/homebrew/old-1.0 (34.6MB)\nWould remove: /opt/homebrew/new-2.0 (5MB)\n").unwrap();
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(f, ctx), Probe::Changed(_))));
}

#[test]
fn cargo_and_gradle_are_review_trash() {
    let fx = Fx::new();
    fx.write(".cargo/registry/cache/index/a.crate", &vec![1u8; 20_000]);
    fx.write(".cargo/git/db/repo/pack", &vec![1u8; 20_000]);
    fx.write(".cargo/bin/cargo-tool", b"keep");
    fx.write(".gradle/caches/modules-2/x.jar", &vec![1u8; 20_000]);
    let o = fx.scan(ScanKind::Quick, &[Box::new(Cargo), Box::new(Gradle)]);
    let c = find(&o, CARGO);
    assert_eq!((c.suggestion.risk, c.suggestion.action), (Risk::Review, Action::Trash));
    let Target::Paths { paths } = &c.target else { panic!() };
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().all(|p| !p.contains("/bin")));
    let g = find(&o, GRADLE);
    assert_eq!((g.suggestion.risk, g.suggestion.action), (Risk::Review, Action::Trash));
    assert!(g.suggestion.blocked_reason.is_none());
}

#[test]
fn gradle_blocked_while_daemon_runs_or_unknown() {
    let mut fx = Fx::new();
    fx.write(".gradle/caches/x", &vec![1u8; 20_000]);
    fx.tool("ps", "echo '/usr/bin/java -cp gradle-launcher.jar org.gradle.launcher.daemon.bootstrap.GradleDaemon 8.10'");
    let o = fx.scan(ScanKind::Quick, &[Box::new(Gradle)]);
    assert!(find(&o, GRADLE).suggestion.blocked_reason.as_deref().unwrap().contains("daemon is running"));
    fx.tool("ps", "exit 1");
    let o = fx.scan(ScanKind::Quick, &[Box::new(Gradle)]);
    assert!(find(&o, GRADLE).suggestion.blocked_reason.as_deref().unwrap().contains("couldn't check"));
    // Windows: the daemons' logs and their process IDs, no `ps`.
    fx.platform = "windows";
    let o = fx.scan(ScanKind::Quick, &[Box::new(Gradle)]);
    assert!(find(&o, GRADLE).suggestion.blocked_reason.is_none(), "no daemon logs: nothing runs");
    let mut child = std::process::Command::new(if cfg!(windows) { "cmd" } else { "true" })
        .args(if cfg!(windows) { &["/c", "exit"][..] } else { &[][..] })
        .spawn()
        .unwrap();
    let gone = child.id();
    child.wait().unwrap();
    fx.write(&format!(".gradle/daemon/8.10/daemon-{gone}.out.log"), b"stopped");
    let o = fx.scan(ScanKind::Quick, &[Box::new(Gradle)]);
    assert!(find(&o, GRADLE).suggestion.blocked_reason.is_none(), "log of a stopped daemon");
    fx.write(&format!(".gradle/daemon/8.10/daemon-{}.out.log", std::process::id()), b"running");
    let o = fx.scan(ScanKind::Quick, &[Box::new(Gradle)]);
    assert!(find(&o, GRADLE).suggestion.blocked_reason.as_deref().unwrap().contains("daemon is running"));
}

#[test]
fn xcode_and_simulators_are_macos_only() {
    let mut fx = Fx::new();
    fx.write("Library/Developer/Xcode/DerivedData/App-abc/Build/x", &vec![1u8; 20_000]);
    let json = fx.root.join("simctl.json");
    std::fs::write(&json, include_str!("../../../../tests/fixtures/simctl/unavailable.json")).unwrap();
    fx.tool("xcrun", &format!("cat '{}'", json.display()));
    fx.platform = "macos";
    let o = fx.scan(ScanKind::Quick, &[Box::new(Xcode), Box::new(Simulators)]);
    let x = find(&o, XCODE);
    assert_eq!((x.suggestion.risk, x.suggestion.action), (Risk::Safe, Action::Delete));
    let s = find(&o, SIMULATORS);
    assert_eq!(s.suggestion.risk, Risk::Review);
    let Target::Native { args, .. } = &s.target else { panic!() };
    assert_eq!(args.len(), 4, "bounded to the two listed UDIDs: {args:?}");
    assert_eq!(s.suggestion.size_bytes, 323_502_080 + 45_056 + 13_316_096);
    fx.platform = "linux";
    assert!(fx.scan(ScanKind::Quick, &[Box::new(Xcode), Box::new(Simulators)]).findings.is_empty());
    fx.platform = "macos";
    std::fs::write(&json, "not json").unwrap();
    let o = fx.scan(ScanKind::Quick, &[Box::new(Simulators)]);
    assert_eq!(o.errors[0].kind, "malformed");
}

#[test]
fn homebrew_excluded_on_windows() {
    let mut fx = Fx::new();
    fx.tool("brew", "echo '==> This operation would free approximately 1GB of disk space.'");
    fx.platform = "windows";
    assert!(fx.scan(ScanKind::Quick, &[Box::new(Homebrew)]).findings.is_empty());
}

#[test]
fn gradle_probe_rechecks_daemon() {
    let fx = Fx::new();
    fx.write(".gradle/caches/x", &vec![1u8; 20_000]);
    let o = fx.scan(ScanKind::Quick, &[Box::new(Gradle)]);
    let f = find(&o, GRADLE).clone();
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(&f, ctx), Probe::Unchanged));
    fx.tool("ps", "echo GradleDaemon");
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(&f, ctx), Probe::Changed(_))));
}
