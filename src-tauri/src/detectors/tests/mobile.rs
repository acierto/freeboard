use super::*;
use crate::detectors::mobile::*;
use crate::detectors::{probe, Probe};

fn info(name: &str, days: i64) -> Vec<u8> {
    let d = chrono::DateTime::from_timestamp(NOW - days * DAY, 0).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    format!("<?xml version=\"1.0\"?><plist><dict><key>Device Name</key><string>{name}</string><key>Last Backup Date</key><date>{d}</date></dict></plist>").into_bytes()
}

#[test]
fn ios_backups_older_than_180_days_split_by_backup() {
    let mut fx = Fx::new();
    fx.platform = "macos";
    let root = "Library/Application Support/MobileSync/Backup";
    fx.write(&format!("{root}/00008030-AAAA/Info.plist"), &info("Old iPhone", 400));
    fx.write(&format!("{root}/00008030-AAAA/Manifest.db"), &[1u8; 50_000]);
    fx.write(&format!("{root}/00008030-BBBB/Info.plist"), &info("iPad", 181));
    fx.write(&format!("{root}/00008030-BBBB/Manifest.db"), &[1u8; 30_000]);
    // Backed up last week: kept out.
    fx.write(&format!("{root}/00008030-CCCC/Info.plist"), &info("Current iPhone", 7));
    fx.write(&format!("{root}/00008030-CCCC/Manifest.db"), &[1u8; 30_000]);
    let o = fx.scan(ScanKind::Quick, &[Box::new(IosBackups)]);
    let f = find(&o, IOS_BACKUPS);
    let labels: Vec<&str> = f.suggestion.parts.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["Old iPhone", "iPad"]);
    assert_eq!((f.suggestion.risk, f.suggestion.action), (Risk::Review, Action::Trash));
    assert!(f.parts[1].suggestion.last_used_label.contains("181 days"));
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(&f.parts[0], ctx), Probe::Unchanged));
    // Linux has no device backups.
    fx.platform = "linux";
    assert!(fx.scan(ScanKind::Quick, &[Box::new(IosBackups)]).findings.is_empty());
}

#[test]
fn android_emulators_not_started_for_90_days_and_never_while_running() {
    let fx = Fx::new();
    fx.write(".android/avd/Pixel_6_API_33.avd/userdata-qemu.img", &[1u8; 60_000]);
    fx.write(".android/avd/Pixel_6_API_33.ini", b"path=...");
    fx.write(".android/avd/Old_Tablet.avd/userdata-qemu.img", &[1u8; 40_000]);
    fx.write(".android/avd/Recent.avd/userdata-qemu.img", &[1u8; 40_000]);
    age(&fx.home.join(".android/avd/Pixel_6_API_33.avd"), 120);
    age(&fx.home.join(".android/avd/Old_Tablet.avd"), 91);
    age(&fx.home.join(".android/avd/Recent.avd"), 89);
    let o = fx.scan(ScanKind::Quick, &[Box::new(AndroidAvd)]);
    let f = find(&o, ANDROID_AVD);
    let labels: Vec<&str> = f.suggestion.parts.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["Pixel 6 API 33", "Old Tablet"]);
    // The emulator's .ini goes with its folder.
    assert!(f.parts[0].paths().iter().any(|p| p.ends_with("Pixel_6_API_33.ini")));
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(&f.parts[0], ctx), Probe::Unchanged));
    // Started after the scan: the emulator holds lock files.
    fx.write(".android/avd/Pixel_6_API_33.avd/multiinstance.lock", b"");
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(&f.parts[0], ctx), Probe::Changed(_))));
    let o = fx.scan(ScanKind::Quick, &[Box::new(AndroidAvd)]);
    assert_eq!(find(&o, ANDROID_AVD).suggestion.title, "Android emulators not started for 90+ days");
    assert!(find(&o, ANDROID_AVD).paths().iter().all(|p| !p.contains("Pixel")));
}
