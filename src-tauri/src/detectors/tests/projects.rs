use super::*;
use crate::detectors::projects::*;
use crate::detectors::{probe, Probe};

fn dets() -> Vec<Box<dyn Detector>> {
    vec![Box::new(Dependencies), Box::new(BuildOutput)]
}

fn titles(o: &ScanOutcome) -> Vec<String> {
    let mut v: Vec<String> = o.findings.iter().map(|f| f.suggestion.title.clone()).collect();
    v.sort();
    v
}

#[test]
fn ownership_evidence_table() {
    let fx = Fx::new();
    node_project(&fx, "projects/web", 60);

    // node_modules without a lockfile: no evidence.
    let p = fx.mkdir("projects/nolock/node_modules/x");
    fx.write("projects/nolock/package.json", b"{}");
    std::fs::write(p.join("i.js"), b"x").unwrap();

    // Cargo target.
    fx.write("projects/rust-cli/Cargo.toml", b"[package]");
    fx.write("projects/rust-cli/target/debug/app", &vec![1u8; 50_000]);

    // .venv with requirements.txt: Review, Trash.
    fx.write("projects/data-tools/requirements.txt", b"requests");
    fx.write("projects/data-tools/.venv/lib/site.py", b"x");

    // dist ignored by Git beside a manifest.
    let site = fx.mkdir("projects/site");
    git(&site, &["init", "-q"]);
    fx.write("projects/site/package.json", b"{}");
    fx.write("projects/site/.gitignore", b"dist/\n");
    fx.write("projects/site/dist/app.js", b"x");
    // build not ignored by Git: no evidence.
    fx.write("projects/site/build/out.js", b"x");
    // Unrelated folder named build without a manifest.
    fx.write("projects/photos/build/raw.jpg", b"x");
    // .next not in a Git repository: ignore evidence is unknown.
    fx.write("projects/nogit/package.json", b"{}");
    fx.write("projects/nogit/.next/cache", b"x");

    age(&fx.home.join("projects"), 60);
    let o = fx.scan(ScanKind::Deep, &dets());
    assert!(o.errors.is_empty(), "{:?}", o.errors);
    assert_eq!(
        titles(&o),
        vec![".venv in data-tools", "Rust target folder: rust-cli", "dist folder in site", "node_modules in web"]
    );
    let venv = o.findings.iter().find(|f| f.suggestion.title.starts_with(".venv")).unwrap();
    assert_eq!(venv.suggestion.risk, Risk::Review);
    assert_eq!(venv.suggestion.action, Action::Trash);
    assert!(venv.suggestion.recovery.contains("requirements.txt"));
    let nm = o.findings.iter().find(|f| f.suggestion.title == "node_modules in web").unwrap();
    assert_eq!(nm.suggestion.risk, Risk::Safe);
    assert_eq!(nm.suggestion.action, Action::Delete);
    assert_eq!(nm.suggestion.recovery, "Run npm install in the project.");
    assert!(nm.suggestion.preselected());
    assert_eq!(nm.suggestion.group, Group::Projects);
}

#[test]
fn active_projects_are_skipped() {
    let fx = Fx::new();
    node_project(&fx, "projects/old", 60);
    node_project(&fx, "projects/fresh", 3);
    let o = fx.scan(ScanKind::Deep, &dets());
    assert_eq!(titles(&o), vec!["node_modules in old"]);
}

#[test]
fn inactivity_threshold_follows_settings() {
    let mut fx = Fx::new();
    node_project(&fx, "projects/p", 20);
    assert!(fx.scan(ScanKind::Deep, &dets()).findings.is_empty());
    fx.settings.inactive_project_days = 10;
    assert_eq!(fx.scan(ScanKind::Deep, &dets()).findings.len(), 1);
}

#[test]
fn nested_projects_inside_generated_folders_are_not_discovered() {
    let fx = Fx::new();
    let p = node_project(&fx, "projects/outer", 60);
    // A package inside node_modules that looks like a project.
    let inner = p.join("node_modules/inner");
    std::fs::create_dir_all(inner.join("node_modules/x")).unwrap();
    std::fs::write(inner.join("package.json"), b"{}").unwrap();
    std::fs::write(inner.join("package-lock.json"), b"{}").unwrap();
    age(&p, 60);
    let o = fx.scan(ScanKind::Deep, &dets());
    assert_eq!(titles(&o), vec!["node_modules in outer"]);
}

#[test]
fn running_process_skips_project() {
    let fx = Fx::new();
    let p = node_project(&fx, "projects/busy", 60);
    node_project(&fx, "projects/idle", 60);
    let canon = std::fs::canonicalize(&p).unwrap();
    fx.tool("lsof", &format!("case \"$*\" in *'{}'*) echo p4242;; esac; exit 0", canon.display()));
    let mut child = std::process::Command::new("sleep").arg("30").current_dir(&p).spawn().unwrap();
    let o = fx.scan(ScanKind::Deep, &dets());
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(titles(&o), vec!["node_modules in idle"]);
}

#[test]
fn inv11_unknown_process_state_raises_risk() {
    if cfg!(target_os = "linux") {
        return; // /proc is always readable for the user's own processes.
    }
    let fx = Fx::new();
    node_project(&fx, "projects/p", 60);
    fx.tool("lsof", "exit 3");
    let o = fx.scan(ScanKind::Deep, &dets());
    let s = &o.findings[0].suggestion;
    assert_eq!(s.risk, Risk::Review, "unknown raises Safe to Review");
    assert_eq!(s.action, Action::Trash);
    assert!(!s.preselected());
}

#[test]
fn pnpm_linked_node_modules_report_only_reclaimable_space() {
    let fx = Fx::new();
    let store = fx.mkdir("Library/pnpm/store/v10/files/00");
    let blob = store.join("blob");
    std::fs::write(&blob, vec![7u8; 400_000]).unwrap();
    let p = fx.mkdir("projects/mono");
    std::fs::write(p.join("package.json"), b"{}").unwrap();
    std::fs::write(p.join("pnpm-lock.yaml"), b"x").unwrap();
    std::fs::create_dir_all(p.join("node_modules/.pnpm/dep")).unwrap();
    std::fs::hard_link(&blob, p.join("node_modules/.pnpm/dep/index.js")).unwrap();
    std::fs::write(p.join("node_modules/.modules.yaml"), b"x").unwrap();
    age(&p, 60);
    let o = fx.scan(ScanKind::Deep, &dets());
    let s = &o.findings[0].suggestion;
    assert!(s.size_bytes < 100_000, "shared store data is not reclaimable: {}", s.size_bytes);
    assert!(s.size_is_estimate);
    assert_eq!(s.recovery, "Run pnpm install in the project.");
}

#[test]
fn excluded_and_linked_folders_are_not_scanned() {
    let mut fx = Fx::new();
    node_project(&fx, "projects/kept", 60);
    node_project(&fx, "projects/excluded/app", 60);
    let outside = node_project(&fx, "elsewhere/app", 60);
    std::os::unix::fs::symlink(&outside, fx.home.join("projects/link-to-elsewhere")).unwrap();
    fx.settings.exclusions = vec![fx.home.join("projects/excluded").to_string_lossy().to_string()];
    let o = fx.scan(ScanKind::Deep, &dets());
    assert_eq!(titles(&o), vec!["node_modules in kept"]);
}

#[test]
fn probe_checks_manifest_files_and_links() {
    let fx = Fx::new();
    let p = node_project(&fx, "projects/web", 60);
    let o = fx.scan(ScanKind::Deep, &dets());
    let f = o.findings[0].clone();
    fx.with_ctx(ScanKind::Deep, |ctx| assert_eq!(probe(&f, ctx), Probe::Unchanged));

    std::fs::write(p.join("node_modules/dep/index.js"), b"edited").unwrap();
    fx.with_ctx(ScanKind::Deep, |ctx| assert_eq!(probe(&f, ctx), Probe::Changed("Files inside it changed after the scan.".into())));

    let o = fx.scan(ScanKind::Deep, &dets());
    let f = o.findings[0].clone();
    std::fs::remove_file(p.join("package-lock.json")).unwrap();
    fx.with_ctx(ScanKind::Deep, |ctx| assert!(matches!(probe(&f, ctx), Probe::Changed(r) if r.contains("manifest"))));
}

#[test]
fn dependencies_are_one_item_split_by_project() {
    // Spec `project.dependencies`: "split by project". Three inactive
    // projects become one node_modules item with one part per project,
    // largest first, each part keeping its own complete finding.
    let fx = Fx::new();
    node_project(&fx, "projects/web-app", 60);
    node_project(&fx, "projects/docs-site", 60);
    node_project(&fx, "projects/admin", 60);
    fx.write("projects/web-app/node_modules/big.bin", &vec![1u8; 300_000]);
    fx.write("projects/admin/node_modules/mid.bin", &vec![1u8; 100_000]);
    age(&fx.home.join("projects"), 60);
    let o = fx.scan(ScanKind::Deep, &dets());
    assert!(o.errors.is_empty(), "{:?}", o.errors);
    assert_eq!(titles(&o), vec!["node_modules in 3 inactive projects"]);
    let f = &o.findings[0];
    let s = &f.suggestion;
    assert_eq!(s.group, Group::Projects);
    assert_eq!(s.map_area, "projects");
    assert_eq!(s.location, "~/projects");
    assert_eq!(s.action_label, "Delete the 3 folders");
    let labels: Vec<_> = s.parts.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, vec!["web-app", "admin", "docs-site"], "parts are largest first");
    assert!(s.parts.iter().all(|p| p.selected), "every part starts selected");
    let sum: u64 = s.parts.iter().map(|p| p.size_bytes).sum();
    assert_eq!(s.size_bytes, sum, "the item's size is the sum of its selected parts");
    assert_eq!(s.full_size_bytes, sum);
    assert_eq!(f.parts.len(), 3, "each part keeps its own finding");
    for (part, sub) in s.parts.iter().zip(&f.parts) {
        assert_eq!(part.id, sub.suggestion.id);
        assert_eq!(sub.paths().len(), 1);
        assert!(sub.paths()[0].ends_with(&format!("{}/node_modules", part.label)));
        assert!(!sub.suggestion.fingerprint.is_empty());
    }
    // Stable identity: the same projects give the same item and part IDs.
    let again = fx.scan(ScanKind::Deep, &dets());
    assert_eq!(again.findings[0].suggestion.id, s.id);
    assert_eq!(again.findings[0].suggestion.parts, s.parts);
}

#[test]
fn a_single_project_stays_a_plain_item() {
    let fx = Fx::new();
    node_project(&fx, "projects/only", 60);
    let o = fx.scan(ScanKind::Deep, &dets());
    assert_eq!(titles(&o), vec!["node_modules in only"]);
    assert!(o.findings[0].suggestion.parts.is_empty());
    assert_eq!(o.findings[0].suggestion.full_size_bytes, o.findings[0].suggestion.size_bytes);
}
