# Tasks

Implement one group at a time. Each exit gate must pass on macOS, Linux and Windows before the next group starts.

**Vertical slices (re-planned 2026-11-06 with the user).** Groups 3–5 used to build the interface against sample data and the Rust core in later milestones; that left the installed app as an empty shell. Every group from 3 on ships a feature end to end: Rust commands, interface, tests, and a click-through of the real installed app. No group may leave a control on `main` that calls a command the Rust core does not have; the contract test in 3.1 enforces it. Run scanning and cleanup tests only against temporary fixtures and fake tool executables, never against this machine. Source: `docs/reference/FREEBOARD_SPEC.md`. Appearance: `docs/reference/freeboard-prototype.html`. Approach: `design.md`.

## 1. Foundation

- [x] 1.1 Record the rebuild decisions in `docs/decisions.md`: the superseded change, the macOS apps Trash exception, and the prototype-only parts not shipped. Verify each entry has a date and reason.
- [x] 1.2 Replace the bundled fonts with Fredoka (500/600/700), Nunito (500–800) and DM Mono (400/500), with their licenses. Verify the app and the verifier load all faces offline, with no remote request.
- [x] 1.3 Port the prototype's light and night color tokens and base styles into `src/styles`. Verify the token values match the prototype's `:root` and dark blocks exactly.
- [x] 1.4 Add the storage migration for `suggestion_parts`, `item_decisions`, `pile_state`, `map_snapshots`, `blind_spot_decisions`, `tidy_rules` and `tidy_runs`, with retention pruning. Verify upgrade from a previous-schema fixture database keeps settings, samples and history, and that pruning tests pass for each table.
- [x] 1.5 Extend settings with goal, Where from, inactive-project days, old-installer days, rules off and pause-on-battery. Verify the defaults test matches the spec table and that inconsistent thresholds are rejected.
- [x] 1.6 Update Tauri capabilities and CSP for the new command set. Verify the contract tests deny shell and filesystem access and reject path or command inputs on every new command.
- [x] 1.7 Rework the visual verifier for the new prototype: font adapter, hidden `.sim` and Start over, and new `REQUIRED_FONTS`. Delete the old matrix and baselines. Verify the self-test fails on a changed heading font, Dinghy shape, color and spacing.
- [x] 1.8 Update CI to build installers and run Rust, TypeScript, contract and visual jobs on all three platforms, keeping visual reports as artifacts. Verify a failing visual entry fails the job.
- [x] 1.9 Foundation exit gate: attach installers from all three platforms, contract-test results and the verifier self-test report. Stop before milestone 2.
  - Evidence (2026-11-06): CI run https://github.com/acierto/freeboard/actions/runs/42222070943 on commit `main`. Core and contract tests passed on macOS, Ubuntu 22.04 and Windows. Unsigned installers were built and uploaded for all three (`installers-macos-latest`, `installers-ubuntu-22.04`, `installers-windows-latest`). The verifier self-test passed on the CI runner. The visual comparison and the overall CI gate fail by design until milestone 2 adds matrix entries and Dinghy.

## 2. Monitoring and home view

- [x] 2.1 Re-verify existing sampling, hysteresis, notification and forecast code against the disk-monitoring spec, and fix any gaps. Verify the boundary, oscillation, 6-hour reminder, paused, sparse-history and cleanup-jump tests pass.
- [x] 2.2 Rewrite `src/lib/fixtureBackend.ts` to serve the prototype's example data (items, parts, tiles, blind spots, growth, places, Logbook) through the real command contract. Verify the frontend type check passes with no fixture code in `dist/`.
- [x] 2.3 Build the Dinghy component from the prototype SVG (sky, sun or moon, stars, clouds, waves, hull, cabin, face states, up to 9 crates), with deck height linear from 0–60 GB and reduced-motion handling. Verify unit tests for height and mood at 0, 4, 12.4, 40, 60 and 120 GB.
- [x] 2.4 Build the home layout (top bar with brand, tray button and gear; stage with bubble; helm with free space, status pill, forecast, goal and Where from chips, Toss button, note, See what is inside, Trash note) and the tabs shell. Verify keyboard focus order and visible focus.
- [ ] 2.5 Build the tray view (status, free space, pile total, Review the pile) and wire native tray rings, the menu, hide-to-tray and webview destruction after 60 seconds. Verify the tray view closes by button, Esc and outside click, and that state matches after reopening.
  - macOS and Linux GNOME verified 2026-11-06 (`visual/native/macos`, `visual/native/linux-gnome`); Windows pending.
- [x] 2.6 Add home visual matrix entries (Healthy, Low, Critical; light and dark; 1040, 760 and 390 widths; tray view open). Verify the verifier reports PASS for each, with reference, actual and diff images.
- [ ] 2.7 Measure hidden-mode memory and 5-minute idle CPU on each platform and record them in `docs/performance.md`, separating measured values from targets.
  - macOS 21 MB / 0.053% and Linux GNOME 22.9 MB / 0.328% measured 2026-11-06; Windows pending.
- [ ] 2.8 Monitoring exit gate: attach threshold, notification and forecast test results, native tray and notification evidence, memory measurements and passing home visuals for all platforms. Windows is deferred until after group 3 (user, 2026-11-06); this gate closes when the Windows checks run.

## 3. Find and choose, working in the real app (slice 1)

- [x] 3.1 Add a contract test that every command the interface calls (`src/lib/api.ts`) is registered in the Rust shell and on the permission allowlist, and that every registered command is called or documented. Verify it fails on today's `main` (pile, disk map and Logbook commands are missing) and passes once this group lands.
- [x] 3.2 Change the Rust model to the spec's suggestion shape: `group`, `mapArea`, `fullSizeBytes`, `parts`, `decision`, and plans with part IDs. Map existing detectors to groups (`cache.simulators` becomes `mobile.simulators`) and split `project.dependencies` into one part per project. Verify every existing Rust test passes and a serde test matches `src/lib/types.ts`.
- [x] 3.3 Bring back scan-folder control: Settings lists scan and excluded folders with Remove, Add folder (backend picker), the offered folders that hold Git repositories, Scan now and Run deep scan now; first-run setup asks for folders before the home view. Verify interface checks, and in the real app that adding a folder and scanning shows its findings.
- [x] 3.4 Implement `selection/fill` as a pure function and the pile commands (`get_pile`, `set_pile_goal`, `set_pile_areas`, `add_to_pile`, `take_out_of_pile`) with `pile_changed`, persisted in `pile_state`. Verify one unit test per row of the Building the pile table, plus fill-to-goal and never-auto-add-Review.
- [x] 3.5 Implement `set_item_decision` (Keep, Stop keeping, Ask me later) and `set_item_parts`, persisted by stable resource ID. Verify sizes follow selected parts and decisions survive a rescan.
- [x] 3.6 Build Quick questions (one card, facts, acknowledgment, Choose which, Add to pile, Keep, don't ask again, Ask me later, finished state with bring-back). Verify ordering and exclusions, and that the Risky tick gates Add to pile.
- [x] 3.7 Build Everything I found (groups with totals, Remove all here, Add all to pile, item rows with checkbox, Keep or Stop keeping, Choose which) and the protected list. Remove now and Remove all here open the confirmation from group 4; until then they are not shown. Verify group actions exclude blocked, kept, changed and confirmation-needing items, and open groups and focus survive re-render.
- [x] 3.8 Add selection visual entries (goals, Where from, manual mode, empty pile, pile open, question cards, finished state, groups and parts open, protected list) in light and dark at three widths. Verify PASS.
- [ ] 3.9 Add a scripted click-through of the real installed app (`scripts/native/`): launch, open each control, screenshot, and compare what it shows with the backend's data. Verify it runs on macOS and the Linux VM.
  - Linux (2026-11-06): `scripts/native/e2e.mjs` drives the installed `.deb` through WebKitWebDriver with fixtures from `scripts/native/fixtures.sh`; 20 of 20 checks pass (`visual/native/linux-gnome/e2e-slice1/`). macOS has no WebDriver for WKWebView, so the macOS click-through is done by hand with real mouse events and screenshots; scripting it (accessibility API) is still open.
- [ ] 3.10 Slice 1 exit gate: the click-through passes on macOS and Linux (Windows with 2.8): scan finds items, Where from lists their groups, goals fill the pile with Safe items, take-out and Keep work, Quick questions and Everything I found show real items. Attach screenshots and test results.

## 4. Clean up and Logbook, working in the real app (slice 2)

- [x] 4.1 Adapt plans to `{ suggestion_id, part_ids }`, `create_plan` from the pile or IDs, and per-part revalidation. Verify the expiry, new-scan invalidation and missing-acknowledgment rejection tests.
- [x] 4.2 Add part-level execution and keep unselected parts listed afterward. Verify partial-selection tests that update sizes.
- [x] 4.3 Build the confirmation sheet (three totals, item list with exact actions, per-item acknowledgments, confirm button always visible), the progress sheet (current item, bar, Cancel, crates leaving and the boat rising) and the result sheet (measured change, Trash note, skipped reasons, Docker Desktop note), from Toss, Remove now and Remove all here. Verify focus trap, Esc rules and focus return.
- [x] 4.4 Build Empty it with its own confirmation, limited to Freeboard's Trash items, with Windows Recycle Bin labels. Verify the scoped-emptying test and that the no-usable-Trash path offers a separate permanent plan.
- [x] 4.5 Rename the history command to `list_logbook` and build the Logbook list, run details and empty state. Verify runs show measured freed space, Trash amount and skipped reasons.
- [x] 4.6 Re-run and extend the change-during-cleanup, banned-command, protected-path, symlink-swap, cancellation and failure tests. Verify every safety invariant has a named passing test, listed in `docs/safety-traceability.md`.
- [x] 4.7 Add cleanup visual entries (confirmation with acknowledgment, long-list scroll, progress, result, Trash confirmation, Logbook, run details) in light and dark at two widths. Verify PASS.
- [ ] 4.8 Slice 2 exit gate: the click-through cleans disposable test resources only (a fixture folder and a test Docker image), shows the result and the Logbook entry, and empties Freeboard's Trash items, on macOS and Linux (Windows with 2.8). Record the evidence in `docs/manual-checks.md`.
  - Linux (2026-11-06): real cleanup of fixture projects through the installed app, 31/31 checks (`visual/native/linux-gnome/e2e-slice2/`). Still open: Trash emptying and a test Docker image on Linux; macOS.

## 5. Disk map and the new detectors, working in the real app (slice 3)

- [x] 5.1 Add `cache.npm` (folder removal), `cache.jetbrains`, `cache.electron_apps` and `cache.playwright`. Verify fixture tests for evidence rules.
  - `cache.npm` landed earlier; the other three on 2026-11-08 (`tests::app_caches`).
- [x] 5.2 Add `docker.disk_slack` as information-only, and check that the existing Docker detectors meet the used-image and attached-volume rules. Verify fixtures for stopped-container references, the 14-day age and the unavailable daemon.
- [x] 5.3 Add `ai.ollama` (per-model parts), `ai.huggingface` and `ai.lm_studio` (per-model parts), after checking each tool's real output. Verify with fake-executable and folder fixtures, including malformed output marking the detector unavailable.
  - Hugging Face's layout checked on a real cache (2026-11-08). Ollama and LM Studio are not installed on the development Mac; their formats come from the documented on-disk layouts (`docs/decisions.md`, 2026-11-08). Validate them on a machine with each tool before release.
- [x] 5.4 Add `mobile.ios_backups`, `mobile.android_avd` and `mobile.simulators` with parts. Verify fixtures for age thresholds and the unavailable-runtime rule.
- [x] 5.5 Add `tools.node_versions` and `tools.rust_toolchains`, with project-requested versions kept out. Verify fixtures where `.nvmrc`, `engines`, `rust-toolchain.toml` and `package.json` pin versions.
- [x] 5.6 Add `apps.unused` (macOS Trash per app; Windows and Linux link to the uninstaller), only when last-opened data exists, with the narrow guard exception and its positive and negative tests.
- [x] 5.7 Add `files.duplicates` (size, head hash, then `blake3`; exclusions), `files.large_media`, and the installers rule for 90 days and the listed extensions. Verify near-duplicate, hidden-folder, bundle and repository exclusion fixtures.
- [x] 5.8 Add `system.snapshots`, gated on confirming the latest backup, and `system.stale_temp`. Verify fixtures for the confirmed, unconfirmed and running-app cases.
- [x] 5.9 Implement `diskmap/` accounting (claims, most-specific priority, inode deduplication, Not explained, coverage, the 1% incomplete flag, macOS purgeable and snapshot reporting) and `get_disk_map`. Verify accounting fixtures for hard links, nesting, unreadable folders and totals within 1%.
- [x] 5.10 Implement blind spots, `resolve_blind_spot`, places checked, `map_snapshots` growth and `create_disk_report`. Verify resolving each kind updates coverage, that unavailable is never shown as clean, and that the report contains no file contents.
- [x] 5.11 Prove scans are read-only across all detectors. Verify the before-and-after fixture tree hash is identical for quick and deep scans, and that symlink traps are not followed.
- [x] 5.12 Build the Disk map tab (coverage ring, squarified treemap of buttons, area details with leave-alone rows and item controls, blind spots, growth, places checked, disk report copy with textarea fallback). Add visual entries for each tile and blind-spot state, light and dark, at three widths. Verify PASS.
- [ ] 5.13 Measure quick-scan time and the 1-million-file deep scan on each platform and record them in `docs/performance.md`.
- [ ] 5.14 Slice 3 exit gate: the click-through shows a real disk map whose totals match used space within 1% (or says it is incomplete), resolves a blind spot, and copies the report, on macOS and Linux (Windows with 2.8).

## 6. Keep me tidy and release

- [x] 6.1 Implement `rules/`: both rules, off by default, rule-sourced plans through `cleanup::run`, low priority, `tidy_rule_ran`, and Logbook records. Verify the off-by-default, kept-item, Review-in-scope and weekly-run tests.
- [ ] 6.2 Wire the Keep me tidy toggles with the "what it will remove" statement. Verify the UI test, the visual entry and the real app.
  - UI checks (`scripts/ui/check.mjs`, tidy) and visual entries `tidy-card`, `tidy-ask`, `tidy-on` pass (2026-11-08). The real installed app is still to be checked.
- [x] 6.3 Add the just-in-time permission flow (macOS Full Disk Access from its blind spot). Verify the no-early-prompt test.
- [x] 6.4 Complete the Settings sheet (scan schedule, startup, notifications, battery, extra volumes). Verify validation and visual entries.
- [x] 6.5 Build scan progress, scan failure, detector-unavailable, new-install, no-suggestions and empty-Logbook states, plus Windows and Linux labels. Verify supplemental visual entries, flagged for product-owner review.
- [ ] 6.6 Run axe and keyboard checks on every view and sheet (focus visible, focus trap, Esc, focus return, reduced motion, no color-only meaning). Verify zero violations and record screen-reader findings.
- [ ] 6.7 Run the full visual matrix in light and dark at every width, and capture native webview and tray evidence on all three platforms. Verify every row has reference, actual, diff and a review disposition, with none failing or incomplete.
- [ ] 6.8 Measure installer size, hidden memory, idle CPU, quick scan and deep scan on each platform against the five targets. Fix any misses or record them as decisions.
- [ ] 6.9 Regenerate `site/` screenshots and the demo video from the new UI, and update `README`, `docs/architecture.md`, `docs/detectors.md` and `docs/cleanup.md`. Verify that the documented commands run as written.
- [ ] 6.10 Release exit gate: verify every acceptance criterion in the spec with linked evidence, then archive this change.
