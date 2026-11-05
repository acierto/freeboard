# Decisions

Product and implementation decisions, with date and reason (source instruction 5). Sources: `docs/reference/FREEBOARD_SPEC.md` (behavior and safety) and `docs/reference/freeboard-prototype.html` (appearance). Safety invariants win over feature requirements and prototype behavior.

## 2026-11-04 — Foundation

**Core library separated from the Tauri shell.** The Rust crate builds with `--no-default-features` (no Tauri) so guard, runner, store, monitor, scan, detectors and cleanup can be tested on any CI runner without WebKitGTK/WebView2. The `desktop` feature adds `commands/`, `tray.rs` and the shell. Reason: the safety boundary must be testable in isolation, and the core must keep running without a webview.

**`app_core.rs` added to the source layout.** It owns shared state for commands, tray and background tasks. It is named `app_core` rather than `core` to avoid shadowing Rust's `core` crate. Everything else follows the source repository layout.

**Background work uses OS threads, not Tokio tasks.** Monitoring is one sample every five minutes, and scans/cleanups are long-running blocking I/O. Dedicated threads keep the dependency surface small (source instruction 3: avoid dependencies when the standard library is enough); Tauri still uses its own Tokio runtime for IPC.

**The interface never sends paths, including scan folders.** Settings updates from the interface use `UiSettingsPatch`, which has no path fields (`deny_unknown_fields`). Folders are added with `add_folder`, which opens a native picker in the backend; the chosen path never passes through the interface. Removal is by index. Reason: invariant 3 ("The frontend sends only plan and suggestion IDs. It never sends a path"). The dialog plugin is registered in Rust only; the interface has no `dialog:` permission.

**Additional commands beyond the source table.** `get_plan`, `get_run`, `get_trash`, `add_folder`, `remove_folder` and `set_notifications_paused` were added. They take IDs, enums, or numbers only. `get_trash` returns the opaque confirmation token required by `empty_freeboard_trash`.

**IDs are validated before lookup.** Any ID that is not a short token of `[A-Za-z0-9._-]` (no `..`) is rejected with `invalid_id` before reaching the engine (`app_core::validate_id`).

**Protected locations.** The guard rejects (after canonicalizing the parent and never resolving the final component): filesystem roots; home and its ancestors; `Desktop`, `Documents`, `Downloads`, `Library`, `Pictures`, `Music`, `Movies`, `Videos`, `.ssh`, `.config` in home (the folders themselves, not their contents, except `.ssh` which is protected entirely); configured scan folders and the app data directory (themselves); system subtrees (`/System`, `/usr`, `/bin`, `/sbin`, `/etc`, `/Library`, `/Applications`, `/opt`, `/dev`, and on Linux `/lib*`, `/boot`, `/proc`, `/sys`, `/snap`, `/srv`, `/root`, `/var/lib`, `/var/log`; on Windows `Windows`, `Program Files*`, `ProgramData`, `$Recycle.Bin`); `~/Applications`; any `*.app` bundle; any path with a `.git` component; any directory that contains a `.git` entry (repository roots, including linked worktree roots). A tree being deleted that contains a `.git` entry anywhere is refused before anything is removed.

**Commands.** The runner allows only `docker`, `git`, `pnpm`, `pip`, `pip3`, `brew`, `xcrun`, `npm`, `lsof` and `ps`, resolved on `PATH` (plus the standard Homebrew/Docker Desktop locations on macOS, because GUI apps start with a minimal `PATH`). Scans use a read-only subcommand allowlist; cleanups use an action allowlist and require a `Confirmed` token that only the cleanup engine can create. Banned commands (invariant 7) and force flags (except `docker builder prune` / `docker image prune` after confirmation) are refused under both policies. Output is capped at 8 MB; the default timeout is 20 seconds.

**Fake tools in tests use `/bin/sh` scripts.** Production never starts a shell. The test helper `runner::fake::tool` writes executable scripts that stand in for `docker`, `git` and others.

## 2026-11-04 — Monitoring

**Status hysteresis.** Worsening applies immediately. Leaving Critical needs `critical + 2 GB`; leaving Low needs `low + 2 GB`. A jump from Critical straight past `low + 2 GB` goes to Healthy. Threshold changes in Settings apply immediately (hysteresis protects only against noise in measured free space).

**Notifications.** One notification on entering Low from Healthy and one on entering Critical, then a reminder every 6 hours while Critical is unchanged. Improving from Critical to Low does not notify (it is good news, and "Crossing Low or Critical sends exactly one notification" is about getting worse). Paused or disabled notifications are dropped, not replayed. The last status is persisted so a restart does not notify again.

**Pause Notifications** is a toggle that persists until turned off.

**Notification click.** Desktop notification plugins do not report clicks reliably on all platforms. A notification sets a pending view; the next time the app is activated (tray Open, macOS Reopen, or window focus) the shell emits `navigate` to Suggestions. Native click behavior must be checked manually per platform (tasks 2.3).

**Forecast.** Ordinary least squares over the last 14 days of samples (hourly persisted samples plus the current reading). Requires samples spanning at least 3 days. A trend shallower than −0.1 GB/day counts as flat. Cleanup jumps are excluded by removing positive steps that span a Freeboard cleanup or Trash emptying, and any rise of 1 GB or more between consecutive samples (cleanups made outside Freeboard).

**Trend.** One point per day (last sample of each day) for 14 days, plus the current reading, matching the prototype's chart.

**Volumes.** The home volume (longest mount point containing the canonical home path) is always monitored; additional fixed volumes come from Settings. WSL mounts are never included.

## 2026-11-04 — Background lifecycle

Closing the window hides it; a timer destroys the webview after 60 seconds hidden (generation counter cancels the timer when reopened). `RunEvent::ExitRequested` without an exit code is prevented so the app stays in the tray; only the tray's Quit exits. Reopening recreates the webview, which reloads all state from the backend. Login startup is registered only from release builds.

## 2026-11-05 — Docker 29 image listing

Docker 29 leaves untagged images out of a plain `docker image ls`. Freeboard now also runs `docker image ls --filter dangling=true` (read-only, already allowed) and merges the two lists. `--all` is not used because it also lists intermediate build layers, which are not dangling. The test fake behaves like Docker 29 and answers `image inspect` only for the requested IDs, so the test fails without the fix.

## 2026-11-05 — Public preview downloads (requested by the user)

A download page (`site/`, GitHub Pages) and preview installers on GitHub Releases, built by `.github/workflows/release.yml` from a `v*` tag: a universal macOS `.dmg` (ad-hoc signed, not notarized) and Linux `.deb`/AppImage for x86_64 and arm64. Releases are labeled preview and the page explains the macOS "Open Anyway" step. This relaxes the earlier rule that nothing ships before signed artifacts exist; signed, notarized builds remain open (task 5.7). The page uses the prototype-derived fixture screenshots, not captures of a real machine.

## 2026-11-05 — macOS free space excludes purgeable space

The headline free space on macOS is now what the volume has free right now (`statvfs`, the figure `df` shows). It was `sysinfo`'s figure, which on macOS is the important-usage capacity: free plus purgeable space, the figure Finder shows. Purgeable space was then computed as that capacity minus itself, so it was always near zero and never reported, against the platform note "Report APFS purgeable space and local snapshots separately". Found by the user comparing the app (174.4 GB) with `df -h` (116 GiB = 124.6 GB). Thresholds and notifications use the same, more conservative figure.

## 2026-11-05 — Visual verification in CI compares against the prototype rendered on the runner

The stored baselines were rendered on the development Mac (macOS 27). GitHub runners rasterize text differently (about 20,000 differing pixels per capture on Ubuntu, about 1,000 on macOS 15), so the "baseline is still what the prototype renders" check failed for all 199 captures; CI had never run before 2026-11-05, so this had not been seen. CI now runs `verify:visual -- --live` and `verify:visual:selftest -- --live`: the app and the prototype are both rendered on the same runner and compared with the same zero tolerance. The prototype SHA-256 check still applies. The stored baselines are not changed or regenerated in CI; on the machine that made them, `npm run verify:visual` still checks them.

## Source conflicts

**Repository roots vs. linked worktree removal (task 4.1, decided 2026-11-04 by the user).** The source protects repository roots (invariant 5) but also lists `git worktree remove <path>` for linked worktrees, whose directories are themselves repository roots. Decision: linked worktree roots are the single exception. Only clean Review worktrees (merged, or pushed but not merged) can be removed; removal uses plain `git worktree remove` (never `--force`) after the pre-cleanup probe and `Guard::check_linked_worktree`, which accepts only a non-link directory whose `.git` file points into the main repository's `.git/worktrees/`. The main worktree, `.git` and all other repository roots stay protected. Details and tests: `docs/cleanup.md`.

**npm cache.** `npm cache clean` requires `--force`, which is not an allowed exception, so `cache.npm` removes `_cacache` through the guarded delete path (source detector note).

## 2026-11-04 — Release decisions (task 5.6, decided by the user)

- **License: MIT.** `LICENSE` at the repository root; `license = "MIT"` in `Cargo.toml` and `package.json`; the installers include the license file. Bundled fonts keep their SIL Open Font Licenses (`src/fonts/`).
- **Release order: all three platforms together.** No platform ships a signed release until signed artifacts and acceptance evidence exist for macOS, Linux and Windows (task 5.7). *Superseded 2026-11-04 by the user: v1 is scoped to macOS and Linux, released together; Windows is out of scope for now (`docs/release.md`).*
- **Product name: Freeboard is final.** Domain, package-registry and trademark checks remain the owner's responsibility before public distribution.

## 2026-11-04 — Memory target measure (task 5.4, decided by the user)

The 40 MB hidden-in-tray target is measured as the app's own memory as the OS reports it: physical footprint on macOS (`footprint`, Activity Monitor's Memory column), proportional set size (PSS) on Linux, and private working set on Windows. RSS is recorded for reference only, because it counts system-framework pages (WebKit, AppKit) that stay mapped after the webview is destroyed and are shared with other processes. `scripts/perf/measure-process.mjs` reports this measure per platform, and reports "unknown" (never a pass) when it cannot be read.

## 2026-11-05 — Tray mode and single instance (decided by the user for Linux memory)

- **Tray mode (`--tray`).** Freeboard can start with only the tray and monitoring, without creating a window (the configured window is removed before start-up). Login startup uses it on both platforms, so logging in no longer opens a window.
- **Linux: restart into tray mode.** WebKitGTK and its libraries stay resident in a process that has shown a window (measured 100 MB PSS hidden). 60 s after the window is hidden, Freeboard starts `freeboard --tray --after-pid <pid>` and exits; the new copy waits for the old one to exit before starting. It never restarts while a scan or cleanup runs (retries every 30 s). All state it needs is already in SQLite; in-memory five-minute samples of the last day are not carried over (hourly samples are). macOS keeps destroying the webview instead (31 MB footprint already meets the target).
- **Single instance** (`tauri-plugin-single-instance`). A second launch hands over to the running copy, which opens its window. Needed once the app can run with no window: launching it from the app menu must not start a second tray icon.
- **Linux notifications** are shown through the desktop notification service directly (`notify-rust`, already a dependency of the notification plugin) so a click opens Suggestions; the plugin does not report clicks on Linux.

## Visual baselines

- 2026-11-04: Baselines regenerated from `docs/reference/freeboard-prototype.html` (SHA-256 `62c25122e55f…`, chromium 156.0.8078.4, 191 captures). Rationale: Initial reference-derived baselines captured from the supplied prototype; supplemental states are derived from its own components as documented per entry in the manifest. Rendering uses `--disable-partial-raster` with zero pixel tolerance (experiment in `docs/visual-verification.md`). *(Entry restored from `visual/baseline/manifest.json` after an editing error removed the original line.)*
- 2026-11-04: Baselines regenerated from `docs/reference/freeboard-prototype.html` (SHA-256 `62c25122e55f…`, chromium 156.0.8078.4, 191 captures). Rationale: Suggestions empty state (supplemental) gains Scan now and Scan folders now buttons built from the prototype's .actions/.btn components, after a user found the empty screen had no way to start a scan (2026-11-04).
- 2026-11-05: Baselines regenerated from `docs/reference/freeboard-prototype.html` (SHA-256 `62c25122e55f…`, chromium 156.0.8078.4, 195 captures). Rationale: Suggestions: detector-unavailable note shown once per cause (new supplemental state, found on the real app where five Docker detectors repeated it); empty-state button renamed to 'Run deep scan now' to match Settings (2026-11-05).
- 2026-11-05: Baselines regenerated from `docs/reference/freeboard-prototype.html` (SHA-256 `62c25122e55f…`, chromium 156.0.8078.4, 195 captures). Rationale: Correct the docker-unavailable supplemental reference: the Docker group is absent when Docker is unavailable, matching the backend (2026-11-05).
- 2026-11-05: Baselines regenerated from `docs/reference/freeboard-prototype.html` (SHA-256 `62c25122e55f…`, chromium 156.0.8078.4, 199 captures). Rationale: Suggestions: distinguish 'never scanned' from 'scan found nothing' (found on the Linux VM); new supplemental state suggestions-nothing-found (2026-11-05).
