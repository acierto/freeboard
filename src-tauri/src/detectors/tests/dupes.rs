use super::*;
use crate::detectors::dupes::*;
use crate::detectors::{probe, Probe};

fn bytes(seed: u8, n: usize) -> Vec<u8> {
    (0..n).map(|i| seed.wrapping_add((i % 251) as u8)).collect()
}

#[test]
fn duplicates_match_exact_contents_and_keep_the_oldest() {
    let fx = Fx::new();
    let a = fx.write("projects/photos/a.raw", &bytes(1, 1_500_000));
    let b = fx.write("projects/backup/a copy.raw", &bytes(1, 1_500_000));
    let c = fx.write("projects/old/a.raw", &bytes(1, 1_500_000));
    age(&c, 300);
    age(&a, 200);
    // Near duplicates: same size, different contents (late in the file, past
    // the 64 KiB head) and different heads.
    let mut late = bytes(1, 1_500_000);
    late[1_400_000] ^= 0xff;
    fx.write("projects/near/late.raw", &late);
    fx.write("projects/near/head.raw", &bytes(2, 1_500_000));
    // Hard links to one file are one file.
    let solo = fx.write("projects/links/solo.bin", &bytes(9, 1_200_000));
    std::fs::hard_link(&solo, fx.home.join("projects/links/solo-link.bin")).unwrap();
    // Excluded places: hidden folders, bundles, repositories.
    fx.write("projects/.hidden/a.raw", &bytes(1, 1_500_000));
    fx.write("projects/Tool.app/Contents/a.raw", &bytes(1, 1_500_000));
    fx.write("projects/Pictures.photoslibrary/a.raw", &bytes(1, 1_500_000));
    fx.write("projects/repo/.git/HEAD", b"ref: refs/heads/main\n");
    fx.write("projects/repo/assets/a.raw", &bytes(1, 1_500_000));
    // Too small to be worth it.
    fx.write("projects/small/x.txt", &bytes(5, 1000));
    fx.write("projects/small/y.txt", &bytes(5, 1000));

    let o = fx.scan(ScanKind::Deep, &[Box::new(Duplicates)]);
    let f = find(&o, DUPLICATES);
    // One set: the oldest copy (old/a.raw) stays, the other two go.
    let mut paths: Vec<&str> = f.paths().iter().map(|p| p.as_str()).collect();
    paths.sort();
    assert_eq!(paths, [b.to_string_lossy().as_ref(), a.to_string_lossy().as_ref()]);
    assert!(f.suggestion.last_used_label.contains("old/a.raw"), "{}", f.suggestion.last_used_label);
    assert_eq!((f.suggestion.risk, f.suggestion.action, f.suggestion.group), (Risk::Review, Action::Trash, Group::YourFiles));
    assert_eq!(f.suggestion.title, "a.raw: 2 extra copies");
    fx.with_ctx(ScanKind::Deep, |ctx| assert_eq!(probe(f, ctx), Probe::Unchanged));
    // The copy being kept changes: the others stay.
    std::fs::write(&c, bytes(7, 1_500_000)).unwrap();
    fx.with_ctx(ScanKind::Deep, |ctx| assert!(matches!(probe(f, ctx), Probe::Changed(_))));
    std::fs::remove_file(&c).unwrap();
    fx.with_ctx(ScanKind::Deep, |ctx| assert!(matches!(probe(f, ctx), Probe::Changed(_))));
}

#[test]
fn several_duplicate_sets_are_parts() {
    let fx = Fx::new();
    for d in ["x", "y"] {
        fx.write(&format!("projects/{d}/one.bin"), &bytes(1, 1_100_000));
        fx.write(&format!("projects/{d}/two.bin"), &bytes(2, 1_300_000));
    }
    let o = fx.scan(ScanKind::Deep, &[Box::new(Duplicates)]);
    let f = find(&o, DUPLICATES);
    assert_eq!(f.parts.len(), 2);
    assert_eq!(f.suggestion.title, "Duplicate copies of 2 files");
    fx.with_ctx(ScanKind::Deep, |ctx| assert_eq!(probe(&f.parts[0], ctx), Probe::Unchanged));
    assert!(fx.scan(ScanKind::Quick, &[Box::new(Duplicates)]).findings.is_empty());
}

/// A file `len` bytes long that takes almost no space (a sparse tail).
fn big(fx: &Fx, rel: &str, len: u64, days: i64) -> PathBuf {
    let p = fx.write(rel, &bytes(3, 8192));
    std::fs::File::options().write(true).open(&p).unwrap().set_len(len).unwrap();
    age(&p, days);
    p
}

#[test]
fn large_videos_only_and_not_opened_for_six_months() {
    let mut fx = Fx::new();
    fx.platform = "linux";
    big(&fx, "Videos/holiday.mp4", 600_000_000, 200);
    big(&fx, "Downloads/talk.MOV", 700_000_000, 400);
    big(&fx, "projects/clips/raw.mkv", 520_000_000, 190);
    // Watched recently, too small, not a video, hidden.
    big(&fx, "Videos/recent.mp4", 600_000_000, 30);
    big(&fx, "Videos/short.mp4", 400_000_000, 300);
    big(&fx, "Videos/disk.iso", 900_000_000, 300);
    big(&fx, "Videos/.cache/x.mp4", 900_000_000, 300);
    let o = fx.scan(ScanKind::Deep, &[Box::new(LargeMedia)]);
    let f = find(&o, LARGE_MEDIA);
    let mut labels: Vec<&str> = f.suggestion.parts.iter().map(|p| p.label.as_str()).collect();
    labels.sort();
    assert_eq!(labels, ["holiday.mp4", "raw.mkv", "talk.MOV"]);
    assert_eq!((f.suggestion.risk, f.suggestion.action), (Risk::Review, Action::Trash));
    let p = f.parts.iter().find(|p| p.suggestion.title == "holiday.mp4").unwrap();
    fx.with_ctx(ScanKind::Deep, |ctx| assert_eq!(probe(p, ctx), Probe::Unchanged));
    // Watched after the scan.
    let file = std::fs::File::options().write(true).open(fx.home.join("Videos/holiday.mp4")).unwrap();
    file.set_times(std::fs::FileTimes::new().set_accessed(UNIX_EPOCH + Duration::from_secs(NOW as u64))).unwrap();
    fx.with_ctx(ScanKind::Deep, |ctx| assert!(matches!(probe(p, ctx), Probe::Changed(_))));
}
