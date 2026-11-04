//! Scan performance benchmark (read-only).
//!
//!   cargo run --release --no-default-features --example scan_bench -- quick
//!   cargo run --release --no-default-features --example scan_bench -- make-fixture <dir> <files>
//!   cargo run --release --no-default-features --example scan_bench -- deep <dir>
//!
//! `quick` runs the quick-scan detectors against this machine with the
//! read-only runner policy. `deep` walks a fixture folder as the only scan
//! folder. Nothing is modified except creating the fixture.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use freeboard_lib::model::ScanKind;
use freeboard_lib::runner::Runner;
use freeboard_lib::scan::{default_detectors, run_scan, ScanContext};
use freeboard_lib::settings::Settings;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("make-fixture") => make_fixture(&args[1], args[2].parse().unwrap()),
        Some("quick") => bench(ScanKind::Quick, Settings::default()),
        Some("deep") => {
            let mut s = Settings::default();
            s.scan_folders = vec![std::fs::canonicalize(&args[1]).unwrap().to_string_lossy().into_owned()];
            s.onboarding_complete = true;
            bench(ScanKind::Deep, s)
        }
        _ => eprintln!("usage: scan_bench quick | deep <dir> | make-fixture <dir> <files>"),
    }
}

fn bench(kind: ScanKind, settings: Settings) {
    let runner = Runner::default();
    let cancel = AtomicBool::new(false);
    let pause = || false;
    let last = std::sync::Mutex::new((Instant::now(), String::from("start")));
    let progress = |_: &str, d: Option<&str>, _: u64| {
        let mut l = last.lock().unwrap();
        eprintln!("  {:>7.2}s {}", l.0.elapsed().as_secs_f64(), l.1);
        *l = (Instant::now(), d.unwrap_or("").to_string());
    };
    let ctx = ScanContext {
        runner: &runner,
        home: freeboard_lib::monitor::home_or_root(),
        settings,
        now: chrono::Utc::now().timestamp(),
        kind,
        cancel: &cancel,
        should_pause: &pause,
        progress: &progress,
        platform: freeboard_lib::platform::platform_name(),
    };
    let t = Instant::now();
    let out = run_scan(&ctx, &default_detectors());
    let secs = t.elapsed().as_secs_f64();
    progress("end", Some("end"), 0);
    let bytes: u64 = out.findings.iter().map(|f| f.suggestion.size_bytes).sum();
    println!(
        "{{\"kind\":\"{:?}\",\"seconds\":{secs:.2},\"findings\":{},\"bytes\":{bytes},\"detectorErrors\":{}}}",
        kind,
        out.findings.len(),
        out.errors.len()
    );
    for e in &out.errors {
        eprintln!("  {}: {} ({})", e.detector, e.kind, e.message);
    }
}

/// 1 million small files spread over project-like folders: 1000 projects,
/// each with source files and a node_modules tree.
fn make_fixture(dir: &str, files: usize) {
    let root = std::path::Path::new(dir);
    let per_project = files / 1000;
    for p in 0..1000 {
        let proj = root.join(format!("proj-{p:04}"));
        std::fs::create_dir_all(proj.join("src")).unwrap();
        std::fs::write(proj.join("package.json"), b"{}").unwrap();
        std::fs::write(proj.join("pnpm-lock.yaml"), b"lockfileVersion: 9").unwrap();
        for i in 0..(per_project / 10) {
            std::fs::write(proj.join("src").join(format!("f{i}.ts")), b"x").unwrap();
        }
        for d in 0..(per_project * 9 / 10 / 100).max(1) {
            let nm = proj.join("node_modules").join(format!("pkg{d}"));
            std::fs::create_dir_all(&nm).unwrap();
            for i in 0..100 {
                std::fs::write(nm.join(format!("m{i}.js")), b"x").unwrap();
            }
        }
    }
}
