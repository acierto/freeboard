//! Whole-scan tests: read-only proof, link traps, isolation, ordering and
//! the suggestion contract.

use super::*;
use crate::scan::sort_findings;

/// A fixture with something for every detector, plus traps.
fn world(fx: &mut Fx) -> PathBuf {
    fx.docker(true);
    node_project(fx, "projects/web", 60);
    fx.write("projects/rust-cli/Cargo.toml", b"[package]");
    fx.write("projects/rust-cli/target/debug/app", &vec![1u8; 50_000]);
    let app = fx.mkdir("projects/repo");
    git(&app, &["init", "-q"]);
    std::fs::write(app.join("a.txt"), "a").unwrap();
    git(&app, &["add", "."]);
    git(&app, &["commit", "-q", "-m", "a"]);
    git(&app, &["worktree", "add", "-q", "-b", "feature", &app.join(".worktrees/feature").to_string_lossy()]);
    fx.write("Downloads/old.dmg", &vec![1u8; 4096]);
    fx.write("Downloads/report.pdf", b"doc");
    fx.write(".npm/_cacache/x", &vec![1u8; 4096]);
    fx.write(".cargo/registry/cache/a.crate", &vec![1u8; 4096]);
    fx.write(".gradle/caches/x", &vec![1u8; 4096]);
    fx.write("Library/Developer/Xcode/DerivedData/x/y", &vec![1u8; 4096]);
    fx.write("pnpm-store/v10/files/a", &vec![1u8; 4096]);
    fx.write("pip-cache/http/a", &vec![1u8; 4096]);
    fx.tool("pnpm", &format!("echo '{}'", fx.home.join("pnpm-store/v10").display()));
    fx.tool("pip3", &format!("echo '{}'", fx.home.join("pip-cache").display()));
    fx.tool("npm", &format!("echo '{}'", fx.home.join(".npm").display()));
    fx.tool("brew", "echo 'Would remove: /x (1MB)'; echo '==> This operation would free approximately 1MB of disk space.'");
    fx.tool("xcrun", "echo '{\"devices\":{}}'");
    // Detectors added in slice 3 (tasks 5.1–5.8), one finding each.
    for v in ["IntelliJIdea2023.1", "IntelliJIdea2024.2"] {
        fx.write(&format!("Library/Caches/JetBrains/{v}/caches/x"), &vec![1u8; 4096]);
    }
    fx.write("Library/Application Support/Slack/Cache/a", &vec![1u8; 4096]);
    for b in ["chromium-1100", "chromium-1150"] {
        fx.write(&format!("Library/Caches/ms-playwright/{b}/bin"), &vec![1u8; 4096]);
    }
    let digest = format!("sha256:{}", "e".repeat(64));
    fx.write(".ollama/models/manifests/registry.ollama.ai/library/llama3/8b", format!(r#"{{"layers":[{{"digest":"{digest}","size":4096}}]}}"#).as_bytes());
    fx.write(&format!(".ollama/models/blobs/{}", digest.replace(':', "-")), &vec![1u8; 4096]);
    fx.tool("ollama", "exit 0");
    fx.write(".cache/huggingface/hub/models--org--bert/blobs/a", &vec![1u8; 4096]);
    fx.write(".lmstudio/models/pub/model-GGUF/m.gguf", &vec![1u8; 4096]);
    fx.write(
        "Library/Application Support/MobileSync/Backup/0000-AAAA/Info.plist",
        b"<plist><dict><key>Device Name</key><string>Old iPhone</string><key>Last Backup Date</key><date>2024-01-01T00:00:00Z</date></dict></plist>",
    );
    fx.write(".android/avd/Pixel.avd/userdata-qemu.img", &vec![1u8; 4096]);
    for v in ["v18.19.0", "v22.8.0"] {
        fx.write(&format!(".nvm/versions/node/{v}/bin/node"), &vec![1u8; 4096]);
    }
    for t in ["stable-aarch64-apple-darwin", "1.70.0-aarch64-apple-darwin"] {
        fx.write(&format!(".rustup/toolchains/{t}/bin/rustc"), &vec![1u8; 4096]);
    }
    fx.tool("rustup", "echo 'stable-aarch64-apple-darwin (default)'; echo '1.70.0-aarch64-apple-darwin'");

    // Traps: a link into a precious tree outside the scan roots, a link
    // loop, and an excluded folder containing a valid project.
    let precious = fx.root.join("precious");
    std::fs::create_dir_all(precious.join("node_modules/x")).unwrap();
    std::fs::write(precious.join("package.json"), b"{}").unwrap();
    std::fs::write(precious.join("package-lock.json"), b"{}").unwrap();
    std::os::unix::fs::symlink(&precious, fx.home.join("projects/trap")).unwrap();
    std::os::unix::fs::symlink(fx.home.join("projects"), fx.home.join("projects/web/loop")).unwrap();
    std::os::unix::fs::symlink(&precious, fx.home.join("projects/web/node_modules/escape")).unwrap();
    node_project(fx, "projects/excluded/app", 60);
    fx.settings.exclusions = vec![fx.home.join("projects/excluded").to_string_lossy().to_string()];
    age(&fx.home, 120);
    age(&precious, 120);
    precious
}

#[test]
fn inv01_scans_are_read_only() {
    let mut fx = Fx::new();
    let precious = world(&mut fx);
    let before_home = tree(&fx.home);
    let before_precious = tree(&precious);
    let quick = fx.scan_all(ScanKind::Quick);
    let deep = fx.scan_all(ScanKind::Deep);
    assert!(!quick.findings.is_empty() && !deep.findings.is_empty());
    assert_eq!(tree(&fx.home), before_home, "a scan changed the fixture tree");
    assert_eq!(tree(&precious), before_precious);
    assert!(!fx.root.join("docker-state/actions").exists(), "a scan ran a mutating docker command");
}

#[test]
fn inv06_scans_never_follow_links() {
    let mut fx = Fx::new();
    let precious = world(&mut fx);
    let o = fx.scan_all(ScanKind::Deep);
    let precious = precious.to_string_lossy().to_string();
    for f in &o.findings {
        let s = &f.suggestion;
        assert!(!s.location.contains("trap") && !s.location.contains("precious"), "{}", s.location);
        assert!(!s.location.contains("excluded"), "exclusion ignored: {}", s.location);
        if let Target::Paths { paths } = &f.target {
            assert!(paths.iter().all(|p| !p.starts_with(&precious) && !p.contains("/trap")), "{paths:?}");
        }
    }
    // The node_modules size does not include the escaped link target.
    let nm = o.findings.iter().find(|f| f.suggestion.title == "node_modules in web").unwrap();
    assert!(nm.suggestion.size_bytes < 400_000, "{}", nm.suggestion.size_bytes);
}

#[test]
fn every_detector_runs_and_contract_is_complete() {
    let mut fx = Fx::new();
    world(&mut fx);
    fx.platform = "macos";
    let o = fx.scan_all(ScanKind::Deep);
    assert!(o.errors.is_empty(), "{:?}", o.errors);
    let ids: std::collections::BTreeSet<&str> = o.findings.iter().map(|f| f.suggestion.detector.as_str()).collect();
    for id in [
        "docker.build_cache",
        "docker.dangling_images",
        "docker.unused_images",
        "docker.stopped_containers",
        "docker.volumes",
        "git.worktrees",
        "project.dependencies",
        "project.build_output",
        "cache.pnpm",
        "cache.pip",
        "cache.npm",
        "cache.homebrew",
        "cache.cargo",
        "cache.gradle",
        "cache.xcode",
        "cache.jetbrains",
        "cache.electron_apps",
        "cache.playwright",
        "ai.ollama",
        "ai.huggingface",
        "ai.lm_studio",
        "mobile.ios_backups",
        "mobile.android_avd",
        "tools.node_versions",
        "tools.rust_toolchains",
        "files.installers",
    ] {
        assert!(ids.contains(id), "missing {id}: {ids:?}");
    }
    let mut seen = std::collections::HashSet::new();
    for f in &o.findings {
        let s = &f.suggestion;
        assert!(seen.insert(s.id.clone()), "duplicate id {}", s.id);
        for (name, v) in [
            ("title", &s.title),
            ("location", &s.location),
            ("lastUsedLabel", &s.last_used_label),
            ("actionLabel", &s.action_label),
            ("reason", &s.reason),
            ("recovery", &s.recovery),
            ("fingerprint", &s.fingerprint),
        ] {
            assert!(!v.is_empty(), "{} has empty {name}", s.detector);
        }
        // Detector IDs are the registered IDs (core relies on this).
        assert!(crate::detectors::all().iter().any(|d| d.id() == s.detector));
        // Only eligible Safe items are preselected.
        assert_eq!(s.preselected(), s.risk == Risk::Safe && s.blocked_reason.is_none() && !s.needs_confirmation);
    }
    // Stable IDs across scans.
    let again = fx.scan_all(ScanKind::Deep);
    let a: Vec<&str> = o.findings.iter().map(|f| f.suggestion.id.as_str()).collect();
    let b: Vec<&str> = again.findings.iter().map(|f| f.suggestion.id.as_str()).collect();
    assert_eq!(a, b);
}

#[test]
fn grouped_by_category_and_sorted_by_size() {
    let mut fx = Fx::new();
    world(&mut fx);
    let o = fx.scan_all(ScanKind::Deep);
    for w in o.findings.windows(2) {
        let (a, b) = (&w[0].suggestion, &w[1].suggestion);
        assert!(a.group.rank() <= b.group.rank());
        if a.group == b.group {
            assert!(a.size_bytes >= b.size_bytes);
        }
    }
    let mut shuffled = o.findings.clone();
    shuffled.reverse();
    sort_findings(&mut shuffled);
    assert_eq!(shuffled, o.findings);
}

#[test]
fn one_failing_detector_does_not_stop_others() {
    let mut fx = Fx::new();
    world(&mut fx);
    std::fs::remove_file(fx.bin.join("pnpm")).unwrap();
    fx.tool("pip3", "echo 'not a path'");
    std::fs::write(fx.root.join("docker-state/down"), "").unwrap();
    let o = fx.scan_all(ScanKind::Deep);
    let errs: Vec<(&str, &str)> = o.errors.iter().map(|e| (e.detector.as_str(), e.kind.as_str())).collect();
    assert!(errs.contains(&("cache.pnpm", "unavailable")), "{errs:?}");
    assert!(errs.contains(&("cache.pip", "malformed")), "{errs:?}");
    assert!(errs.contains(&("docker.volumes", "unavailable")), "{errs:?}");
    assert!(o.findings.iter().any(|f| f.suggestion.detector == "project.dependencies"));
    assert!(o.findings.iter().any(|f| f.suggestion.detector == "files.installers"));
}

#[test]
fn deep_scan_pauses_on_battery_and_cancels() {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    let mut fx = Fx::new();
    world(&mut fx);
    let cancel = AtomicBool::new(false);
    let polls = AtomicU32::new(0);
    // Report "on battery" a few times, then cancel while paused.
    let pause = || {
        let n = polls.fetch_add(1, Ordering::SeqCst);
        if n > 3 {
            cancel.store(true, Ordering::SeqCst);
        }
        true
    };
    let phases = std::sync::Mutex::new(Vec::new());
    let progress = |p: &str, _: Option<&str>, _: u64| phases.lock().unwrap().push(p.to_string());
    let ctx = ScanContext {
        runner: &fx.runner,
        home: fx.home.clone(),
        settings: fx.settings.clone(),
        now: NOW,
        kind: ScanKind::Deep,
        cancel: &cancel,
        should_pause: &pause,
        progress: &progress,
        platform: fx.platform,
    };
    let o = run_scan(&ctx, &crate::detectors::all());
    assert!(o.findings.is_empty(), "no detector runs while paused");
    assert!(phases.lock().unwrap().iter().any(|p| p == "paused"));
    // Quick scans are lightweight and are not paused.
    let cancel2 = AtomicBool::new(false);
    let ctx = ScanContext { kind: ScanKind::Quick, cancel: &cancel2, ..ctx };
    assert!(!run_scan(&ctx, &crate::detectors::all()).findings.is_empty());
}
