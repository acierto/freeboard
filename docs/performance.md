# Performance report

Actual measurements against the source targets. Targets are not results; every row says where and how it was measured. Platforms without a measurement are listed as **not measured**.

## Rebuild, slice 3: every detector (2026-11-08, task 5.13)

Scan times with all 35 detectors of the spec's summary. macOS on the development Mac (Apple silicon, macOS 27, internal SSD). Linux in an Ubuntu 22.04 ARM64 container on the same Mac (Docker 29.8, linuxkit kernel 7.0, 10 CPUs, fixture on a Docker volume; `linux-evidence/bench/*-2026-11-08.txt`). Windows is **not measured yet**: the Windows test VM is in use for another project.

| Platform | Metric | Target | Measured | Status | How |
| --- | --- | --- | --- | --- | --- |
| macOS | Quick scan | < 10 s | 5.87 s, 6.11 s, 7.32 s (17 findings, 59.1 GB; Docker Desktop running) | met | `cargo run --release --no-default-features --example scan_bench -- quick` |
| macOS | Deep scan, 1 million files | < 60 s | 23.6 s and 25.8 s (1,000,000 files; the fixture walk plus every quick detector against this home) | met | `scan_bench make-fixture <dir> 1000000` (78 s), then `scan_bench deep <dir>` twice |
| Linux | Quick scan | < 10 s | 0.16 s, 0.08 s, 0.09 s (empty home: no findings; Docker, pnpm, pip and Homebrew not installed, reported unavailable) | met, but a lower bound: the 2026-11-04 GNOME VM with real tool data took 1.48 s | `scan_bench quick` three times |
| Linux | Deep scan, 1 million files | < 60 s | 0.99 s and 0.77 s with the fixture just made (every project active, only sources read); 2.05 s and 1.93 s with every file backdated 400 days (all 1,000 `node_modules` sized: one finding, 3.7 GB) | met | `scan_bench make-fixture /bench/fx 1000000` (1,002,000 files, 10 s), `scan_bench deep` twice, `touch -d "400 days ago"` on every file, `scan_bench deep` twice; page cache warm (a container cannot drop it) |

The deep scan is slower than the 12.8–18.0 s of 2026-11-04 because deep scans now also look for duplicates (size buckets first, so the fixture's small files are never hashed), large videos, Playwright browsers, Node versions and Rust toolchains. Four detectors report unavailable on this Mac, as they should without Full Disk Access or a backup disk: Installers in Downloads (not a scan folder), Docker's disk, device backups, and Time Machine snapshots (no backup destination).

### Installer sizes, release 1.6.0 (2026-11-08, task 6.8)

Sizes of the files attached to https://github.com/acierto/freeboard/releases/tag/v1.6.0 (`gh release view v1.6.0 --json assets`). Target ≤ 15 MB.

| File | Size | Status |
| --- | --- | --- |
| `macOS-arm64.dmg` (app 7.2 MB) | 4.25 MB | met |
| `macOS-x64.dmg` (app 8.0 MB) | 4.59 MB | met |
| `windows-x64-setup.exe` | 3.08 MB | met |
| `windows-arm64-setup.exe` | 2.77 MB | met |
| `windows-x64.msi` | 4.06 MB | met |
| `linux-x86_64.deb` | 3.60 MB | met |
| `linux-arm64.deb` | 3.52 MB | met |
| `linux-x86_64.AppImage` | 83.08 MB | **missed**: it bundles WebKitGTK and its libraries (decision 2026-11-08) |
| `linux-arm64.AppImage` | 81.06 MB | **missed**, same reason |

## Rebuild, milestone 2 (2026-11-06)

The harbor interface (openspec change `rebuild-freeboard`) replaced the hull-gauge one. Hidden memory and idle CPU were measured again on each platform (task 2.7). Scan times are measured again in milestone 3.

| Platform | Memory hidden in tray (≤ 40 MB) | Idle CPU over 5 minutes (< 1%) | How |
| --- | --- | --- | --- |
| macOS 27, Apple silicon | 21 MB average and peak physical footprint over 304 s (RSS 108.1 MB, reference only): **met** | 0.053%: **met** | Release build; tray view opened and closed; main window closed; `destroying hidden webview` logged at 14:12:24Z; then `node scripts/perf/measure-process.mjs --name freeboard --seconds 300 --interval 10` |
| Linux, Ubuntu 24.04 GNOME (X11), ARM64 VM | 22.9 MB average and peak PSS over 305 s (RSS 52 MB, reference only): **met** | 0.328%: **met** | `.deb` from the release workflow (run 42232860033); main window closed with Alt+F4; Freeboard restarted itself as `freeboard --tray --after-pid <pid>` after 60 s; then `node measure-process.mjs --name freeboard --seconds 300 --interval 10` inside the VM |
| Windows 11 (Arm64 VM) | not measured yet | not measured yet | VM being rebuilt |

## macOS (Apple silicon, macOS 27, 2026-11-04)

| Metric | Target | Measured | Status | How |
| --- | --- | --- | --- | --- |
| Installer size | ≤ 15 MB | 3.38 MB (`.dmg`); `.app` 6.2 MiB | met | `npx tauri build --bundles app,dmg`, `node scripts/perf/installer-size.mjs` |
| Memory hidden in tray (webview destroyed) | ≤ 40 MB | 31.2 MB average, 32.0 MB peak physical footprint over 304 s (RSS 109.9 MB, reference only) | met |  `scripts/perf/measure-process.mjs` after closing the window and waiting > 60 s (destroy logged) |
| Idle CPU over 5 minutes | < 1% | 0.02% (release, hidden, 304 s); 0.03% (release, 360 s); 0.17% (debug, 304 s) | met | same script |
| Quick scan | < 10 s | 5.8–6.2 s (2026-11-05, Docker 29.8, 363 findings, ~113 GB) after one slow first run of 18.6 s right after a release build; earlier 6.9–7.8 s (4 runs) with Docker Desktop running and 356 findings (~103 GB); 7.0–7.8 s with Docker stopped | met | `cargo run --release --no-default-features --example scan_bench -- quick` (`-- each` times detectors one by one) |
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
| Hidden memory (PSS, Freeboard and its child processes) | ≤ 40 MB | 22.8 MB in tray mode (after the automatic restart); 100 MB before the change (81.5 MB main + 18.5 MB WebKitNetworkProcess) | met |
| Idle CPU over 5 minutes | < 1% | 0.026% (tray mode); 0.07% before | met |
| Quick scan | < 10 s | 0.39 s (4 runs, GNOME VM with git, npm, pip and Docker 29 data; no Homebrew or pnpm); 1.48 s after adding the dangling-image query | met |
| Deep scan, 1 million files | < 60 s | 2.0–2.2 s with recently changed projects (only sources are read); 6.4 s when all 1,000 projects are idle and every `node_modules` is sized (1,000 findings, 3.7 GB); disk cache dropped before each run | met |
| KDE Plasma tray mode | ≤ 40 MB, < 1% | 26.2 MB PSS; idle CPU 0.11% once settled | met |

Scan timings: `scan_bench` built in the Ubuntu 22.04 container and run in the VM (`linux-evidence/bench/`). The VM has 4 virtual CPUs on Apple silicon and a virtio disk, so these times are not comparable with a physical Linux machine.

Installer sizes from `scripts/linux/verify.sh` (headless build and tests in Docker). Memory and CPU measured 2026-11-05 in the GNOME VM (`scripts/linux/vm/`, Ubuntu 24.04.5 ARM64, software rendering): window closed, automatic restart into tray mode logged, then PSS of Freeboard and any child processes sampled every 10 s for 300 s.

macOS tray mode (login startup, no window opened yet): 11 MB footprint.
