//! Accounting tests (spec test plan): hard links, nested detectors,
//! unreadable folders, totals, blind spots, places checked, growth and the
//! disk report. Everything runs on temporary fixture folders.

use super::roots::{measure, MeasureEnv, RootBytes};
use super::*;
use crate::model::*;
use crate::scan::size::{self, MeasureOpts};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

const KB: usize = 1024;

fn write(p: &Path, bytes: usize, fill: &[u8]) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    let mut v = Vec::with_capacity(bytes);
    while v.len() < bytes {
        v.extend_from_slice(fill);
    }
    v.truncate(bytes);
    fs::write(p, v).unwrap();
}

fn on_disk(p: &Path) -> u64 {
    size::measure(p, &MeasureOpts { serial: true, ..Default::default() }).bytes
}

fn sugg(id: &str, area: &str, group: Group, bytes: u64) -> Suggestion {
    let mut s = crate::detectors::common::base("test.detector", group, id);
    s.map_area = area.into();
    s.title = format!("Item {id}");
    s.location = format!("~/{id}");
    s.size_bytes = bytes;
    s.full_size_bytes = bytes;
    s.risk = Risk::Safe;
    s.action = Action::Delete;
    s
}

fn path_finding(id: &str, area: &str, p: &Path) -> Finding {
    Finding::new(sugg(id, area, Group::Projects, on_disk(p)), Target::Paths { paths: vec![p.to_string_lossy().into_owned()] })
}

fn native_finding(id: &str, area: &str, bytes: u64) -> Finding {
    let mut s = sugg(id, area, Group::Docker, bytes);
    s.action = Action::NativeCommand;
    Finding::new(s, Target::Native { program: "docker".into(), args: vec![], resources: vec![], cwd: None })
}

struct Fixture {
    _t: tempfile::TempDir,
    home: PathBuf,
}

/// A home folder with a project (with node_modules), an unknown data folder
/// inside the scan folder, an unscanned folder, caches, AI models, a hard
/// link shared between two places, and loose files.
fn fixture() -> Fixture {
    let t = tempfile::tempdir().unwrap();
    let home = fs::canonicalize(t.path()).unwrap().join("home");
    let proj = home.join("projects");
    fs::create_dir_all(proj.join("app/.git")).unwrap();
    write(&proj.join("app/package.json"), 200, b"{}");
    write(&proj.join("app/src/index.js"), 300 * KB, b"console.log(1);\n");
    write(&proj.join("app/node_modules/dep/index.js"), 900 * KB, b"module.exports=1;\n");
    write(&proj.join("tools/sim-data/run1.dat"), 700 * KB, b"FREEBOARD-SECRET-MARKER ");
    write(&proj.join("tools/sim-data/run2.dat"), 700 * KB, b"FREEBOARD-SECRET-MARKER ");
    write(&home.join("archive/old-laptop/disk.img"), 1200 * KB, b"FREEBOARD-SECRET-MARKER ");
    write(&home.join(".cache/pip/wheels/a.whl"), 400 * KB, b"pip");
    write(&home.join(".cache/huggingface/hub/model.bin"), 800 * KB, b"weights");
    write(&home.join("Pictures/holiday.jpg"), 500 * KB, b"jpeg");
    write(&home.join("notes.txt"), 20 * KB, b"FREEBOARD-SECRET-MARKER ");
    // The same file in two places: counted once.
    write(&home.join("Pictures/big.raw"), 600 * KB, b"raw");
    fs::hard_link(home.join("Pictures/big.raw"), proj.join("app/src/big-copy.raw")).unwrap();
    Fixture { _t: t, home }
}

fn env<'a>(home: &Path, platform: &'a str, cancel: &'a AtomicBool, repos: &'a [PathBuf]) -> MeasureEnv<'a> {
    MeasureEnv {
        home: home.to_path_buf(),
        platform,
        scan_roots: vec![home.join("projects")],
        exclusions: vec![],
        repos,
        full_disk_access: if platform == "macos" { Some(false) } else { None },
        outside_home: false,
        cancel,
        progress: &|_| {},
        now: 1_800_000_000,
        min_blind_spot: 1_000_000,
    }
}

fn input<'a>(used: u64, claims: &'a [Claim], native: &'a HashMap<String, u64>, m: Option<&'a Measurement>, decisions: &'a HashMap<String, SpotDecision>) -> BuildInput<'a> {
    BuildInput { used_bytes: used, platform: "linux", claims, native, measurement: m, system_volume_bytes: 0, decisions, purgeable_bytes: None, snapshot_count: None }
}

fn area<'a>(m: &'a DiskMap, id: &str) -> &'a DiskMapArea {
    m.areas.iter().find(|a| a.id == id).unwrap()
}

fn total(m: &DiskMap) -> u64 {
    m.areas.iter().map(|a| a.bytes).sum::<u64>() + m.not_explained_bytes
}

#[test]
fn nested_claims_are_counted_once_most_specific_first() {
    // A worktree (3 MB) holding a node_modules (1 MB) claimed by the projects detector.
    let wt = Claim { paths: vec![PathBuf::from("/h/wt")], bytes: 3_000_000, area: "projects".into() };
    let nm = Claim { paths: vec![PathBuf::from("/h/wt/node_modules")], bytes: 1_000_000, area: "projects".into() };
    let dup = Claim { paths: vec![PathBuf::from("/h/wt/node_modules")], bytes: 1_000_000, area: "caches".into() };
    let r = resolve_claims(vec![wt, nm, dup]);
    assert_eq!(r.len(), 2, "the same path is claimed once");
    assert_eq!(r.iter().map(|c| c.bytes).sum::<u64>(), 3_000_000);
    assert_eq!(r[0].paths[0], PathBuf::from("/h/wt/node_modules"), "most specific first");
    assert_eq!(r[1].bytes, 2_000_000);
}

#[test]
fn fixture_totals_match_used_bytes_within_one_percent() {
    let fx = fixture();
    let used = on_disk(&fx.home);
    let nm = fx.home.join("projects/app/node_modules");
    let findings = vec![path_finding("deps", "projects", &nm)];
    let (claims, native) = claims_of(&findings);
    let cancel = AtomicBool::new(false);
    let repos = vec![fx.home.join("projects/app")];
    let m = measure(&env(&fx.home, "linux", &cancel, &repos), &claims);
    let decisions = HashMap::new();
    let map = build(&input(used, &claims, &native, Some(&m), &decisions));
    let diff = (total(&map) as f64 - used as f64).abs() / used as f64;
    assert!(diff <= TOLERANCE, "map total {} vs used {used} ({:.3}%)", total(&map), diff * 100.0);
    assert!(!map.incomplete);
    // Nesting: node_modules is counted in Projects once, as the claim, and
    // not again in the project's own files.
    let proj = area(&map, "projects");
    let nm_bytes = on_disk(&nm);
    assert!(proj.bytes >= nm_bytes && proj.leave_alone_bytes < proj.bytes - nm_bytes + 1, "{proj:?}");
    // Hugging Face sits in .cache but is an AI model: most specific wins.
    assert!(area(&map, "ai").bytes >= 800 * KB as u64, "{:?}", area(&map, "ai"));
    assert!(area(&map, "caches").bytes < 800 * KB as u64, "{:?}", area(&map, "caches"));
    // The hard-linked file is counted once (Pictures or the project, not both).
    let pictures_and_project = area(&map, "files").bytes + proj.bytes;
    assert!(pictures_and_project < on_disk(&fx.home.join("Pictures")) + on_disk(&fx.home.join("projects/app")) - 500 * KB as u64);
    // The unscanned folder and the unknown data are Not explained, and are blind spots.
    let unknown = on_disk(&fx.home.join("projects/tools"));
    let archive = on_disk(&fx.home.join("archive"));
    assert!(map.not_explained_bytes >= unknown + archive - 64 * KB as u64, "{} < {}", map.not_explained_bytes, unknown + archive);
    assert_eq!(m.unscanned.len(), 1);
    assert!(m.unscanned[0].path.ends_with("archive"));
    assert_eq!(m.unknown.len(), 1);
    assert!(m.unknown[0].path.ends_with("projects/tools"));
    assert_eq!(m.unknown[0].ext.as_deref(), Some("dat"));
}

#[cfg(unix)]
#[test]
fn unreadable_folder_is_not_explained_and_a_blind_spot() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc::geteuid() } == 0 {
        return; // root reads everything
    }
    let fx = fixture();
    let locked = fx.home.join("projects/app/private");
    write(&locked.join("secret.bin"), 800 * KB, b"x");
    let used = on_disk(&fx.home);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let cancel = AtomicBool::new(false);
    let repos = vec![fx.home.join("projects/app")];
    let m = measure(&env(&fx.home, "linux", &cancel, &repos), &[]);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    let decisions = HashMap::new();
    let native = HashMap::new();
    let with_lock = build(&input(used, &[], &native, Some(&m), &decisions));
    assert!(with_lock.not_explained_bytes >= 800 * KB as u64, "its space stays in Not explained");
    let spots = blind_spots(&spot_input(Some(&m), &[], &decisions, &fx.home));
    let perm: Vec<_> = spots.iter().filter(|s| s.kind == BlindSpotKind::NeedsPermission).collect();
    assert_eq!(perm.len(), 1, "{spots:#?}");
    assert!(perm[0].location.ends_with("projects/app/private"));
    assert_eq!(perm[0].size_bytes, None, "unknown, never guessed");
    // Readable again: coverage rises.
    let m2 = measure(&env(&fx.home, "linux", &cancel, &repos), &[]);
    let readable = build(&input(used, &[], &native, Some(&m2), &decisions));
    assert!(readable.not_explained_bytes + 700 * KB as u64 <= with_lock.not_explained_bytes);
}

fn spot_input<'a>(m: Option<&'a Measurement>, errors: &'a [DetectorError], decisions: &'a HashMap<String, SpotDecision>, home: &'a Path) -> SpotInput<'a> {
    SpotInput {
        measurement: m,
        errors,
        decisions,
        scan_folders: &[],
        exclusions: &[],
        full_disk_access: None,
        scanning: false,
        now: 1_800_000_100,
        last_scan_at: Some(1_800_000_000),
        platform: "linux",
        home,
        findings: &[],
    }
}

#[test]
fn claims_over_used_bytes_flag_the_map_incomplete() {
    let native = HashMap::new();
    let d = HashMap::new();
    let claims = vec![Claim { paths: vec!["/h/a".into()], bytes: 102 * GB, area: "projects".into() }];
    let over = build(&input(100 * GB, &claims, &native, None, &d));
    assert!(over.incomplete, "2% over is shown as incomplete");
    assert_eq!(over.over_bytes, 2 * GB);
    assert_eq!(over.not_explained_bytes, 0);
    let close = vec![Claim { paths: vec!["/h/a".into()], bytes: 100 * GB + GB / 2, area: "projects".into() }];
    assert!(!build(&input(100 * GB, &close, &native, None, &d)).incomplete, "0.5% is within tolerance");
    // Never redistributed: coverage below 100% shows as Not explained.
    let under = vec![Claim { paths: vec!["/h/a".into()], bytes: 60 * GB, area: "projects".into() }];
    let m = build(&input(100 * GB, &under, &native, None, &d));
    assert_eq!(m.not_explained_bytes, 40 * GB);
    assert_eq!(total(&m), 100 * GB);
}

#[test]
fn leave_alone_space_is_the_area_minus_its_suggestions() {
    // Spec scenario: AI models 61.6 GB, suggestions 53.6 GB, 8.0 GB to leave alone.
    let claims = vec![Claim { paths: vec!["/h/.ollama/models/a".into()], bytes: 53_600_000_000, area: "ai".into() }];
    let m = Measurement { roots: vec![RootBytes { path: "/h/.ollama".into(), area: "ai".into(), bytes: 8_000_000_000 }], claims: vec![roots::ClaimRecord { paths: vec!["/h/.ollama/models/a".into()], bytes: 53_600_000_000 }], ..Default::default() };
    let native = HashMap::new();
    let d = HashMap::new();
    let map = build(&input(500 * GB, &claims, &native, Some(&m), &d));
    let ai = area(&map, "ai");
    assert_eq!(ai.bytes, 61_600_000_000);
    assert_eq!(ai.leave_alone_bytes, 8_000_000_000);
    assert_eq!(ai.leave_alone_label, "Models you used recently");
    assert_eq!(area(&map, "system").label, "Linux");
}

#[test]
fn docker_virtual_disk_is_claimed_without_double_counting() {
    let d = HashMap::new();
    let m = Measurement { vm_disks: vec![roots::VmDisk { path: "/h/Docker.raw".into(), area: "docker".into(), bytes: 50 * GB }], ..Default::default() };
    // Images and build cache (30 GB) live inside the 50 GB disk.
    let native: HashMap<String, u64> = [("docker".to_string(), 30 * GB)].into();
    let map = build(&input(400 * GB, &[], &native, Some(&m), &d));
    assert_eq!(area(&map, "docker").bytes, 50 * GB);
    assert_eq!(area(&map, "docker").leave_alone_bytes, 20 * GB);
    // Without a virtual disk (Linux) the items alone make the area.
    let map = build(&input(400 * GB, &[], &native, None, &d));
    assert_eq!(area(&map, "docker").bytes, 30 * GB);
    assert_eq!(area(&map, "docker").leave_alone_bytes, 0);
}

#[test]
fn quick_scans_reconcile_claims_against_the_stored_measurement() {
    let t = tempfile::tempdir().unwrap();
    let still_there = t.path().join("old");
    fs::create_dir_all(&still_there).unwrap();
    let root = t.path().to_string_lossy().into_owned();
    let m = Measurement {
        roots: vec![RootBytes { path: root, area: "caches".into(), bytes: 10 * GB }],
        claims: vec![roots::ClaimRecord { paths: vec![still_there.to_string_lossy().into_owned()], bytes: 3 * GB }],
        ..Default::default()
    };
    let native = HashMap::new();
    let d = HashMap::new();
    // The old claim is no longer suggested but still on disk: back in the root.
    let map = build(&input(100 * GB, &[], &native, Some(&m), &d));
    assert_eq!(area(&map, "caches").bytes, 13 * GB);
    // A new claim inside the root moves bytes out of the root, not on top.
    let new = vec![Claim { paths: vec![t.path().join("new")], bytes: 2 * GB, area: "caches".into() }];
    let map = build(&input(100 * GB, &new, &native, Some(&m), &d));
    assert_eq!(area(&map, "caches").bytes, 13 * GB);
    assert_eq!(area(&map, "caches").leave_alone_bytes, 11 * GB);
}

#[test]
fn macos_private_places_are_never_read_without_full_disk_access() {
    let fx = fixture();
    write(&fx.home.join("Library/Containers/com.example/Data/big.bin"), 500 * KB, b"c");
    write(&fx.home.join("Documents/report.pdf"), 300 * KB, b"FREEBOARD-SECRET-MARKER ");
    write(&fx.home.join("Library/Caches/app/cache.db"), 300 * KB, b"cache");
    let cancel = AtomicBool::new(false);
    let repos = vec![];
    let m = measure(&env(&fx.home, "macos", &cancel, &repos), &[]);
    assert!(m.denied.iter().any(|d| d.path.ends_with("Library/Containers") && d.kind == size::Denied::Privacy), "{:?}", m.denied);
    assert!(!m.roots.iter().any(|r| r.path.contains("Containers")), "Containers were not read");
    // Documents would make macOS ask: not read, size unknown.
    // Music, Movies and Pictures hold the Apple Music, TV and Photos libraries:
    // reading them asks for media library access (found on a real Mac).
    for f in ["Music/Music/Media.localized", "Movies/TV", "Pictures/Shots", "Library/Calendars", "Library/Application Support/AddressBook"] {
        write(&fx.home.join(f).join("data.bin"), 100 * KB, b"m");
    }
    let m = measure(&env(&fx.home, "macos", &cancel, &repos), &[]);
    for f in ["Music", "Movies", "Pictures"] {
        assert!(!m.roots.iter().any(|r| r.path.ends_with(f)), "{f} must not be read on macOS");
        assert!(m.unscanned.iter().any(|c| c.path.ends_with(f) && c.bytes.is_none()), "{f} is a blind spot of unknown size");
    }
    for f in ["Library/Calendars", "Library/Application Support/AddressBook"] {
        assert!(!m.roots.iter().any(|r| r.path.ends_with(f)), "{f} must not be read without Full Disk Access");
    }
    let docs = m.unscanned.iter().find(|c| c.path.ends_with("Documents")).expect("Documents is a blind spot");
    assert_eq!(docs.bytes, None);
    assert!(!m.roots.iter().any(|r| r.path.ends_with("Documents")));
    // Library/Caches is still measured.
    assert!(m.roots.iter().any(|r| r.path.ends_with("Library/Caches") && r.area == "caches" && r.bytes >= 300 * KB as u64));
    // Apple's media data in Library is never read: walking all of ~/Library
    // asked for the Apple Music media library on a real Mac (2026-11-07).
    // Third-party app data in Application Support still counts.
    for f in ["Library/AppleMediaServices", "Library/Application Support/com.apple.MediaPlayer", "Library/Caches/com.apple.iTunesCloud"] {
        write(&fx.home.join(f).join("media.bin"), 700 * KB, b"m");
    }
    write(&fx.home.join("Library/Application Support/SomeApp/data.bin"), 200 * KB, b"a");
    let m = measure(&env(&fx.home, "macos", &cancel, &repos), &[]);
    assert!(!m.roots.iter().any(|r| r.path.ends_with("/Library")), "all of Library must not be walked without Full Disk Access");
    let support = m.roots.iter().find(|r| r.path.ends_with("Library/Application Support")).expect("Application Support is measured");
    assert!(support.bytes >= 200 * KB as u64 && support.bytes < 700 * KB as u64, "{}", support.bytes);
    let caches = m.roots.iter().find(|r| r.path.ends_with("Library/Caches")).unwrap();
    assert!(caches.bytes < 700 * KB as u64, "Apple's caches are left alone: {}", caches.bytes);
    let d = HashMap::new();
    let mut i = spot_input(Some(&m), &[], &d, &fx.home);
    i.platform = "macos";
    let spots = blind_spots(&i);
    let perm = spots.iter().find(|s| s.kind == BlindSpotKind::NeedsPermission).unwrap();
    assert_eq!(perm.action, BlindSpotAction::OpenSettings);
    assert_eq!(perm.action_label, "Allow access");
    assert!(perm.text.contains("Full Disk Access"), "{}", perm.text);
    // With Full Disk Access, the same places are measured.
    let mut e = env(&fx.home, "macos", &cancel, &repos);
    e.full_disk_access = Some(true);
    let m2 = measure(&e, &[]);
    assert!(m2.denied.is_empty(), "{:?}", m2.denied);
    assert!(m2.roots.iter().any(|r| r.path.ends_with("Library/Containers")));
}

#[test]
fn blind_spot_kinds_and_actions() {
    let home = Path::new("/home/me");
    let m = Measurement {
        at: 1_800_000_000,
        unscanned: vec![Candidate { path: "/home/me/archive".into(), bytes: Some(11_200_000_000), ext: None }],
        unknown: vec![Candidate { path: "/home/me/tools/sim-data".into(), bytes: Some(4_200_000_000), ext: Some("dat".into()) }],
        ..Default::default()
    };
    let errors = vec![
        DetectorError { detector: "docker.build_cache".into(), kind: "unavailable".into(), message: "Docker is not running.".into() },
        DetectorError { detector: "docker.volumes".into(), kind: "unavailable".into(), message: "Docker is not running.".into() },
    ];
    let d = HashMap::new();
    let spots = blind_spots(&spot_input(Some(&m), &errors, &d, home));
    assert_eq!(spots.len(), 3, "one spot per tool family: {spots:#?}");
    let un = &spots[0];
    assert_eq!(un.kind, BlindSpotKind::UnscannedFolder);
    assert_eq!(un.text, "~/archive holds 11.2 GB that no detector looked at.");
    assert_eq!(un.action_label, "Scan this folder");
    let mys = &spots[1];
    assert_eq!(mys.text, "~/tools/sim-data has 4.2 GB of .dat files from an unknown tool.");
    assert_eq!(mys.action, BlindSpotAction::MarkMine);
    let tool = &spots[2];
    assert_eq!(tool.kind, BlindSpotKind::ToolUnavailable);
    assert_eq!(tool.text, "Docker is not running. Start the Docker service, then scan again.");
    assert_eq!(tool.action, BlindSpotAction::ScanAgain);
    // Excluded or scanned folders are not unscanned blind spots.
    let ex = vec![PathBuf::from("/home/me/archive")];
    let mut i = spot_input(Some(&m), &[], &d, home);
    i.exclusions = &ex;
    assert!(!blind_spots(&i).iter().any(|s| s.kind == BlindSpotKind::UnscannedFolder));
}

#[test]
fn marking_unknown_files_as_mine_counts_them_under_your_files() {
    let home = Path::new("/home/me");
    let m = Measurement { at: 1_800_000_000, unknown: vec![Candidate { path: "/home/me/tools/sim-data".into(), bytes: Some(4_200_000_000), ext: None }], ..Default::default() };
    let native = HashMap::new();
    let none = HashMap::new();
    let before = build(&input(100 * GB, &[], &native, Some(&m), &none));
    let id = spot_id(BlindSpotKind::UnknownFiles, "/home/me/tools/sim-data");
    let mine: HashMap<String, SpotDecision> = [(
        id.clone(),
        SpotDecision { decision: "mine".into(), at: 1_800_000_050, kind: Some(BlindSpotKind::UnknownFiles), title: "Files I do not recognize".into(), location: "~/tools/sim-data".into(), path: "/home/me/tools/sim-data".into(), bytes: Some(4_200_000_000) },
    )]
    .into();
    let after = build(&input(100 * GB, &[], &native, Some(&m), &mine));
    assert_eq!(area(&after, "files").bytes, area(&before, "files").bytes + 4_200_000_000);
    assert_eq!(after.not_explained_bytes + 4_200_000_000, before.not_explained_bytes, "coverage rises");
    let spots = blind_spots(&spot_input(Some(&m), &[], &mine, home));
    assert_eq!(spots.len(), 1);
    assert_eq!(spots[0].state, SpotState::Done);
    assert_eq!(spots[0].done_text.as_deref(), Some("Marked as yours. I will not ask again."));
    // Not raised again, even after the "done" note has gone.
    let mut later = spot_input(Some(&m), &[], &mine, home);
    later.now += 30 * 86_400;
    assert!(blind_spots(&later).is_empty());
}

#[test]
fn permission_spot_stays_open_until_access_is_really_granted() {
    let home = Path::new("/Users/me");
    let m = Measurement {
        at: 1_800_000_000,
        full_disk_access: Some(false),
        denied: vec![Denial { path: "/Users/me/Library/Containers".into(), kind: size::Denied::Privacy, own: true }],
        ..Default::default()
    };
    let id = spot_id(BlindSpotKind::NeedsPermission, "privacy");
    let asked: HashMap<String, SpotDecision> = [(
        id.clone(),
        SpotDecision { decision: "asked".into(), at: 1_800_000_050, kind: Some(BlindSpotKind::NeedsPermission), title: "A place I am not allowed to read".into(), location: "~/Library/Containers".into(), path: String::new(), bytes: None },
    )]
    .into();
    let mut i = spot_input(Some(&m), &[], &asked, home);
    i.platform = "macos";
    i.full_disk_access = Some(false);
    let s = blind_spots(&i);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].state, SpotState::Open, "opening System Settings resolves nothing by itself");
    assert_eq!(s[0].action, BlindSpotAction::OpenSettings);
    // Access granted (checked live): offer a scan, still open.
    i.full_disk_access = Some(true);
    let s = blind_spots(&i);
    assert_eq!(s[0].state, SpotState::Open);
    assert_eq!(s[0].action, BlindSpotAction::ScanAgain);
    // A later measurement with access reads the place: done.
    let m2 = Measurement { at: 1_800_000_100, full_disk_access: Some(true), ..Default::default() };
    let mut j = spot_input(Some(&m2), &[], &asked, home);
    j.platform = "macos";
    let s = blind_spots(&j);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].state, SpotState::Done);
}

#[test]
fn places_checked_never_show_unavailable_as_clean() {
    let findings = vec![native_finding("d1", "docker", 5 * GB)];
    let mut f2 = findings.clone();
    f2[0].suggestion.detector = "cache.pip".into();
    let errors = vec![DetectorError { detector: "docker.volumes".into(), kind: "unavailable".into(), message: "Docker is not running.".into() }];
    let places = merge_places(vec![], &["cache.pip", "cache.npm", "docker.volumes"], &f2, &errors);
    let by = |id: &str| places.iter().find(|p| p.detector == id).unwrap().state;
    assert_eq!(by("cache.pip"), PlaceState::Found);
    assert_eq!(by("cache.npm"), PlaceState::Clean);
    assert_eq!(by("docker.volumes"), PlaceState::Unavailable);
    assert_eq!(places.iter().find(|p| p.detector == "cache.pip").unwrap().label, "pip cache");
    // A later scan that did not run Docker keeps its last (unavailable) state.
    let again = merge_places(places, &["cache.npm"], &[], &[]);
    assert_eq!(again.iter().find(|p| p.detector == "docker.volumes").unwrap().state, PlaceState::Unavailable);
}

#[test]
fn growth_lists_the_fastest_growing_areas_from_daily_snapshots() {
    let day = |n: i64, docker: f64, ai: f64, caches: f64| {
        let g = |x: f64| (x * 1e9) as u64;
        (n, DaySnapshot { used_bytes: 0, not_explained_bytes: 0, areas: [("docker".into(), g(docker)), ("ai".into(), g(ai)), ("caches".into(), g(caches))].into() })
    };
    let days = vec![day(100, 62.7, 54.6, 60.0), day(103, 66.0, 58.0, 59.0), day(107, 72.0, 61.6, 58.0)];
    let g = growth(&days, 7, "macos");
    assert_eq!(g.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["docker", "ai"], "shrinking areas are not growing");
    assert_eq!(g[0].bytes, 9_300_000_000);
    assert_eq!(g[1].bytes, 7_000_000_000);
    assert_eq!(g[1].label, "AI models");
    assert!(growth(&days[2..], 7, "macos").is_empty(), "one day is not a trend");
}

#[test]
fn report_has_sizes_names_and_risks() {
    let native = HashMap::new();
    let d = HashMap::new();
    let map = build(&input(400 * GB, &[], &native, None, &d));
    let s = vec![sugg("x", "caches", Group::Caches, 2_600_000_000)];
    let r = report(&ReportInput { total_bytes: 512 * GB, free_bytes: 12_400_000_000, platform: "macos", map: &map, spots: &[], suggestions: &s });
    assert_eq!(r["disk_gb"], 512.0);
    assert_eq!(r["free_gb"], 12.4);
    assert_eq!(r["suggestions"][0]["gb"], 2.6);
    assert_eq!(r["suggestions"][0]["risk"], "Safe");
    assert_eq!(r["suggestions"][0]["where"], "~/x");
    assert_eq!(r["areas"].as_array().unwrap().last().unwrap()["name"], "Not explained");
    assert_eq!(r["folders_measured"], false);
}

// ----- through the core -------------------------------------------------------------

mod core {
    use super::*;
    use crate::app_core::tests::core_with;
    use crate::app_core::{AppCore, VolumeId};
    use std::sync::Arc;

    fn wait_idle(core: &Arc<AppCore>) {
        for _ in 0..600 {
            if !core.is_busy() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("scan did not finish");
    }

    fn deep(core: &Arc<AppCore>) {
        core.run_scan_blocking("deep-1", ScanKind::Deep);
    }

    fn setup() -> (Fixture, Arc<AppCore>, Arc<MemorySink>) {
        let fx = fixture();
        let (core, _free, _clock, sink) = core_with(50, &fx.home);
        core.set_blind_spot_min(1_000_000);
        core.add_folder(crate::app_core::FolderList::ScanFolders, &fx.home.join("projects")).unwrap();
        (fx, core, sink)
    }

    #[test]
    fn deep_scan_measures_and_emits_the_map() {
        let (_fx, core, sink) = setup();
        let before = core.get_disk_map(VolumeId::Home).unwrap();
        assert!(before.measured_at.is_none());
        deep(&core);
        let map = core.get_disk_map(VolumeId::Home).unwrap();
        assert!(map.measured_at.is_some());
        assert!(area(&map, "ai").bytes > 0);
        assert_eq!(sink.events(EV_DISK_MAP_CHANGED).len(), 1);
        assert_eq!(sink.events(EV_BLIND_SPOTS_CHANGED).len(), 1);
        // A quick scan reuses the measurement.
        let stored = core.store.meta_get("disk_measurement");
        core.run_scan_blocking("quick-1", ScanKind::Quick);
        assert_eq!(core.store.meta_get("disk_measurement"), stored, "quick scans never re-measure");
        assert_eq!(area(&core.get_disk_map(VolumeId::Home).unwrap(), "ai").bytes, area(&map, "ai").bytes);
        assert_eq!(sink.events(EV_DISK_MAP_CHANGED).len(), 2);
        // Today's totals are stored for growth.
        assert_eq!(core.store.map_days("home", 0).unwrap().len(), 1);
    }

    #[test]
    fn scan_this_folder_adds_it_and_runs_a_deep_scan() {
        let (fx, core, _sink) = setup();
        deep(&core);
        let spots = core.get_blind_spots(VolumeId::Home).unwrap();
        let archive = spots.iter().find(|s| s.kind == BlindSpotKind::UnscannedFolder && s.location.ends_with("archive")).expect("archive spot").clone();
        let before = core.get_disk_map(VolumeId::Home).unwrap();
        // Wrong action for the spot is refused.
        assert_eq!(core.resolve_blind_spot(&archive.id, BlindSpotAction::MarkMine).unwrap_err().code, "invalid_action");
        core.resolve_blind_spot(&archive.id, BlindSpotAction::ScanFolder).unwrap();
        wait_idle(&core);
        let s = core.settings();
        assert!(s.scan_folders.iter().any(|f| f.ends_with("archive")), "{:?}", s.scan_folders);
        let after = core.get_disk_map(VolumeId::Home).unwrap();
        assert!(after.not_explained_bytes + 1_000_000 <= before.not_explained_bytes, "coverage rises: {} -> {}", before.not_explained_bytes, after.not_explained_bytes);
        let spots = core.get_blind_spots(VolumeId::Home).unwrap();
        let done = spots.iter().find(|s| s.id == archive.id).expect("shown as done");
        assert_eq!(done.state, SpotState::Done);
        assert!(done.done_text.as_deref().unwrap().starts_with("Scanned."));
        drop(fx);
    }

    #[test]
    fn mark_as_mine_resolves_and_raises_coverage() {
        let (_fx, core, _sink) = setup();
        deep(&core);
        let spot = core.get_blind_spots(VolumeId::Home).unwrap().into_iter().find(|s| s.kind == BlindSpotKind::UnknownFiles).expect("unknown files spot");
        let before = core.get_disk_map(VolumeId::Home).unwrap();
        let r = core.resolve_blind_spot(&spot.id, BlindSpotAction::MarkMine).unwrap();
        assert_eq!(area(&r.map, "files").bytes, area(&before, "files").bytes + spot.size_bytes.unwrap());
        assert!(r.blind_spots.iter().any(|s| s.id == spot.id && s.state == SpotState::Done));
        // Still resolved after the next deep scan.
        deep(&core);
        assert!(!core.get_blind_spots(VolumeId::Home).unwrap().iter().any(|s| s.id == spot.id && s.state == SpotState::Open));
        // IDs only: anything else is refused.
        assert_eq!(core.resolve_blind_spot("../../etc", BlindSpotAction::MarkMine).unwrap_err().code, "invalid_id");
        assert_eq!(core.resolve_blind_spot("deadbeef", BlindSpotAction::MarkMine).unwrap_err().code, "unknown_id");
    }

    #[test]
    fn disk_report_never_contains_file_contents() {
        let (fx, core, _sink) = setup();
        deep(&core);
        let r = serde_json::to_string(&core.create_disk_report().unwrap()).unwrap();
        assert!(!r.contains("FREEBOARD-SECRET-MARKER"), "report includes file contents: {r}");
        // It does say what it could not explain, by folder name and size.
        assert!(r.contains("~/archive holds"), "{r}");
        assert!(r.contains("\"explained_pct\""));
        drop(fx);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn allow_access_opens_the_settings_pane_and_keeps_the_spot_open() {
        let (fx, core, _sink) = setup();
        fs::create_dir_all(fx.home.join("Library/Containers/com.example")).unwrap();
        let log = fx.home.join("open.log");
        crate::runner::fake::tool(&fx.home.join(".fake-tools"), "open", &format!(r#"for a in "$@"; do echo "$a" >> '{}'; done"#, log.display()));
        deep(&core);
        let spot = core.get_blind_spots(VolumeId::Home).unwrap().into_iter().find(|s| s.kind == BlindSpotKind::NeedsPermission).expect("permission spot");
        assert_eq!(spot.action, BlindSpotAction::OpenSettings);
        let r = core.resolve_blind_spot(&spot.id, BlindSpotAction::OpenSettings).unwrap();
        assert_eq!(fs::read_to_string(&log).unwrap().trim(), "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles");
        let still = r.blind_spots.iter().find(|s| s.id == spot.id).unwrap();
        assert_eq!(still.state, SpotState::Open, "never simulated");
        assert!(still.text.contains("I opened System Settings"));
    }
}
