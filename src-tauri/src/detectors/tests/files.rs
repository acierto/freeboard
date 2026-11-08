use super::*;
use crate::detectors::files::*;
use crate::detectors::{probe, Probe};

#[test]
fn downloads_only_old_installers() {
    let fx = Fx::new();
    for n in ["a.dmg", "b.pkg", "c.exe", "d.msi", "e.AppImage", "f.deb", "g.rpm", "h.iso"] {
        fx.write(&format!("Downloads/{n}"), &vec![1u8; 4096]);
    }
    for n in ["report.pdf", "movie.mp4", "photo.jpg", "notes.docx"] {
        fx.write(&format!("Downloads/{n}"), b"doc");
    }
    age(&fx.home.join("Downloads"), 120);
    // Recently opened installer (old mtime, recent access): kept.
    let recent = fx.write("Downloads/recent.dmg", b"x");
    age(&recent, 120);
    let f = std::fs::File::options().write(true).open(&recent).unwrap();
    f.set_times(std::fs::FileTimes::new().set_accessed(UNIX_EPOCH + Duration::from_secs((NOW - 5 * DAY) as u64))).unwrap();
    // Recently downloaded installer: kept.
    let newer = fx.write("Downloads/new.dmg", b"x");
    age(&newer, 10);

    let o = fx.scan(ScanKind::Quick, &[Box::new(Installers)]);
    let f = find(&o, INSTALLERS);
    let Target::Paths { paths } = &f.target else { panic!() };
    let mut names: Vec<String> = paths.iter().map(|p| Path::new(p).file_name().unwrap().to_string_lossy().to_string()).collect();
    names.sort();
    assert_eq!(names, vec!["a.dmg", "b.pkg", "c.exe", "d.msi", "e.AppImage", "f.deb", "g.rpm", "h.iso"]);
    assert_eq!((f.suggestion.risk, f.suggestion.action), (Risk::Review, Action::Trash));
    assert!(f.suggestion.reason.starts_with("8 installers"));
    assert!(!f.suggestion.preselected());
}

#[test]
fn installer_age_follows_settings() {
    let mut fx = Fx::new();
    let p = fx.write("Downloads/a.dmg", b"x");
    age(&p, 40);
    assert!(fx.scan(ScanKind::Quick, &[Box::new(Installers)]).findings.is_empty());
    fx.settings.old_installer_days = 30;
    assert_eq!(fx.scan(ScanKind::Quick, &[Box::new(Installers)]).findings.len(), 1);
}

#[test]
fn opening_an_installer_after_the_scan_skips_it() {
    let fx = Fx::new();
    let p = fx.write("Downloads/a.dmg", b"x");
    age(&p, 120);
    let o = fx.scan(ScanKind::Quick, &[Box::new(Installers)]);
    let f = find(&o, INSTALLERS).clone();
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(&f, ctx), Probe::Unchanged));
    let file = std::fs::File::options().write(true).open(&p).unwrap();
    file.set_times(std::fs::FileTimes::new().set_accessed(UNIX_EPOCH + Duration::from_secs((NOW - 60) as u64))).unwrap();
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(&f, ctx), Probe::Changed(_))));
}

#[test]
fn windows_temp_old_entries_go_to_recycle_bin() {
    let mut fx = Fx::new();
    fx.write("AppData/Local/Temp/old.tmp", &vec![1u8; 4096]);
    fx.write("AppData/Local/Temp/olddir/x.log", b"x");
    age(&fx.home.join("AppData/Local/Temp"), 30);
    let n = fx.write("AppData/Local/Temp/new.tmp", b"x");
    age(&n, 2);
    fx.platform = "windows";
    let o = fx.scan(ScanKind::Quick, &[Box::new(WindowsTemp)]);
    let f = find(&o, WINDOWS_TEMP);
    let Target::Paths { paths } = &f.target else { panic!() };
    assert_eq!(paths.len(), 2);
    assert_eq!(f.suggestion.action_label, "Move to Recycle Bin");
    assert_eq!(f.suggestion.recovery, "Restore them from the Recycle Bin.");
    assert_eq!(f.suggestion.risk, Risk::Review);
    // Not run on other platforms.
    fx.platform = "linux";
    assert!(fx.scan(ScanKind::Quick, &[Box::new(WindowsTemp)]).findings.is_empty());
}

#[cfg(unix)]
#[test]
fn installer_links_are_ignored() {
    let fx = Fx::new();
    let target = fx.write("elsewhere/big.iso", b"x");
    age(&target, 200);
    fx.mkdir("Downloads");
    std::os::unix::fs::symlink(&target, fx.home.join("Downloads/link.iso")).unwrap();
    assert!(fx.scan(ScanKind::Quick, &[Box::new(Installers)]).findings.is_empty());
}

#[test]
fn installers_turn_old_at_90_days() {
    let fx = Fx::new();
    let a = fx.write("Downloads/a.pkg", b"x");
    let b = fx.write("Downloads/b.pkg", b"x");
    age(&a, 89);
    age(&b, 91);
    let o = fx.scan(ScanKind::Quick, &[Box::new(Installers)]);
    let paths = find(&o, INSTALLERS).paths();
    assert_eq!(paths.len(), 1);
    assert!(paths[0].ends_with("b.pkg"));
}
