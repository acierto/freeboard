# Design

## Context

See proposal.md for why. The codebase was built for the previous spec, and about 12,000 lines of Rust exist:

- `guard.rs`: canonical protected-path checks and link handling.
- `runner.rs`: executable plus argument array, 20-second timeout, banned-command list.
- `store.rs`: SQLite with migrations.
- `monitor/`: sampling, status with hysteresis, notifications, forecast.
- `scan/`: orchestration, walking, on-disk size with hard-link deduplication, folders.
- `detectors/`: Docker (5), git worktrees, projects, caches (pnpm, pip, npm, homebrew, cargo, gradle, xcode, simulators), files (installers, windows_temp).
- `cleanup/`: plans, acknowledgments, revalidation, execution, cancellation, Trash ledger, persistence.
- `tray.rs`, `platform/` (macOS, Unix, Windows) and `settings.rs`.

The safety tests in `cleanup/tests.rs` and `detectors/tests/` cover most invariants. The frontend (`src/screens`, `src/components`) and the visual verifier matrix target the old prototype. The spec says to keep Rust code that meets it and rebuild the interface.

## Goals / Non-Goals

**Goals:**
- Reuse and extend the existing safety core instead of rewriting it, so the invariant tests keep guarding cleanup throughout the rebuild.
- Make the backend the single owner of selection state (pile, decisions, parts) so the tray view, background rules and the window always agree.
- Keep one fixture data set that drives both the visual verifier and frontend development, mirroring the prototype's example data.

**Non-Goals:**
- Preserving any old screen, component or flow.
- Manual folder browsing, WSL filesystem scanning, Mac App Store distribution (product non-goals).
- Changing the Tauri, React or Vite stack.

## Decisions

### D1. Keep the Rust safety core, change the model in place
`model.rs` changes `Category` to `Group`, adds `map_area`, `full_size_bytes` and `parts: Vec<SuggestionPart>`, and keeps `fingerprint`, `risk`, `action`, `blocked_reason` and `needs_confirmation`. Existing detectors map their categories to groups: worktrees and builds map to Projects; `cache.simulators` becomes `mobile.simulators`. Plans change from `item_ids` to `{ suggestion_id, part_ids }`, and revalidation checks each selected part's fingerprint.
*Alternative:* a clean rewrite. Rejected, because it would discard tested guard, runner and cleanup code and reopen safety risk for no product gain.

### D2. Parts are first-class, size follows selection
A suggestion's `size_bytes` is always the sum of its selected parts, computed in the backend from `item_decisions`. The full size is kept separately for the map. After a partial cleanup, the next scan re-emits the remaining parts under the same stable resource IDs, so earlier selections still apply.
*Alternative:* model parts as separate suggestions. Rejected, because the prototype shows one row with Choose which, and group totals would double count.

### D3. Pile logic lives in `selection/` as a pure function
`fill(pile_state, suggestions, decisions) -> item_ids` is a pure, deterministic function over plain data. The rules table in cleanup-selection maps one-to-one to unit tests. Commands mutate the stored `PileState` and call `fill`, and every mutation emits `pile_changed`. The frontend never computes the pile.
*Alternative:* compute the pile in React like the prototype does. Rejected, because tray, rules and notifications need it without a webview, and invariant 3 keeps decisions in the backend.

### D4. Disk accounting by claimed ranges
`diskmap/` builds the map from three sources. First, the volume's used bytes (`sysinfo`). Second, detector claims: each suggestion's paths plus each detector's leave-alone paths, such as active projects, images in use and recently used models. Third, area roots for the scanned folders and well-known locations (Applications, Library, Documents, Pictures, Movies, Music, Desktop, and the OS area). Claims are resolved most specific first, with paths deduplicated by device and inode through the existing `scan/size.rs`, so each byte lands once. Not explained is used bytes minus all claims. If claims exceed used bytes by more than 1% (APFS clones, purgeable space, snapshots), the map is flagged incomplete with the difference, and APFS purgeable space and local snapshots are reported separately on macOS. Docker Desktop's VM disk is claimed as Docker, with in-VM usage from `docker system df`, so the two are not added together.
*Alternative:* a full disk walk for every scan. Rejected for quick scans because of the 10-second target. Deep scans walk the configured folders. Everything outside them stays in Not explained or becomes a blind spot, which is the honest result.

### D5. Blind spots are derived, decisions persisted
Blind spots are computed after each scan:
- **Unscanned folder:** a large top-level home folder not in the scan list or exclusions.
- **Needs permission:** a read failure with EPERM or EACCES, or a macOS TCC-protected location.
- **Unknown files:** a large folder inside a scanned tree that no detector claims.
- **Tool unavailable:** a detector reported unavailable.

`resolve_blind_spot` performs one action per kind:
- **Scan this folder:** add the folder to the scan list and run a deep scan of it.
- **Open permission flow:** open the System Settings pane through the platform module, then re-check on focus.
- **Mark as mine:** write a `blind_spot_decisions` row so the bytes count under Your files.
- **Tool unavailable:** show how to start the tool, then scan again.

The prototype's instant "Allow access" result is not reproduced.

### D6. New detector families follow the existing pattern
Each family gets a module under `detectors/` (`ai.rs`, `mobile.rs`, `tools.rs`, `apps.rs`, `system.rs`, and duplicate and media detection in `files.rs`). All share `common.rs` helpers and the runner. Each tool's real interface is checked at implementation time (spec instruction 2). Examples: `ollama list`, `nvm ls` (a shell function, so read `$NVM_DIR/versions` instead), `rustup toolchain list`, `xcrun simctl list -j`, `tmutil listlocalsnapshots` and `tmutil latestbackup`. Duplicates use size buckets, then a 64 KiB head hash, then full `blake3`, inside scanned folders only. `docker.disk_slack` is information-only until a supported compaction method is verified per platform.

### D7. Tidy rules reuse the plan path
A rule run builds a normal plan from the named detectors' Safe, unkept items, marks it as rule-sourced, and executes it through the same `cleanup::run` with the same revalidation. Invariant 2's exception is enforced in one place: `rules/` can only create rule-sourced plans, and `cleanup::run` refuses a rule-sourced plan that contains a non-Safe item or a detector outside the rule's list.

### D8. Frontend rebuilt by feature, with one store fed by events
`src/features/{home,questions,disk-map,cleanup,logbook}` and `src/components` (Dinghy, Treemap, Sheet, Chip, ItemRow, Parts) implement the UI. A small React store holds the latest `Status`, `PileState`, suggestions, map, blind spots and runs, refreshed by commands and backend events. The DOM structure and class names follow the prototype (`.hero`, `.stage`, `.helm`, `.tabs`, `.grp`, `.row-item`, `.tile`, `.sheet`, and so on), so the visual verifier can drive both sides with shared steps. Dinghy, the crates and the waves are the prototype's SVG geometry, ported into a component. The treemap uses the prototype's squarify algorithm.

### D9. Visual verifier kept, matrix rewritten
`scripts/visual/lib.mjs` stays: adapter, pinned Chromium, zero tolerance, report. Changes:
- The adapter now loads Fredoka, Nunito and DM Mono locally and hides `.sim` and `[data-act="reset"]`.
- `matrix.mjs` is rewritten for the visual-verification capability's matrix.
- The old baselines are deleted.
- `src/lib/fixtureBackend.ts` is rewritten to serve the prototype's example data through the real command contract, including part and pile behavior, so actual captures exercise production UI code.
- The prototype's internal state can be driven only through its own controls. Supplemental states use documented DOM edits as before.

### Prototype simulation versus production
| Prototype | Production |
| --- | --- |
| Fixed example items and map | Real detectors and accounting, which can be empty, partial or unavailable |
| Pretend free space slider | Removed. Free space comes from the monitor |
| Start over | Removed |
| `old-experiment` always skipped | Items are skipped only when revalidation finds a change |
| Allow access resolves instantly | Opens the OS permission flow, and stays open until access is really granted |
| Run takes fixed time, frees expected size | Real execution, then free space is measured. Docker Desktop may return less |
| Empty it frees immediately, no confirmation | Separate confirmation, and empties only Freeboard's own Trash items |
| Rules toggle with no effect | Real background rules with Logbook records |
| No first run, scan progress, failure, empty or cancel states | Built in the prototype's style and verified as supplemental entries |

## Risks / Trade-offs

- [Accounting cannot reach 1% on APFS: clones, purgeable space and snapshots] → Report purgeable space and snapshots separately, flag the map as incomplete with the exact difference, and test the rule on fixtures. Measure real coverage on macOS, Linux and Windows during milestone 3, and record the results.
- [Duplicate hashing could blow the deep-scan budget] → Hash only size-colliding candidates, use head hashes first, and skip files under 1 MiB by default. Measure against the 1-million-file target.
- [New tool CLIs differ by version (ollama, xcrun, tmutil)] → Detect version, mark the detector unavailable on unknown output (invariant 11), and use fake-executable fixtures for each known format.
- [The apps exception widens the guard] → Allow-list exactly one shape (a top-level non-system `.app` in two directories), Trash only, with acknowledgment. Cover it with positive and negative guard tests.
- [Zero-tolerance pixel matching against a new prototype with animations] → Captures use reduced motion and a paused clock as before. Any per-entry tolerance needs a recorded experiment.
- [Breaking model change ripples through cleanup tests] → Land D1 first with all existing tests green before any UI work starts.

## Migration Plan

1. Migrate the database. A new migration adds the new tables and maps old `category` rows. History rows become Logbook rows unchanged.
2. Change the model and adapt detectors, keeping all existing Rust tests passing.
3. Add the new backend modules and commands behind the existing contract tests.
4. Replace the frontend and the fixture backend, then the visual matrix and baselines.
5. Delete the old screens, components, matrix entries and baselines in the same milestone that replaces them, so `main` always builds.

Rollback: the previous product is the commit before this change. The migration is additive, so the old binary can still read a migrated database, apart from the new tables.

## Open Questions

- The product open questions in the spec: license, which OS gets the first signed release, final names, and manual folder browsing later. None of them changes this plan.
- Which Docker Desktop compaction method is supported on each platform. Until one is verified, `docker.disk_slack` stays information-only, which the specs already allow.
