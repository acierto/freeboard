# Tasks

Implement one milestone at a time. Each exit gate must pass on macOS, Linux and Windows before the next milestone starts. Run scanning and cleanup tests only against temporary fixtures and fake tool executables, never against this machine. Source: `docs/reference/FREEBOARD_SPEC.md`. Appearance: `docs/reference/freeboard-prototype.html`. Approach: `design.md`.

## 1. Foundation

- [x] 1.1 Record the rebuild decisions in `docs/decisions.md`: the superseded change, the macOS apps Trash exception, and the prototype-only parts not shipped. Verify each entry has a date and reason.
- [x] 1.2 Replace the bundled fonts with Fredoka (500/600/700), Nunito (500–800) and DM Mono (400/500), with their licenses. Verify the app and the verifier load all faces offline, with no remote request.
- [x] 1.3 Port the prototype's light and night color tokens and base styles into `src/styles`. Verify the token values match the prototype's `:root` and dark blocks exactly.
- [x] 1.4 Add the storage migration for `suggestion_parts`, `item_decisions`, `pile_state`, `map_snapshots`, `blind_spot_decisions`, `tidy_rules` and `tidy_runs`, with retention pruning. Verify upgrade from a previous-schema fixture database keeps settings, samples and history, and that pruning tests pass for each table.
- [x] 1.5 Extend settings with goal, Where from, inactive-project days, old-installer days, rules off and pause-on-battery. Verify the defaults test matches the spec table and that inconsistent thresholds are rejected.
- [x] 1.6 Update Tauri capabilities and CSP for the new command set. Verify the contract tests deny shell and filesystem access and reject path or command inputs on every new command.
- [x] 1.7 Rework the visual verifier for the new prototype: font adapter, hidden `.sim` and Start over, and new `REQUIRED_FONTS`. Delete the old matrix and baselines. Verify the self-test fails on a changed heading font, Dinghy shape, color and spacing.
- [x] 1.8 Update CI to build installers and run Rust, TypeScript, contract and visual jobs on all three platforms, keeping visual reports as artifacts. Verify a failing visual entry fails the job.
- [ ] 1.9 Foundation exit gate: attach installers from all three platforms, contract-test results and the verifier self-test report. Stop before milestone 2.

## 2. Monitoring and home view

- [ ] 2.1 Re-verify existing sampling, hysteresis, notification and forecast code against the disk-monitoring spec, and fix any gaps. Verify the boundary, oscillation, 6-hour reminder, paused, sparse-history and cleanup-jump tests pass.
- [ ] 2.2 Rewrite `src/lib/fixtureBackend.ts` to serve the prototype's example data (items, parts, tiles, blind spots, growth, places, Logbook) through the real command contract. Verify the frontend type check passes with no fixture code in `dist/`.
- [ ] 2.3 Build the Dinghy component from the prototype SVG (sky, sun or moon, stars, clouds, waves, hull, cabin, face states, up to 9 crates), with deck height linear from 0–60 GB and reduced-motion handling. Verify unit tests for height and mood at 0, 4, 12.4, 40, 60 and 120 GB.
- [ ] 2.4 Build the home layout (top bar with brand, tray button and gear; stage with bubble; helm with free space, status pill, forecast, goal and Where from chips, Toss button, note, See what is inside, Trash note) and the tabs shell. Verify keyboard focus order and visible focus.
- [ ] 2.5 Build the tray view (status, free space, pile total, Review the pile) and wire native tray rings, the menu, hide-to-tray and webview destruction after 60 seconds. Verify the tray view closes by button, Esc and outside click, and that state matches after reopening.
- [ ] 2.6 Add home visual matrix entries (Healthy, Low, Critical; light and dark; 1040, 760 and 390 widths; tray view open). Verify the verifier reports PASS for each, with reference, actual and diff images.
- [ ] 2.7 Measure hidden-mode memory and 5-minute idle CPU on each platform and record them in `docs/performance.md`, separating measured values from targets.
- [ ] 2.8 Monitoring exit gate: attach threshold, notification and forecast test results, native tray and notification evidence, memory measurements and passing home visuals for all platforms. Stop before milestone 3.

## 3. Disk understanding

- [ ] 3.1 Change the model: `Group`, `map_area`, `full_size_bytes`, `parts`, and plans with part IDs. Remap existing detectors (`cache.simulators` becomes `mobile.simulators`). Verify every existing Rust test passes after the change.
- [ ] 3.2 Split `project.dependencies` into per-project parts, and add `cache.npm` (folder removal), `cache.jetbrains`, `cache.electron_apps` and `cache.playwright`. Verify fixture tests for evidence rules, inactivity and running-process skips.
- [ ] 3.3 Add `docker.disk_slack` as information-only, and check that the existing Docker detectors meet the used-image and attached-volume rules. Verify fixtures for stopped-container references, the 14-day age and the unavailable daemon.
- [ ] 3.4 Add `ai.ollama` (per-model parts), `ai.huggingface` and `ai.lm_studio` (per-model parts), after checking each tool's real output. Verify with fake-executable and folder fixtures, including malformed output marking the detector unavailable.
- [ ] 3.5 Add `mobile.ios_backups`, `mobile.android_avd` and `mobile.simulators` with parts. Verify fixtures for age thresholds and the unavailable-runtime rule.
- [ ] 3.6 Add `tools.node_versions` and `tools.rust_toolchains`, with project-requested versions kept out. Verify fixtures where `.nvmrc`, `engines`, `rust-toolchain.toml` and `package.json` pin versions.
- [ ] 3.7 Add `apps.unused` (macOS Trash per app; Windows and Linux link to the uninstaller) that suggests apps only when last-opened data exists. Add the narrow guard exception with positive and negative tests.
- [ ] 3.8 Add `files.duplicates` (size, head hash, then `blake3`; exclusions), `files.large_media`, and the installers rule for 90 days and the listed extensions. Verify near-duplicate, hidden-folder, bundle and repository exclusion fixtures.
- [ ] 3.9 Add `system.snapshots`, gated on confirming the latest backup, and `system.stale_temp`. Verify fixtures for the confirmed, unconfirmed and running-app cases.
- [ ] 3.10 Implement `diskmap/` accounting (claims, most-specific priority, inode deduplication, Not explained, coverage, the 1% incomplete flag, macOS purgeable and snapshot reporting) and `get_disk_map`. Verify accounting fixtures for hard links, nesting, unreadable folders and totals within 1%.
- [ ] 3.11 Implement blind spots, `resolve_blind_spot`, places checked, `map_snapshots` growth and `create_disk_report`. Verify resolving each kind updates coverage, that unavailable is never shown as clean, and that the report contains no file contents.
- [ ] 3.12 Prove scans are read-only across all detectors. Verify the before-and-after fixture tree hash is identical for quick and deep scans, and that symlink traps are not followed.
- [ ] 3.13 Build the Disk map tab (coverage ring, squarified treemap of buttons, area details with leave-alone rows, blind spots, growth, places checked, disk report copy with textarea fallback). Add visual entries for each tile and blind-spot state, light and dark, at three widths. Verify PASS.
- [ ] 3.14 Measure quick-scan time and the 1-million-file deep scan on each platform and record them in `docs/performance.md`.
- [ ] 3.15 Disk understanding exit gate: attach detector fixture results, unchanged-fixture proof, accounting results per platform, and passing Disk map visuals. Stop before milestone 4.

## 4. Selection

- [ ] 4.1 Implement `selection/fill` as a pure function, and implement the pile commands (`get_pile`, `set_pile_goal`, `set_pile_areas`, `add_to_pile`, `take_out_of_pile`) with `pile_changed`. Verify one unit test per row of the Building the pile table, plus the fill-to-goal and never-auto-add-Review tests.
- [ ] 4.2 Implement `set_item_decision` (Keep, Stop keeping, Ask me later) and `set_item_parts` with persistence by stable resource ID. Verify sizes follow parts and that decisions survive a rescan.
- [ ] 4.3 Build See what is inside (pile rows with take-out), the Toss button and the pile notes (manual, empty, short of goal). Verify UI tests for each message.
- [ ] 4.4 Build Quick questions (one card, facts, acknowledgment, Choose which, Add to pile, Keep, don't ask again, Ask me later, finished state with bring-back). Verify ordering and exclusion rules, and that the Risky tick gates Add to pile.
- [ ] 4.5 Build Everything I found (groups with totals, Remove all here, Add all to pile, item rows with checkbox, Remove now, Keep or Stop keeping, Choose which), the protected list and the Keep me tidy card. Verify that group actions exclude blocked, kept, changed and confirmation-needing items, and that open groups and focus survive re-render.
- [ ] 4.6 Add selection visual entries (goals, Where from, manual mode, empty pile, pile open, question cards, finished state, groups and parts open, protected list) in light and dark at three widths. Verify PASS.
- [ ] 4.7 Selection exit gate: attach the pile-rule test results and passing selection visuals for all platforms. Stop before milestone 5.

## 5. Cleanup

- [ ] 5.1 Adapt plans to `{ suggestion_id, part_ids }`, `create_plan` from the pile or IDs, and per-part revalidation. Verify the expiry, new-scan invalidation and missing-acknowledgment rejection tests.
- [ ] 5.2 Add part-level execution and keep unselected parts listed afterward. Verify partial-selection tests that update sizes and the map.
- [ ] 5.3 Re-run and extend the change-during-cleanup, banned-command, protected-path, symlink-swap, cancellation and failure tests for all new detectors' actions. Verify every safety invariant has a named passing test, listed in `docs/safety-traceability.md`.
- [ ] 5.4 Build the confirmation sheet (three totals, item list with exact actions, per-item acknowledgments, confirm button always visible), the progress sheet (current item, bar, Cancel, crates leaving and the boat rising) and the result sheet (measured change, Trash note, skipped reasons, Docker Desktop note). Verify focus trap, Esc rules and focus return.
- [ ] 5.5 Build Empty it with its own confirmation, limited to Freeboard's Trash items, and Windows Recycle Bin labels. Verify that the scoped-emptying test and the no-usable-Trash path offer a separate permanent plan.
- [ ] 5.6 Build the Logbook list, run details and empty state on `list_logbook`. Verify that runs show measured freed space, Trash amount and skipped reasons.
- [ ] 5.7 Add cleanup visual entries (confirmation with acknowledgment, long-list scroll, progress, result, Trash confirmation, Logbook, run details) in light and dark at two widths. Verify PASS.
- [ ] 5.8 Run manual Trash checks on each platform with disposable test resources only, and record the evidence in `docs/manual-checks.md`.
- [ ] 5.9 Cleanup exit gate: attach safety-invariant test results, race and cancellation tests, native Trash evidence and passing cleanup visuals for all platforms. Stop before milestone 6.

## 6. Keep me tidy and release

- [ ] 6.1 Implement `rules/`: both rules, off by default, rule-sourced plans through `cleanup::run`, low priority, `tidy_rule_ran`, and Logbook records. Verify the off-by-default, kept-item, Review-in-scope and weekly-run tests.
- [ ] 6.2 Wire the Keep me tidy toggles with the "what it will remove" statement. Verify the UI test and visual entry.
- [ ] 6.3 Build first-run setup (what it reads, what it never does, folder offers, choose folder, skip) and the just-in-time permission flow. Verify the no-early-prompt test and visual entries.
- [ ] 6.4 Build the Settings sheet (warning levels, scan folders with add and remove, excluded folders, scan schedule, deep scan now, startup, notifications, battery). Verify validation and visual entries.
- [ ] 6.5 Build scan progress, scan failure, detector-unavailable, new-install, no-suggestions and empty-Logbook states, plus Windows and Linux labels. Verify supplemental visual entries, flagged for product-owner review.
- [ ] 6.6 Run axe and keyboard checks on every view and sheet (focus visible, focus trap, Esc, focus return, reduced motion, no color-only meaning). Verify zero violations and record screen-reader findings.
- [ ] 6.7 Run the full visual matrix in light and dark at every width, and capture native webview and tray evidence on all three platforms. Verify every row has reference, actual, diff and a review disposition, with none failing or incomplete.
- [ ] 6.8 Measure installer size, hidden memory, idle CPU, quick scan and deep scan on each platform against the five targets. Fix any misses or record them as decisions.
- [ ] 6.9 Regenerate `site/` screenshots and the demo video from the new UI, and update `README`, `docs/architecture.md`, `docs/detectors.md` and `docs/cleanup.md`. Verify that the documented commands run as written.
- [ ] 6.10 Release exit gate: verify every acceptance criterion in the spec with linked evidence, then archive this change.
