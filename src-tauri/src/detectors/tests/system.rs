use super::*;
use crate::detectors::system::*;
use crate::detectors::{probe, Probe};

fn mac() -> Fx {
    let mut fx = Fx::new();
    fx.platform = "macos";
    fx
}

/// Fake tmutil: snapshots and the latest backup from files the test edits.
fn tmutil(fx: &Fx, snapshots: &[&str], latest: &str) -> (PathBuf, PathBuf) {
    let (s, l) = (fx.root.join("snaps.txt"), fx.root.join("latest.txt"));
    let list: String = snapshots.iter().map(|d| format!("com.apple.TimeMachine.{d}.local\n")).collect();
    std::fs::write(&s, format!("Snapshots for disk /:\n{list}")).unwrap();
    std::fs::write(&l, latest).unwrap();
    fx.tool("tmutil", &format!("case \"$1\" in listlocalsnapshots) cat '{}';; latestbackup) cat '{}';; *) exit 9;; esac", s.display(), l.display()));
    (s, l)
}

#[test]
fn snapshots_older_than_a_confirmed_backup_one_part_each() {
    let fx = mac();
    let (_, latest) = tmutil(&fx, &["2026-10-01-101010", "2026-10-20-090000", "2026-11-03-120000"], "/Volumes/.timemachine/X/2026-11-01-080000.backup/2026-11-01-080000.backup\n");
    let o = fx.scan(ScanKind::Quick, &[Box::new(Snapshots)]);
    let f = find(&o, SNAPSHOTS);
    // The snapshot newer than the backup stays.
    let labels: Vec<&str> = f.suggestion.parts.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["Snapshot of 2026-10-20", "Snapshot of 2026-10-01"]);
    assert!(f.suggestion.size_is_estimate);
    assert_eq!((f.suggestion.risk, f.suggestion.action), (Risk::Review, Action::NativeCommand));
    let Target::Native { args, .. } = &f.parts[1].target else { panic!() };
    assert_eq!(args, &["deletelocalsnapshots", "2026-10-01-101010"]);
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(&f.parts[1], ctx), Probe::Unchanged));
    // The backup disk is gone before cleanup: nothing is deleted.
    std::fs::write(&latest, "Failed to mount backup destination, error: Code=17\n").unwrap();
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(&f.parts[1], ctx), Probe::Unknown(_))));
}

#[test]
fn unconfirmed_backup_reports_unavailable_and_suggests_nothing() {
    let fx = mac();
    // What tmutil prints with no destination; it exits 0 (checked live).
    tmutil(&fx, &["2026-10-01-101010"], "Failed to mount backup destination, error: Error Domain=com.apple.backupd.ErrorDomain Code=17\n");
    let o = fx.scan(ScanKind::Quick, &[Box::new(Snapshots)]);
    assert!(o.findings.is_empty());
    assert_eq!((o.errors[0].detector.as_str(), o.errors[0].kind.as_str()), (SNAPSHOTS, "unavailable"));
    tmutil(&fx, &["2026-10-01-101010"], "");
    assert_eq!(fx.scan(ScanKind::Quick, &[Box::new(Snapshots)]).errors[0].kind, "unavailable");
}

#[test]
fn tmutil_deletes_one_snapshot_by_date_only() {
    use crate::runner::{check, Cmd, Confirmed, Policy};
    let ok = Confirmed::new("p");
    let act = |a: &[&str]| check(&Cmd::new("tmutil", a.iter().copied()), Policy::Action, Some(&ok)).is_ok();
    assert!(act(&["deletelocalsnapshots", "2026-10-01-101010"]));
    assert!(!act(&["deletelocalsnapshots", "/"]));
    assert!(!act(&["deletelocalsnapshots", "2026-10-01-101010", "2026-10-02-101010"]));
    assert!(!act(&["thinlocalsnapshots", "/", "999999999999"]));
    assert!(check(&Cmd::new("tmutil", ["latestbackup"]), Policy::ReadOnly, None).is_ok());
}

#[test]
fn stale_temp_leaves_files_apps_still_use() {
    let fx = mac();
    fx.write("tmp/com.vendor.tool.abc/cache.bin", &vec![1u8; 50_000]);
    fx.write("tmp/leftover.dat", &vec![1u8; 30_000]);
    fx.write("tmp/com.apple.Safari/x", &vec![1u8; 30_000]);
    fx.write("tmp/in-use/session.db", &vec![1u8; 30_000]);
    age(&fx.home.join("tmp"), 30);
    fx.write("tmp/fresh.tmp", &vec![1u8; 30_000]);
    let held = fx.home.join("tmp/in-use/session.db");
    // Like macOS's lsof: open files listed, exit 1.
    fx.tool("lsof", &format!("printf 'p42\\nn{}\\n'; exit 1", held.display()));
    let o = fx.scan(ScanKind::Quick, &[Box::new(StaleTemp)]);
    let f = find(&o, STALE_TEMP);
    let mut names: Vec<String> = f.paths().iter().map(|p| Path::new(p).file_name().unwrap().to_string_lossy().to_string()).collect();
    names.sort();
    assert_eq!(names, ["com.vendor.tool.abc", "leftover.dat"]);
    assert_eq!((f.suggestion.risk, f.suggestion.action, f.suggestion.group), (Risk::Review, Action::Trash, Group::System));
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(f, ctx), Probe::Unchanged));
    // An app opens one of them before cleanup.
    let now_held = fx.home.join("tmp/leftover.dat");
    fx.tool("lsof", &format!("printf 'p43\\nn{}\\n'", now_held.display()));
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(f, ctx), Probe::Changed(_))));
    // lsof cannot tell: unavailable, nothing suggested.
    fx.tool("lsof", "echo 'lsof: permission denied' >&2; exit 2");
    let o = fx.scan(ScanKind::Quick, &[Box::new(StaleTemp)]);
    assert!(o.findings.is_empty() && o.errors[0].detector == STALE_TEMP);
}
