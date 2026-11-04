## Purpose

Reclaim space only through explicitly confirmed local plans with conservative revalidation, bounded execution, and durable action records.

Source: `docs/reference/FREEBOARD_SPEC.md`, Safety invariants, Delete and restore policy, Cleanup rules, Pre-cleanup checks, Trash.

## ADDED Requirements

### Requirement: Exact saved plan confirmation
The backend SHALL create and persist plans from suggestion IDs, show immediate space, Trash-held space, unrecoverable items, and recovery methods, and execute only the exact confirmed plan. Plans SHALL expire after 30 minutes or a new scan. Required acknowledgments SHALL be per item and scoped to that plan.

#### Scenario: Invalid confirmation
- **WHEN** a plan is expired, invalidated by a scan, altered, or missing an item acknowledgment
- **THEN** execution is rejected without any cleanup action

### Requirement: Immediate resource revalidation
Each item SHALL be checked immediately before action for identity, canonical path, fingerprint, modification state, and applicable tool status. Changed or unknown items SHALL be skipped with reasons without blocking other items.

#### Scenario: Concurrent changes
- **WHEN** a file changes, a Git commit appears, a container starts using an image or volume, or a path is swapped for a symlink after scan
- **THEN** that item is skipped, retained in Suggestions with an explanation, and recorded in history

#### Scenario: Resource-specific checks
- **WHEN** an unchanged resource is considered for cleanup
- **THEN** worktrees are still clean and pushed or merged and unlocked; projects retain manifests and have no running process; images and volumes are unused by every container; containers remain stopped

### Requirement: Protected paths and links
Cleanup SHALL reject canonical filesystem roots, home, system and application directories, repository roots, and .git files or directories. Scanning and deletion SHALL never follow symlinks or junctions; removing a permitted link SHALL affect only the link.

#### Scenario: Canonicalization attack
- **WHEN** traversal, an alias, a junction, or a symlink resolves toward a protected target
- **THEN** the operation is rejected and the target is unchanged

### Requirement: Restricted command execution
Commands SHALL execute as an executable plus argument array without a shell. Git clean, forced worktree removal, Docker system prune, and Docker volume prune SHALL be banned. Force flags SHALL be allowed only for Docker builder prune and image prune after exact plan confirmation.

#### Scenario: Forbidden commands
- **WHEN** any cleanup path attempts a prohibited command or another force flag
- **THEN** execution is refused even if a plan was confirmed

### Requirement: Plan scope bounds native cleanup
A native cleanup action SHALL not remove resources outside the confirmed plan. A changed native resource set SHALL require skipping or rebuilding and reconfirming the plan.

#### Scenario: Aggregate prune changes
- **WHEN** additional prune-eligible resources appear after a plan is saved
- **THEN** the backend does not silently expand the confirmed deletion scope

### Requirement: Risk and recovery enforcement
Git work with uncommitted or unpushed changes SHALL be blocked. Docker volumes SHALL never be preselected and SHALL require individual acknowledgment. Safe regenerable items SHALL use native cleanup or permanent deletion; Review items SHALL use Trash where possible or the specified native action.

#### Scenario: Unrecoverable volume
- **WHEN** a user selects an unattached Docker volume
- **THEN** the plan states its exact recovery or that it is not recoverable and cannot execute until its individual acknowledgment is recorded

### Requirement: Serialized cancellable execution
Only one cleanup SHALL run at a time. Cancellation SHALL take effect between items without interrupting an active action. Individual failures SHALL be recorded and remaining items processed unless canceled.

#### Scenario: Cancel during action
- **WHEN** cancellation is requested while an item is executing
- **THEN** that item completes and is recorded while subsequent items do not start

### Requirement: Scoped Trash handling
Trashed bytes SHALL remain held rather than freed until emptied. Emptying SHALL affect only items moved by Freeboard and require separate confirmation. Unusable Trash SHALL stop the item and require a new separately confirmed plan for permanent deletion.

#### Scenario: Unrelated Trash contents
- **WHEN** the user confirms emptying Freeboard Trash items
- **THEN** unrelated Trash contents remain untouched and measured free space is refreshed

### Requirement: Durable action history
Each run SHALL record the plan, item snapshots, results, errors and skips, and before/after free space. Results SHALL distinguish expected reclaim from measured change, including Docker Desktop virtual-disk behavior.

#### Scenario: Partial run
- **WHEN** a run includes successes, failures, and skipped items
- **THEN** History exposes every result and the measured change without claiming the expected total was freed

### Requirement: Safety invariant verification
Every one of the 13 source safety invariants SHALL have an automated test. Scanning and cleanup tests SHALL operate only on temporary fixtures and fake tools, never real developer resources.

#### Scenario: Safety gate
- **WHEN** cleanup milestone verification runs
- **THEN** a traceability report maps invariants 1 through 13 to passing tests and any missing or failing test blocks milestone completion
