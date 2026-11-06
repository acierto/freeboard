# Cleanup

Source: `docs/reference/FREEBOARD_SPEC.md`, "Safety invariants", "Delete and
restore policy", "Cleanup rules", "Pre-cleanup checks", "Trash". Code:
`src-tauri/src/cleanup/`.

## Flow

1. `create_plan(source, itemIds)` takes the pile (`source: "pile"`, no IDs)
   or item IDs (`source: "items"`), resolves them to the latest scan's
   findings and stores an immutable snapshot (findings plus backend-owned
   targets) with a SHA-256 digest, the scan generation and a 30-minute
   expiry. Unknown, blocked or action-less IDs are rejected. Each plan item
   is `{ suggestionId, partIds }`: an item split into parts covers the parts
   selected with Choose which (read from the stored decisions, never from the
   request), and runs one step per part, each with its own finding, check
   and record. A split item with no part selected, or whose parts lack their
   own findings, is rejected. The plan reports space freed right away, space
   held in the Trash, unrecoverable items (IDs and bytes) and which items
   need acknowledgment.
2. `acknowledge_item(planId, itemId)` records a per-plan acknowledgment for
   an item that needs one (Docker volumes, permanent deletions).
3. `run_plan(planId)` is the confirmation. It is rejected if the plan is
   unknown, altered (digest mismatch), expired, invalidated by a newer scan,
   already used, missing an acknowledgment, or another cleanup is running.
   Only after these checks does the engine create the `Confirmed` token the
   command runner requires for any action.
4. Items run one at a time on a background thread. For each item the state
   is persisted as `checking`, the pre-cleanup checks run, then `running`,
   then the final state (`done`, `moved_to_trash`, `skipped`, `failed`,
   `canceled`). Cancellation is checked only between items.
5. Free space is measured before and after. History stores the plan, each
   item snapshot and result, skip reasons, expected bytes and the measured
   change.

## Pre-cleanup checks

| Resource | Checks | Code |
| --- | --- | --- |
| All paths | Exists, canonical path and file identity unchanged (detects path-for-link swaps), not protected, not a link, same newest-mtime/file-count/size fingerprint | `detectors::files::probe_paths`, `guard` |
| Project folder | Manifest and lockfile evidence unchanged, no process running in the project | `detectors::projects::probe` |
| Worktree | Still clean, still pushed or merged, not locked, no new commits | `detectors::git::probe` |
| Docker image or volume | Still exists and unused by every container, including stopped ones | `detectors::docker::probe` |
| Docker container | Still stopped, unchanged since the scan | `detectors::docker::probe` |
| Caches (pnpm, pip, Homebrew, simulators, Gradle) | Same location; resource set has not grown; Gradle daemon not running | `detectors::caches::probe` |

Changed items are skipped with a reason; unknown results (tool missing,
timeout, daemon down) are skipped too. Skipped items stay in Suggestions with
the reason until the next cleanup of that item succeeds.

## Recovery policy

| Risk | Action | Frees space now | Recovery shown |
| --- | --- | --- | --- |
| Safe | Native command or guarded permanent delete of regenerable data | Yes | Regenerate with the owning tool |
| Review | Trash (paths) or native command (Docker, simulators) | Trash: no, until emptied | Restore from the Trash / Recycle Bin, or recreate |
| Risky | Docker volumes: individual acknowledgment; Git work: blocked | Yes | "Not recoverable." / "Your work exists only here." |

When a detector's process check is unknown, a Safe folder is raised to Review
and moved to the Trash instead of deleted.

## Native action scope

The engine never lets an aggregate command remove resources outside the
confirmed plan:

| Detector | Command run | Bound |
| --- | --- | --- |
| Dangling images | `docker image rm <exact IDs>` instead of `docker image prune` | Exact IDs from the plan; new dangling images are untouched (`aggregate_commands_stay_within_the_plan`). Docker refuses to remove an image a container uses. |
| Build cache | `docker builder prune --force --filter until=<seconds since scan>` | Only cache already unused at scan time; skipped if reclaimable cache grew by more than 10% + 100 MB. |
| Unused image, stopped containers, volume | `docker image rm <tags>`, `docker container rm <ids>`, `docker volume rm <name>` | Exact resources; no force flag. |
| Simulators | `xcrun simctl delete <exact UDIDs>` instead of `delete unavailable` | Exact devices; skipped if any listed device became available. |
| pnpm | `pnpm store prune` | Cannot be limited to a package list. Skipped if unreferenced store data grew by more than 5% + 50 MB since the scan. |
| pip | `pip cache purge` | The confirmed resource is the whole download cache. Skipped if its location changed or it grew by more than 5% + 50 MB. |
| Homebrew | `brew cleanup` | Skipped if `brew cleanup --dry-run` lists any path that was not in the plan. |

Only `docker builder prune` and `docker image prune` may carry `--force`
(the runner enforces this); `image prune` is not used at all. Banned commands
(`git worktree remove --force`, `git clean`, `docker system prune`,
`docker volume prune`, `npm cache clean`) are refused by the runner even
with a confirmed plan.

## Linked worktree removal (task 4.1, decided 2026-11-04)

The source protects repository roots (invariant 5) but also lists
`git worktree remove <path>`, and a linked worktree's root is a repository
root (it holds a `.git` file). The user decided that **linked worktree roots
are the one exception**. Everything else stays protected:

- Only Review worktrees (clean and merged, or clean and pushed but not merged,
  which includes squash merges) are selectable. Dirty, untracked, unpushed,
  no-upstream, locked, merging/rebasing, process-open and unknown worktrees
  stay Risky and blocked.
- Immediately before acting, `detectors::git::probe` re-lists the worktrees and
  re-classifies this one; a new commit, an edit, or any state change skips it.
- `Guard::check_linked_worktree` then requires: same canonical path as the
  scan, not a link, not the main worktree, a `.git` *file* whose `gitdir:`
  points into `<main>/.git/worktrees/`, and none of the other protected-path
  rules. The generic `Guard::check` still treats every repository root as
  protected.
- The command is `git worktree remove <path>` run in the main worktree,
  **never** with `--force`, so Git itself also refuses a worktree with local
  changes. The branch is kept; recovery is `git worktree add <path> <branch>`.

Tests: `guard::tests::linked_worktree_exception_is_narrow`,
`guard::tests::linked_worktree_symlink_is_refused`,
`cleanup::tests::clean_pushed_worktree_is_removed_without_force`,
`cleanup::tests::inv04_worktree_with_new_commit_after_scan_is_skipped`,
`cleanup::tests::inv04_worktree_edited_after_scan_is_skipped`,
`cleanup::tests::inv06_worktree_swapped_for_symlink_is_skipped`,
`cleanup::tests::inv09_unpushed_worktree_cannot_be_planned`.

## Trash

- Moving to the Trash records the canonical original path, time, measured
  size and file identity (device/inode or volume/file ID) in `trash_items`.
  These bytes are "held", not freed.
- `trash_summary()` returns the held bytes, item count and a confirmation
  token bound to that exact set of records. `empty_freeboard_trash(token)`
  permanently deletes only those items:
  - macOS: the entry in `~/.Trash` (or the volume's `.Trashes/<uid>`) whose
    file identity matches;
  - Linux: the freedesktop Trash item with the same original path, deletion
    time within 10 minutes and matching identity;
  - Windows: the Recycle Bin item with the same original path and deletion
    time within 10 minutes (identity is not available there).
  Items not found are marked "gone" (restored or emptied outside Freeboard).
  Unrelated Trash content is never touched.
- If a volume has no usable Trash, the item fails with a reason starting
  "Trash unavailable" and nothing is deleted. Permanent deletion is offered
  only as a new plan (`create_permanent_plan`) for such items; every item in
  it is unrecoverable and needs its own acknowledgment.

## Interruptions

On startup `recover_interrupted()` marks runs left `running` as
`interrupted`: in-flight items become `interrupted`, items that never started
become `canceled`. Nothing is replayed; the plan cannot be run again.

## Limitations

- The native Trash identity matching is tested with a directory-backed
  backend; the real platform round trip is an ignored test
  (`native_trash_round_trip_with_disposable_file`) to run manually on each
  platform with a dedicated disposable file.
- Fixture tests for cleanup use POSIX shell fakes and run on macOS and Linux.
- The engine has not been compiled for Linux or Windows in this environment
  (no cross targets installed); CI must build those targets.
- Docker Desktop may not return pruned space to the main disk right away;
  history shows the measured change next to the expected total.
