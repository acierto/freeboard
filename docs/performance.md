# Performance report

Actual measurements against the source targets. Targets are not results; every row says where and how it was measured. Platforms without a measurement are listed as **not measured**.

## macOS (Apple silicon, macOS 27, 2026-11-04)

| Metric | Target | Measured | Status | How |
| --- | --- | --- | --- | --- |
| Installer size | ≤ 15 MB | 3.38 MB (`.dmg`); `.app` 6.2 MiB | met | `npx tauri build --bundles app,dmg`, `node scripts/perf/installer-size.mjs` |
| Memory hidden in tray (webview destroyed) | ≤ 40 MB | 31.2 MB average, 32.0 MB peak physical footprint over 304 s (RSS 109.9 MB, reference only) | met |  `scripts/perf/measure-process.mjs` after closing the window and waiting > 60 s (destroy logged) |
| Idle CPU over 5 minutes | < 1% | 0.02% (release, hidden, 304 s); 0.03% (release, 360 s); 0.17% (debug, 304 s) | met | same script |
| Quick scan | < 10 s | 6.9–7.8 s (4 runs) with Docker Desktop running and 356 findings (~103 GB); 7.0–7.8 s with Docker stopped | met | `cargo run --release --no-default-features --example scan_bench -- quick` (`-- each` times detectors one by one) |
| Deep scan, 1 million files on SSD | < 60 s | 12.8 s and 18.0 s (2 runs, 1,002,000 files) | met | `scan_bench make-fixture <dir> 1000000`, then `scan_bench deep <dir>` |

Quick scan before tuning was 16.3 s (missed). Two changes fixed it: detectors in a quick scan now run concurrently (each still fails independently), and the runner sets `HOMEBREW_NO_AUTO_UPDATE` and related variables so tools don't update themselves. `brew cleanup --dry-run` alone still takes about 4.7–5.8 s and is now the critical path.

With Docker Desktop running the quick scan first measured 8.6–10.4 s. Two further fixes: the five Docker detectors share one Docker snapshot (the first detector loads it while the others wait, instead of all five querying Docker), and folder-size walks inside a quick scan run serially, because detectors already run in parallel and nested walker pools competed for CPU.

### Hidden memory

Measure (decision 2026-11-04 in `docs/decisions.md`): the app's own memory as the OS reports it — physical footprint on macOS, PSS on Linux, private working set on Windows. RSS is kept for reference; it includes WebKit/AppKit framework pages that stay mapped after the webview is destroyed and are shared with other processes.

Release build, window closed, webview destroyed after 60 s (logged as `destroying hidden webview`), then `node scripts/perf/measure-process.mjs --name freeboard --seconds 300 --interval 10`: footprint 31.2 MB average, 32.0 MB peak.

## Linux (Ubuntu 22.04 ARM64 container, 2026-11-04)

Scope: macOS and Linux (Windows out of scope, decision 2026-11-04).

| Metric | Target | Measured | Status |
| --- | --- | --- | --- |
| Installer size, `.deb` | ≤ 15 MB | 3.78 MB | met |
| Installer size, AppImage | ≤ 15 MB | 80.45 MB | **missed**: it bundles WebKitGTK and its libraries |
| Hidden memory, idle CPU, scans | see above | not measured | needs the desktop VM |

Measured with `scripts/linux/verify.sh` (headless build and tests in Docker). Desktop measurements need a GNOME session; see `docs/release.md`.
