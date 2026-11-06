# Proposal

## Why

The approved product changed on 2026-11-06. The new specification and prototype (`docs/reference/`) replace the hull-gauge Overview, Suggestions and Plan screens with a single harbor home built around Dinghy. They also require the app to account for all used space, not only the items it recognizes, and to let users clean either with one goal or down to individual parts. The earlier `build-freeboard` change described the old product and is superseded by this one.

## What Changes

- **BREAKING** Replace the whole interface. Remove the Overview, Suggestions, Cleanup plan, History and sidebar screens. Add one home view (Dinghy, free space, goal, Where from, the pile) with three sections, Quick questions, Disk map and Logbook, plus Settings behind a gear button and a tray view.
- **BREAKING** Change the suggestion model. Groups (Docker, Projects, Caches, AI models, Phones and emulators, Old tools and apps, Your files, System) replace the five categories. Each suggestion gains a map area, splittable parts, a full size, and Keep or Ask me later decisions.
- Add the pile: size goals (20/50/100 GB, All I can), Where from area limits, automatic filling with Safe items only, manual mode, taking crates out, and Keep.
- Add Quick questions (one Review or Risky card at a time, biggest first) and Everything I found, with per-item, per-group and per-part controls.
- Add disk accounting: a squarified treemap of all used space, a Not explained area, coverage, blind spots with working fixes, places checked (found, clean, unavailable), 7-day growth and a copyable disk report.
- Add detector families: AI models, phones and emulators, old tools and apps, duplicates, large media, Time Machine snapshots and stale temp files, Docker disk slack, plus npm, JetBrains, Electron-app and Playwright caches.
- Add Keep me tidy rules. They are off by default and limited to named Safe detectors.
- Rename History to Logbook and keep the existing safety model: confirmed plans, pre-cleanup checks, guarded runner, scoped Trash and local records.
- Replace the fonts with Fredoka, Nunito and DM Mono, bundled locally, and replace the design tokens with the prototype's light and night themes.
- Rebuild the visual verification matrix against the new prototype. Remove the old matrix and baselines.
- Keep existing Rust modules that already meet the new spec (guard, runner, store, monitor, scan and size measurement, existing detectors, cleanup, Trash, tray, platform) and extend them. Do not rewrite them.

## Capabilities

### New Capabilities

- `desktop-foundation`: local storage and retention, Tauri permissions and content policy, settings, first-run setup, tray and background lifecycle.
- `disk-monitoring`: free-space sampling, Healthy/Low/Critical status with hysteresis, notifications and forecast.
- `resource-detection`: quick and deep scans, every detector family, risk rules, the suggestion and parts model, and on-disk size measurement.
- `disk-accounting`: used-space accounting by area, Not explained, coverage, blind spots and their fixes, places checked, growth and the disk report.
- `cleanup-selection`: goals, Where from, the pile and its rules, Quick questions, item and group controls, parts, and Keep and Ask me later decisions.
- `safe-cleanup`: plans, acknowledgments, pre-cleanup checks, guarded execution, cancellation, Trash handling and the Logbook record.
- `tidy-rules`: opt-in automatic cleanup rules limited to named Safe detectors.
- `desktop-interface`: Dinghy and the harbor scene, the home view and its sections, sheets, the tray view, Settings, copy, visual system, accessibility and states the prototype lacks.
- `visual-verification`: screenshot comparison of the app against the new prototype, with evidence and a blocking fidelity gate.

### Modified Capabilities

None. `openspec/specs/` is empty because the superseded change was never archived.

## Impact

- **Frontend (`src/`)**: rebuilt under `src/features/` (home, questions, disk-map, cleanup, logbook). The current screens, components and fixture backend are replaced.
- **Rust core (`src-tauri/src/`)**: the model changes (`group`, `mapArea`, `parts`, `fullSizeBytes`). New modules: `diskmap/`, `selection/` and `rules/`. New detector modules for each added family. New Tauri commands and events from the spec's contract. `history` becomes `list_logbook`.
- **Storage**: a migration adds `suggestion_parts`, `item_decisions`, `pile_state`, `map_snapshots`, `blind_spot_decisions`, `tidy_rules` and `tidy_runs`.
- **Dependencies**: adds `blake3` for duplicate detection. Swaps the bundled fonts to Fredoka, Nunito and DM Mono.
- **Verification**: replaces `scripts/visual/matrix.mjs`, the baselines and `docs/visual-verification.md`. The site, screenshots and demo video will need regenerating after the UI lands.
- **Removed**: the `build-freeboard` change folder (kept in git history).
