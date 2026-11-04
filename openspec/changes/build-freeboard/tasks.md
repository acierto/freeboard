# Tasks

Implement one milestone at a time; each exit gate must pass on macOS, Linux, and Windows before proceeding. All work below is pending. Use temporary fixtures and fake tools for scanning/cleanup tests. Never run real cleanup against the developer machine.

Source: `docs/reference/FREEBOARD_SPEC.md`; appearance: `docs/reference/freeboard-prototype.html`; requirements: this change’s six capability specs; verification matrix: `design.md`.

## 1. Foundation

- [ ] 1.1 Create the Tauri 2, Rust, React, TypeScript, and Vite skeleton using the source module layout; verify development builds on macOS, Linux, and Windows and commit dependency lockfiles.
- [ ] 1.2 Implement restricted Tauri capabilities and CSP; verify contract tests deny frontend filesystem/shell access, arbitrary command/path input, and remote content.
- [ ] 1.3 Create SQLite migrations and source table retention rules; verify fixture database creation, upgrade, pruning, and settings round trips.
- [ ] 1.4 Implement the guarded filesystem boundary and executable/argument runner with a 20-second timeout; verify protected roots, canonicalization, symlink/junction traps, forbidden commands, and timeout handling using fixtures.
- [ ] 1.5 Implement settings defaults, validation, and persistence with typed get/update commands; verify default values against the source table and reject inconsistent thresholds.
- [ ] 1.6 Implement tray lifecycle, reopen, and Quit; verify state survives webview destruction after 60 seconds and the native tray menu works on all target platforms.
- [ ] 1.7 Create the prototype-derived navigation and Settings shell with local fonts and licenses, tokens, light/dark themes, and responsive rules; verify offline font loading and keyboard focus.
- [ ] 1.8 Build the visual verification harness and documented verify:visual entry point; verify it produces reference/actual/diff images and returns failure for a deliberately changed font, gauge shape, color, or spacing.
- [ ] 1.9 Capture prototype-derived baseline manifests for the design matrix and add reviewed supplemental states absent from the prototype; verify source hashes, fixture data, theme, viewport, engine, and crop metadata are recorded.
- [ ] 1.10 Create cross-platform CI build/test jobs and artifact retention for visual reports; verify unsigned installers build and failed/missing visual evidence blocks the applicable gate.
- [ ] 1.11 Document architecture, verification commands, fixture-only safety policy, and source conflicts in docs/decisions.md; verify a reader can reproduce the foundation checks.
- [ ] 1.12 Foundation exit gate: attach three-platform installer, permission-boundary, tray, and available-screen visual evidence; stop after the gate passes before starting milestone 2.

## 2. Monitoring

- [ ] 2.1 Implement home/additional fixed-volume sampling every five minutes with hourly persistence; verify deterministic clock fixtures and per-volume histories.
- [ ] 2.2 Implement Low/Critical transitions and 2 GB recovery hysteresis; verify exact boundary and oscillation tests with configurable thresholds.
- [ ] 2.3 Implement entry notifications, six-hour Critical reminders, pause/disable controls, and notification navigation; verify fake-clock notification counts and native click behavior.
- [ ] 2.4 Implement the 14-day linear forecast with three-day minimum and cleanup-jump exclusion; verify falling, flat, rising, sparse, and cleanup-containing series.
- [ ] 2.5 Build Overview using the original hull SVG, textual status, threshold markers, category/risk bars, conditional forecast, trend, and scan/Trash surfaces; verify fixture data binding and gauge boundary cases.
- [ ] 2.6 Keep monitor/scheduler alive without a webview and apply battery/priority policy; verify hidden-mode operation, reopen consistency, and battery transition behavior.
- [ ] 2.7 Run Overview visual comparisons for all required pressure states, themes, widths, and reduced motion; verify screenshots match the reference and retain reviewed diff reports.
- [ ] 2.8 Document and measure hidden memory and five-minute idle CPU on all platforms; verify reported actual values are distinguished from targets.
- [ ] 2.9 Monitoring exit gate: attach threshold/notification/forecast tests, native background evidence, memory measurements, and passing Overview visual reports for all platforms; stop before milestone 3.

## 3. Read-only detection

- [ ] 3.1 Implement quick/deep orchestration, progress/finish events, exclusions, low-priority walking, and detector isolation; verify fake timeouts, permissions, malformed output, missing tools, and battery pauses.
- [ ] 3.2 Implement stable suggestion IDs, fingerprints, full source contract, risk defaults, and category/size ordering; verify contract and ordering fixtures.
- [ ] 3.3 Implement allocated-size and identity adapters for Unix and Windows; verify sparse, compressed, hard-linked, and pnpm shared-data fixtures avoid inflated reclaim estimates.
- [ ] 3.4 Implement Docker version checks and all five Docker detectors after validating installed read-only interfaces; verify stopped-container references, 14-day age, volume risk, and unavailable daemon fixtures.
- [ ] 3.5 Implement Git discovery and linked-worktree classification; verify every source Git state, main-worktree exclusion, process detection, and conservative squash-merge reporting.
- [ ] 3.6 Implement project dependency/build detection; verify manifest/lockfile/ignore evidence, inactivity, running-process exclusion, and no nested generated-folder discovery.
- [ ] 3.7 Implement pnpm, pip, npm, and Homebrew cache detectors; verify source platform/action/risk mapping with fake executables and temporary cache fixtures.
- [ ] 3.8 Implement Cargo, Gradle, Xcode, and simulator detectors; verify Review/Safe classifications, daemon gating, and unsupported-platform behavior.
- [ ] 3.9 Implement installer and Windows TEMP discovery; verify all allowed extensions, 90-day Downloads criteria, document/media exclusion, and conservative unknown-age handling.
- [ ] 3.10 Build Suggestions filters, expandable detail rows, blocked checkboxes, category totals, and selection summary; verify only eligible Safe items start selected and every item exposes reason/action/recovery.
- [ ] 3.11 Run before/after fixture comparisons for every detector and tests for invariants 1, 6, 9, 10, and 11; verify scans do not mutate fixtures or follow links.
- [ ] 3.12 Run Suggestions visual matrix and measure quick/deep scan performance on documented fixtures; verify reference comparisons pass and measured timings are reported.
- [ ] 3.13 Document supported CLI formats and detector limitations; verify each source detector and failure state maps to tests.
- [ ] 3.14 Read-only exit gate: attach all detector fixture tests, unchanged-fixture proof, platform results, and passing Suggestions visuals; stop before milestone 4.

## 4. Cleanup

- [ ] 4.1 Resolve and record the source protected-root versus linked-worktree removal conflict with the user before enabling that destructive action; verify removal remains blocked until the clarified rule has tests.
- [ ] 4.2 Implement immutable saved plan snapshots, scan-generation invalidation, 30-minute expiry, three totals, and per-plan item acknowledgment; verify expired, modified, unknown, and unacknowledged plans cannot execute.
- [ ] 4.3 Implement per-item identity/fingerprint/path and resource-status revalidation; verify file edits, new commits, active projects, started containers, and symlink swaps are skipped with reasons.
- [ ] 4.4 Validate native cleanup interfaces and bound aggregate prune/cache commands to the confirmed resource set; verify newly eligible resources cannot be silently removed and unsupported scope guarantees block action.
- [ ] 4.5 Implement guarded Safe deletion and source-native actions with explicit recovery policy; verify banned commands and force flags remain rejected except the two confirmed Docker exceptions.
- [ ] 4.6 Implement individual Docker-volume acknowledgment and permitted worktree handling under the resolved policy; verify unsafe worktrees remain blocked and attached volumes cannot be removed.
- [ ] 4.7 Implement platform Trash movement, per-item identity tracking, separately confirmed scoped emptying, and unavailable-Trash fallback; verify unrelated Trash is untouched and permanent fallback needs a new plan.
- [ ] 4.8 Implement single-run locking, between-item cancellation, independent failures, and durable item transitions; verify duplicate runs, cancellation during action, interruption, and no automatic deletion replay.
- [ ] 4.9 Implement before/after measurements, expected-versus-measured reporting, retained skipped suggestions, and paginated history/run details; verify mixed-result runs and Docker Desktop discrepancies.
- [ ] 4.10 Build plan review, acknowledgment, running/canceled/finished states, History, and Trash confirmation; verify user flows exercise the real typed backend contract through fixtures.
- [ ] 4.11 Run plan/run/History/Trash visual matrix in both themes and required widths; verify reference parity and reviewed supplemental designs for source-missing states.
- [ ] 4.12 Create a safety traceability report for all 13 invariants with passing automated fixture tests, including local-only network behavior and ID-only cleanup; verify no invariant lacks coverage.
- [ ] 4.13 Document cleanup recovery and supported action limitations; verify all native manual Trash checks use dedicated disposable test resources.
- [ ] 4.14 Cleanup exit gate: attach three-platform safety, race, cancellation, history, native Trash, and visual evidence; keep unresolved destructive conflicts blocked and stop before milestone 5.

## 5. Release readiness

- [ ] 5.1 Implement first-run explanation, folder selection, and just-in-time OS permissions using reviewed prototype-derived components; verify onboarding flows and visual evidence on each platform.
- [ ] 5.2 Complete keyboard, accessible labels, focus, non-color risk/status, reduced-motion, long-path, and narrow-window checks; verify accessibility tests and manual screen-reader findings are resolved.
- [ ] 5.3 Run the full visual matrix including 761/760 and 481/480 breakpoint pairs and actual Tauri webviews on all platforms; verify linked reference/actual/diff evidence and explicit review disposition for every row.
- [ ] 5.4 Measure installer size, hidden memory, idle CPU, quick scans, and one-million-file SSD deep scans on each platform; verify actual results against all five targets and resolve misses before final acceptance.
- [ ] 5.5 Verify macOS APFS/snapshot reporting, Linux XDG/GNOME/KDE/WebKitGTK behavior, Windows long paths/junctions/Recycle Bin, Docker Desktop measured reclaim, and WSL exclusion; attach platform test evidence.
- [ ] 5.6 Resolve source release decisions on license, release order, and product name; verify recorded decisions before public distribution.
- [ ] 5.7 Configure macOS signing/notarization, Windows signing, and Linux AppImage/deb packaging; verify signed artifacts and installation smoke tests when credentials are available, otherwise mark the release gate incomplete.
- [ ] 5.8 Finish user/developer documentation, safety policy, recovery guidance, performance report, and verification instructions; verify documented steps against disposable test environments.
- [ ] 5.9 Final exit gate: map every source acceptance criterion to evidence, verify all milestone gates and visual comparisons pass with no unaccepted differences, and leave missing platform/signing evidence explicitly incomplete.
