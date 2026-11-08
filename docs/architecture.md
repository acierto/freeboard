# Architecture and verification

Freeboard is a Tauri 2 desktop app. A Rust core does all monitoring, scanning and cleanup; a React interface only displays results and sends IDs back. The core keeps running in the tray after the webview is destroyed.

```
src/                       React screens (home, quick questions, disk map, worktrees, logbook, settings, first run, tray view)
  lib/types.ts, api.ts     typed contract mirroring src-tauri/src/model.rs
src-tauri/
  capabilities/default.json  only core event listen + Freeboard's own commands
  permissions/app.toml       allowlist of Freeboard commands
  src/
    lib.rs        Tauri shell (desktop feature): window lifecycle, plugins, command registration
    app_core.rs   shared backend state; validates IDs; background loop
    commands/     thin #[tauri::command] handlers
    monitor/      sampling, status + hysteresis, notifications, forecast
    scan/         orchestration, deep walk, allocated-size measurement
    detectors/    docker, worktrees, projects, caches, app caches, AI models, phones and
                  emulators, tool versions, apps, duplicates and videos, system items
    diskmap/      disk accounting, blind spots, places checked, growth, the disk report
    selection/    the pile (goals, Where from) as a pure function
    rules/        Keep me tidy: rule-sourced plans through the cleanup engine
    cleanup/      plans, acknowledgments, revalidation, execution, Trash, history
    platform/     macOS / Linux / Windows differences
    runner.rs     the only module that starts external commands
    guard.rs      protected paths and the only filesystem mutation API
    store.rs      SQLite migrations and retention
    tray.rs       status ring, free space, tray menu
```

## Safety boundary

| Invariant | Where it is enforced |
| --- | --- |
| 1 Scans are read-only | Detectors get a `ScanContext` with a runner (read-only policy) and no guard |
| 2 Confirmed plan only | `runner::Confirmed` can only be created by the cleanup engine after plan validation; a Keep me tidy plan is refused unless every item and part is a Safe, unkept item of the rule's own detectors (`rules::allows`, checked when the plan is made and when it starts) |
| 3 IDs only | `UiSettingsPatch` has no paths; `validate_id`; capability allows only app commands |
| 4 Recheck before cleanup | `detectors::probe` + `Guard::check_expected` per item |
| 5 Protected paths | `Guard::protected_reason` on the canonical form; the one exception, an unused macOS app moved to the Trash, is `Guard::check_unused_app` (Trash only) |
| 6 No link following | `symlink_metadata` everywhere; links removed as links |
| 7 Banned commands | `runner::banned_reason` under every policy |
| 8 No shell | `std::process::Command` with an argument array; program names allowlisted |
| 9, 10 Risky never preselected | `Suggestion::preselected` and plan acknowledgments |
| 11 Unknown is less safe | `Risk::raised`, `Probe::Unknown` skips |
| 12 Local only | strict CSP, no network crates, no remote assets |
| 13 Records | `cleanup_runs`, `cleanup_items`, `trash_items` |

See `docs/safety-traceability.md` for the test that covers each invariant.

## Reproducing the checks

Prerequisites: Rust stable, Node 24, and on Linux `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf`. Git must have `user.name` and `user.email` set for the fixture repositories.

| Check | Command |
| --- | --- |
| Core tests (guard, runner, store, settings, monitor, detectors, cleanup, contract) | `cd src-tauri && cargo test --no-default-features` |
| Shell build and tests | `cd src-tauri && cargo test` |
| Frontend type check and build | `npm ci && npx tsc --noEmit && npm run build` |
| Visual comparison with the prototype | `npm run verify:visual` (report in `visual/report/report.html`) |
| Visual tool self-test | `npm run verify:visual:selftest` |
| Unsigned installers | `npx tauri build` |
| Installer size | `node scripts/perf/installer-size.mjs` |
| Hidden memory and idle CPU | run the app, close the window, wait > 60 s, then `node scripts/perf/measure-process.mjs --name freeboard --seconds 300` |

## Fixture-only safety policy

- Tests never run cleanup against the developer's machine. Scanning and cleanup tests create temporary directories, temporary Git repositories, and fake tool executables (`runner::fake::tool`) that simulate `docker`, `pnpm`, `pip`, `brew`, `xcrun`, `ollama`, `rustup`, `mdls`, `tmutil`, `lsof` and `ps`.
- Detector development may run the installed tools' **read-only** commands (`docker version`, `docker system df`, `git worktree list`) to validate output formats. Prune, remove and delete commands are never run on a real machine during development or tests.
- Native Trash checks (manual) use dedicated disposable files created for the check.
- Kubernetes: `kubectl exec` is never used.

## Prototype versus production

The prototype's numbers, OS switcher, free-space slider, mock menu bar, notification banner and in-memory cleanup are simulation tools. Production uses measured free space, persisted thresholds with hysteresis, a real forecast (shown only when eligible), backend-built plans, backend acknowledgments, per-item revalidation, cancellation, and measured before/after results. The prototype's visual language (hull gauge, typography, colors, spacing, themes, breakpoints) is reproduced exactly and verified by screenshot comparison.
