# Spec Delta

## Purpose

Remove only what the user confirmed, through backend-owned plans, with every item checked again immediately before action, guarded execution, honest Trash accounting and a durable Logbook record.

Source: `docs/reference/FREEBOARD_SPEC.md`, Safety invariants 2–13, Delete and restore policy, Cleanup rules, Pre-cleanup checks, Trash, Milestone 5.

## ADDED Requirements

### Requirement: Backend-built plans
The backend SHALL build and store a plan from the pile or from suggestion IDs, each with its selected part IDs, and return its contents and three totals: space freed right away, space held in the Trash, and items that cannot be recovered. The frontend SHALL send only plan and suggestion IDs. A plan SHALL expire after 30 minutes, or sooner if a new scan runs, and confirming an expired plan SHALL be rejected.

#### Scenario: Expired plan
- **WHEN** the user confirms a plan created 31 minutes ago, or created before the latest scan
- **THEN** nothing runs and the user is asked to make a new plan

#### Scenario: Three totals
- **WHEN** a plan has a native-command item, a Trash item and a Docker volume
- **THEN** the totals separate space freed now, space held in the Trash and 1 item that cannot be recovered

### Requirement: Per-item acknowledgment
Every item needing confirmation SHALL require its own acknowledgment in the confirmation, scoped to that plan. Confirm SHALL stay disabled until all are given.

#### Scenario: Missing acknowledgment
- **WHEN** the backend receives `run_plan` for a plan with an unacknowledged volume
- **THEN** it rejects the run and nothing changes

### Requirement: Pre-cleanup checks
Immediately before each item runs, Freeboard SHALL check its canonical path, protected-path rules, that no symlink was swapped in, and that its fingerprint is unchanged. It SHALL also check: worktrees are still clean, pushed or merged, unlocked and without new commits; project folders still have their manifest and no running process; Docker images and volumes are still unused by every container; containers are still stopped. A changed or unknown item SHALL be skipped with a reason, stay listed, and not block the rest of the plan.

#### Scenario: Changes between scan and cleanup
- **WHEN** after the scan a commit is added to a worktree, a file changes, a container starts using an image, and a path is replaced by a symlink
- **THEN** each of those items is skipped with its reason, the other items run, and the skipped items stay listed as changed

### Requirement: Protected paths and links
Cleanup SHALL canonicalize each path and reject the filesystem root, home directory, system directories, application directories, repository roots, and `.git` files or directories. It SHALL never follow symlinks or junctions, and removing a link SHALL remove only the link.

One exception applies, for `apps.unused` on macOS only: a single top-level, non-system `.app` bundle directly inside `/Applications` or `~/Applications` MAY be moved to the Trash, never deleted permanently, and only after its own acknowledgment in the plan. Nothing inside a bundle, no application directory itself, and no system app SHALL ever be touched.

#### Scenario: Traversal toward home
- **WHEN** a plan item's path resolves to the home directory through `..` or a link
- **THEN** the item is rejected and nothing is touched

#### Scenario: App exception is narrow
- **WHEN** a plan item targets `/Applications/GarageBand.app` with an acknowledgment, `/Applications` itself, a file inside `GarageBand.app`, or `/System/Applications/Mail.app`
- **THEN** only the first is moved to the Trash, and the others are rejected

#### Scenario: App needs acknowledgment
- **WHEN** a plan contains an unused app without its acknowledgment
- **THEN** the run is rejected

### Requirement: Restricted commands
Commands SHALL run as an executable plus an argument array, without a shell. `git worktree remove --force`, `git clean`, `docker system prune` (any form) and `docker volume prune` SHALL be refused. Force flags SHALL be allowed only for `docker builder prune` and `docker image prune` after plan confirmation. The npm cache SHALL be removed as a folder through the normal checks, not with `npm cache clean --force`.

#### Scenario: Banned command
- **WHEN** any code path asks the runner for a banned command or another force flag
- **THEN** the runner refuses it even if a plan was confirmed

### Requirement: Execution, cancellation and partial items
Only one cleanup SHALL run at a time. The user SHALL be able to cancel between items; cancellation SHALL never interrupt a running item. A failed item SHALL be recorded and the run SHALL continue. For split items only selected parts SHALL be removed, and unselected parts SHALL stay listed with updated sizes. Progress SHALL emit `cleanup_item_updated` and `cleanup_finished` events.

#### Scenario: Cancel
- **WHEN** the user cancels while item 3 of 8 is running
- **THEN** item 3 finishes, items 4–8 are recorded as canceled, and nothing else runs

#### Scenario: Partial removal
- **WHEN** a plan removes 2 of 3 Android emulators
- **THEN** the third stays listed afterward with its own size, and the map updates

### Requirement: Measured result
After a run Freeboard SHALL measure free space again and report the measured change alongside the expected total. For Docker Desktop it SHALL say that space may return later, and show the measured change instead of promising the expected amount.

#### Scenario: Measured differs
- **WHEN** expected freed space is 33.1 GB but measured free space rose 14.2 GB
- **THEN** the result and the Logbook both show 14.2 GB measured

### Requirement: Trash accounting
Items moved to the Trash SHALL be listed as held in the Trash until emptied or removed outside Freeboard. Emptying SHALL affect only items Freeboard moved there and SHALL need its own confirmation. If a volume has no usable Trash, the item SHALL stop and permanent deletion SHALL be offered as a new, separately confirmed plan. Windows SHALL use the Recycle Bin and say so.

#### Scenario: Empty only ours
- **WHEN** the Trash holds Freeboard's items and other files and the user confirms Empty it
- **THEN** only Freeboard's items are removed from the Trash

### Requirement: Logbook record
Every run, including tidy-rule runs, SHALL store the plan, each item's result and reason, and free space before and after. The Logbook SHALL list date, result, space freed, Trash amount and skipped items, and SHALL open run details.

#### Scenario: Run details
- **WHEN** the user opens a Logbook entry
- **THEN** it shows each item with its action and result, including skipped reasons
