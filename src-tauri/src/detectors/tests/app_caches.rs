use super::*;
use crate::detectors::app_caches::*;
use crate::detectors::{probe, Probe};

fn linux() -> Fx {
    let mut fx = Fx::new();
    fx.platform = "linux";
    fx
}

#[test]
fn jetbrains_offers_only_versions_older_than_the_newest() {
    let fx = linux();
    for v in ["IntelliJIdea2023.1", "IntelliJIdea2024.2", "PyCharm2024.1"] {
        fx.write(&format!(".cache/JetBrains/{v}/caches/x"), &[1u8; 20_000]);
    }
    // Settings of a newer PyCharm make the cached 2024.1 old, too.
    fx.mkdir(".config/JetBrains/PyCharm2024.3");
    // Not an IDE version: never offered.
    fx.write(".cache/JetBrains/Toolbox/x", &[1u8; 20_000]);
    let o = fx.scan(ScanKind::Quick, &[Box::new(JetBrains)]);
    let f = find(&o, JETBRAINS);
    let labels: Vec<&str> = f.suggestion.parts.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels.len(), 2, "{labels:?}");
    assert!(labels.contains(&"IntelliJIdea2023.1") && labels.contains(&"PyCharm2024.1"));
    assert_eq!((f.suggestion.risk, f.suggestion.action), (Risk::Safe, Action::Delete));
    assert!(f.paths().iter().all(|p| !p.contains("2024.2") && !p.contains("Toolbox")));
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(&f.parts[0], ctx), Probe::Unchanged));
}

#[test]
fn jetbrains_with_one_version_per_product_offers_nothing() {
    let fx = linux();
    fx.write(".cache/JetBrains/GoLand2024.2/caches/x", &[1u8; 20_000]);
    assert!(fx.scan(ScanKind::Quick, &[Box::new(JetBrains)]).findings.is_empty());
}

#[test]
fn electron_caches_only_while_the_app_is_closed() {
    let fx = linux();
    fx.write(".config/Slack/Cache/Cache_Data/a", &[1u8; 30_000]);
    fx.write(".config/Slack/Code Cache/js/b", &[1u8; 30_000]);
    fx.write(".config/discord/GPUCache/c", &[1u8; 30_000]);
    // Settings and messages are never part of the item.
    fx.write(".config/Slack/storage/root-state.json", &[1u8; 30_000]);
    let o = fx.scan(ScanKind::Quick, &[Box::new(ElectronApps)]);
    let f = find(&o, ELECTRON);
    assert_eq!(f.suggestion.parts.len(), 2);
    assert!(f.suggestion.blocked_reason.is_none());
    assert!(f.paths().iter().all(|p| !p.contains("storage")));
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(&f.parts[0], ctx), Probe::Unchanged));

    // Slack opens: it becomes its own blocked item; Discord can still go.
    fx.tool("ps", "echo '/usr/lib/slack/slack --enable-crashpad'");
    let slack_part = f.parts.iter().find(|p| p.paths()[0].contains("Slack")).unwrap();
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(slack_part, ctx), Probe::Changed(_))));
    let o = fx.scan(ScanKind::Quick, &[Box::new(ElectronApps)]);
    assert_eq!(o.findings.len(), 2);
    let blocked: Vec<_> = o.findings.iter().filter(|f| f.suggestion.blocked_reason.is_some()).collect();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].suggestion.title, "Slack cache");
}

#[test]
fn electron_unknown_process_state_blocks() {
    let fx = linux();
    fx.write(".config/discord/Cache/a", &[1u8; 30_000]);
    fx.tool("ps", "exit 1");
    let o = fx.scan(ScanKind::Quick, &[Box::new(ElectronApps)]);
    assert!(find(&o, ELECTRON).suggestion.blocked_reason.as_deref().unwrap().contains("couldn't check"));
}

#[test]
fn playwright_keeps_what_projects_need_and_the_newest() {
    let fx = linux();
    for b in ["chromium-1100", "chromium-1140", "chromium-1150", "firefox-1400", "ffmpeg-1010"] {
        fx.write(&format!(".cache/ms-playwright/{b}/bin"), &[1u8; 40_000]);
    }
    fx.write("projects/web/package.json", b"{}");
    fx.write("projects/web/package-lock.json", b"{}");
    fx.write(
        "projects/web/node_modules/playwright-core/browsers.json",
        br#"{"browsers":[{"name":"chromium","revision":"1140"},{"name":"ffmpeg","revision":"1010"}]}"#,
    );
    let o = fx.scan(ScanKind::Deep, &[Box::new(Playwright)]);
    let f = find(&o, PLAYWRIGHT);
    // chromium-1150 is the newest, 1140 is needed, firefox and ffmpeg are the
    // only versions of theirs: only chromium-1100 goes.
    assert_eq!(f.paths().len(), 1);
    assert!(f.paths()[0].ends_with("chromium-1100"));
    // Never in quick scans: they do not know the projects.
    assert!(fx.scan(ScanKind::Quick, &[Box::new(Playwright)]).findings.is_empty());
}

#[test]
fn playwright_unreadable_needs_mark_it_unavailable() {
    let fx = linux();
    for b in ["chromium-1100", "chromium-1150"] {
        fx.write(&format!(".cache/ms-playwright/{b}/bin"), &[1u8; 40_000]);
    }
    fx.write("projects/web/package.json", b"{}");
    fx.write("projects/web/node_modules/playwright-core/browsers.json", b"{not json");
    let o = fx.scan(ScanKind::Deep, &[Box::new(Playwright)]);
    assert!(o.findings.is_empty());
    assert_eq!(o.errors[0].detector, PLAYWRIGHT);
}
