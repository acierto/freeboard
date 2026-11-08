use super::*;
use crate::detectors::apps::*;
use crate::detectors::{probe, Probe};

/// An app bundle plus what a fake `mdls` reports for it (None: no
/// last-opened date).
pub fn app(fx: &Fx, dir: &str, name: &str, id: &str, days_ago: Option<i64>) -> PathBuf {
    let p = fx.home.join(dir).join(format!("{name}.app"));
    std::fs::create_dir_all(p.join("Contents/MacOS")).unwrap();
    std::fs::write(p.join("Contents/MacOS/bin"), vec![1u8; 40_000]).unwrap();
    let used = match days_ago {
        Some(d) => chrono::DateTime::from_timestamp(NOW - d * DAY, 0).unwrap().format("%Y-%m-%d %H:%M:%S +0000").to_string(),
        None => "(null)".into(),
    };
    let data = fx.root.join("mdls");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join(format!("{name}.app")), format!("kMDItemCFBundleIdentifier = \"{id}\"\nkMDItemLastUsedDate       = {used}\n")).unwrap();
    fake_mdls(fx);
    p
}

fn fake_mdls(fx: &Fx) {
    let data = fx.root.join("mdls");
    fx.tool("mdls", &format!("shift 4; for p in \"$@\"; do f='{}'/\"$(basename \"$p\")\"; [ -f \"$f\" ] || {{ echo \"$p: could not find $p.\" >&2; exit 1; }}; cat \"$f\"; done", data.display()));
}

fn mac() -> Fx {
    let mut fx = Fx::new();
    fx.platform = "macos";
    fx
}

#[test]
fn unused_apps_need_last_opened_data_and_a_year_without_use() {
    let fx = mac();
    app(&fx, "SystemApplications", "Old Editor", "com.example.editor", Some(400));
    app(&fx, "Applications", "Game", "com.example.game", Some(700));
    // Opened last month; never opened as far as macOS knows; Apple's own.
    app(&fx, "SystemApplications", "Daily", "com.example.daily", Some(30));
    app(&fx, "SystemApplications", "Unknown", "com.example.unknown", None);
    app(&fx, "SystemApplications", "Chess", "com.apple.Chess", Some(900));
    let o = fx.scan(ScanKind::Quick, &[Box::new(UnusedApps)]);
    let f = find(&o, UNUSED);
    let labels: Vec<&str> = f.suggestion.parts.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels.len(), 2, "{labels:?}");
    assert!(labels.contains(&"Old Editor") && labels.contains(&"Game"));
    assert_eq!((f.suggestion.risk, f.suggestion.action), (Risk::Review, Action::Trash));
    assert!(f.suggestion.needs_confirmation && f.parts.iter().all(|p| p.suggestion.needs_confirmation));
    let game = f.parts.iter().find(|p| p.suggestion.title == "Game").unwrap();
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(game, ctx), Probe::Unchanged));
    // Opened after the scan.
    app(&fx, "Applications", "Game", "com.example.game", Some(0));
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(game, ctx), Probe::Changed(_))));
    // Linux and Windows keep no last-opened date: nothing.
    let mut fx = fx;
    fx.platform = "linux";
    assert!(fx.scan(ScanKind::Quick, &[Box::new(UnusedApps)]).findings.is_empty());
}

#[test]
fn unused_apps_skip_links_and_running_apps() {
    let fx = mac();
    let real = app(&fx, "elsewhere", "Linked", "com.example.linked", Some(400));
    std::fs::create_dir_all(fx.home.join("Applications")).unwrap();
    std::os::unix::fs::symlink(&real, fx.home.join("Applications/Linked.app")).unwrap();
    let open = app(&fx, "Applications", "Open", "com.example.open", Some(400));
    fx.tool("ps", &format!("echo '{}/Contents/MacOS/bin'", open.display()));
    assert!(fx.scan(ScanKind::Quick, &[Box::new(UnusedApps)]).findings.is_empty());
    // ps fails: unknown means not suggested.
    fx.tool("ps", "exit 1");
    assert!(fx.scan(ScanKind::Quick, &[Box::new(UnusedApps)]).findings.is_empty());
}

#[test]
fn mdls_is_read_only_and_bounded() {
    use crate::runner::{check, Cmd, Policy};
    let ok = |a: &[&str]| check(&Cmd::new("mdls", a.iter().copied()), Policy::ReadOnly, None).is_ok();
    assert!(ok(&["-name", "kMDItemCFBundleIdentifier", "-name", "kMDItemLastUsedDate", "/Applications/A.app", "/Applications/B.app"]));
    assert!(!ok(&["-plist", "/tmp/out", "/Applications/A.app"]));
    assert!(!ok(&["-name", "kMDItemLastUsedDate"]));
    assert!(!ok(&["-name", "kMDItemLastUsedDate", "relative.app"]));
    assert!(!ok(&["-raw", "/Applications/A.app"]));
}
