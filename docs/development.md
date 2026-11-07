# Development guide

How to build, run, test and contribute to Freeboard.

## Stack

| Layer | Technology | Lives in |
| --- | --- | --- |
| Core: monitoring, scanning, cleanup, storage | Rust (library `freeboard_lib`) | `src-tauri/src/` |
| Desktop shell: window, tray, notifications, autostart | Tauri 2 (the `desktop` Cargo feature) | `src-tauri/src/lib.rs`, `tray.rs` |
| Interface | React 19, TypeScript, Vite | `src/` |
| Storage | SQLite (bundled `rusqlite`) | `src-tauri/src/store.rs` |

The React interface never sees paths or runs commands. It displays results and sends IDs back; the Rust core validates them. See [Architecture](architecture.md) for the full safety boundary.

## Prerequisites

- Rust stable (1.80 or newer) and Node 24.
- Linux only: `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf`.
- Git with `user.name` and `user.email` set, because tests create fixture repositories.

## Run

```bash
npm ci
npx tauri dev          # app with hot reload
npm run dev            # interface only, in a browser
```

## Build

```bash
npm run build          # type check + production frontend
npx tauri build        # unsigned installers for this OS
```

Signing, notarization and publishing are covered in [Release builds](release.md).

## Test and verify

| What | Command |
| --- | --- |
| Core tests (guard, runner, store, monitor, detectors, cleanup, contract) | `cd src-tauri && cargo test --no-default-features` |
| Shell build and tests | `cd src-tauri && cargo test` |
| Type check | `npm run typecheck` |
| UI unit tests | `npm run test:ui` |
| UI behavior in a browser | `npm run verify:ui` |
| Accessibility (axe) | `npm run verify:a11y` |
| Visual comparison with the prototype | `npm run verify:visual` (report in `visual/report/report.html`) |
| Visual tool self-test | `npm run verify:visual:selftest` |
| Installer size | `node scripts/perf/installer-size.mjs` |
| Linux build and tests in Docker | `scripts/linux/verify.sh` (output in `linux-evidence/`) |

CI (`.github/workflows/ci.yml`) runs the core tests on macOS, Ubuntu 22.04 and Windows, then builds the desktop app on each.

### Tests never touch your machine

Scan and cleanup tests use temporary directories, temporary Git repositories and fake tool executables that simulate `docker`, `pnpm`, `pip`, `brew` and `xcrun`. Prune, remove and delete commands are never run on a real machine during development or tests. When developing a detector you may run an installed tool's **read-only** commands (`docker system df`, `git worktree list`) to check output formats.

## Project layout

```
src/                  React screens and components (home, questions, map, logbook, settings, tray)
  lib/types.ts        typed contract mirroring src-tauri/src/model.rs
src-tauri/
  src/
    guard.rs          protected paths and the only filesystem mutation API
    runner.rs         the only module that starts external commands
    detectors/        docker, worktrees, projects, caches, files
    scan/             orchestration, deep walk, allocated-size measurement
    cleanup/          plans, acknowledgments, revalidation, execution, Trash, history
    monitor/          sampling, status + hysteresis, notifications, forecast
    diskmap/          disk accounting and blind spots
    selection/        the pile and its rules
  capabilities/       Tauri permissions: only Freeboard's own commands
scripts/              visual, a11y, UI, perf, Linux and Windows verification
docs/                 documentation (this folder)
site/                 the GitHub Pages website
openspec/             change proposals and specs
```

## Adding a detector

1. Implement `Detector` in `src-tauri/src/detectors/`. A detector may only read: it receives a `ScanContext` with a read-only runner and no filesystem mutation API.
2. Add a `probe` so the item can be rechecked right before cleanup. Unknown results must skip, never proceed.
3. Add it to the table in [Detectors](detectors.md) with its risk, action, platforms and tests.
4. Cover it with fixture tests. Add or extend a safety test if it touches an invariant listed in [Safety traceability](safety-traceability.md). `scripts/safety-traceability.sh` checks the mapping.

## Safety rules for contributors

The invariants in [Architecture](architecture.md#safety-boundary) win over convenience. In short: scans are read-only; only a confirmed plan can mutate; the UI sends IDs, never paths; every item is rechecked before cleanup; protected paths and symlinks are never followed; no shell, ever; Risky is never preselected; unknown is treated as less safe; nothing leaves the machine.

## The documentation site

`site/` is published to GitHub Pages by `.github/workflows/pages.yml` on every push to `main` that touches `site/**`.

- `site/index.html`: landing page and downloads.
- `site/docs/index.html`: the online documentation.

Preview locally with any static server:

```bash
npx --yes serve site
```

To enable Pages on a fork: **Settings → Pages → Source: GitHub Actions**.

## Pull requests

- Keep changes focused and run the core tests plus the checks your change touches.
- Update the docs in the same PR when behavior, settings or detectors change.
- Follow the product spec in [`reference/FREEBOARD_SPEC.md`](reference/FREEBOARD_SPEC.md); record deviations in [Decisions](decisions.md).
