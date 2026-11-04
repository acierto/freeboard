## Purpose

Discover reclaimable developer resources through read-only scans with conservative risk classification and honest size estimates.

Source: `docs/reference/FREEBOARD_SPEC.md`, Data model and risk rules, Scans and detectors. The complete detector summary and per-detector rules in that source are normative.

## ADDED Requirements

### Requirement: Read-only isolated scans
Quick scans SHALL read tool reports and known cache locations hourly by default; deep scans SHALL discover projects, worktrees, and files under configured folders on request or daily if enabled. Scans SHALL not mutate resources or follow links, junctions, or reparse points.

#### Scenario: Fixture scan
- **WHEN** both scan kinds run over fixtures containing symlink traps and excluded folders
- **THEN** fixture contents remain unchanged and excluded or linked targets are not traversed

### Requirement: Detector failure isolation
Missing tools, unavailable daemons, permissions, malformed output, and timeouts SHALL be surfaced per detector without stopping others. External commands SHALL time out after 20 seconds by default. Unknown status SHALL never increase safety.

#### Scenario: Docker unavailable
- **WHEN** Docker version checking fails or times out
- **THEN** Docker detectors show unavailable and unrelated detectors still produce results

### Requirement: Common suggestion contract
Each suggestion SHALL expose all fields of the source Suggestion contract, including stable identity, category, actual size, last use, risk, action, reason, recovery, confirmation needs, blocked reason, and fingerprint. Suggestions SHALL be grouped by category and sorted largest first within each group.

#### Scenario: Risk defaults
- **WHEN** suggestions are loaded
- **THEN** only eligible Safe items are preselected; Review and Risky items are not; blocked items cannot be selected

### Requirement: Allocated size accounting
Size SHALL represent allocated disk usage, count hard-linked data once, report only genuinely reclaimable pnpm-linked storage, and prefix inexact estimates with “about”.

#### Scenario: Shared data
- **WHEN** sparse files and hard-linked pnpm files are measured
- **THEN** apparent length and shared bytes are not misrepresented as independently reclaimable space

### Requirement: Docker resource classification
Docker detectors SHALL cover unused build cache, dangling images, unused tagged images, containers stopped for at least 14 days, and unattached volumes. Every container, including stopped containers, SHALL count when checking image and volume usage. Volumes SHALL be Risky and require individual confirmation.

#### Scenario: Stopped container references
- **WHEN** a stopped container references an image or mounts a volume
- **THEN** neither resource is offered as unused or unattached

### Requirement: Worktree classification
Only linked worktrees SHALL be suggested. Clean merged or pushed worktrees SHALL be Review. Dirty, untracked, unpushed, no-upstream, locked, merging, rebasing, process-open, and unknown-state worktrees SHALL be Risky and blocked.

#### Scenario: Worktree state matrix
- **WHEN** fixtures cover every listed state including squash-merged branches
- **THEN** the main worktree is absent, unsafe states are blocked, and squash merges are conservatively described as pushed but not merged when applicable

### Requirement: Project ownership and inactivity
Generated folders SHALL require the source ownership evidence and 30 days without source changes. Active-process projects SHALL be skipped; generated folder contents SHALL not be searched for nested projects.

#### Scenario: Ownership evidence
- **WHEN** a scan finds node_modules, target, .venv, dist, build, or .next
- **THEN** node_modules needs package.json plus a lockfile, target needs Cargo.toml, .venv needs pyproject.toml or requirements.txt and is Review, and dist/build/.next need a manifest plus Git ignore evidence

### Requirement: Cache detector coverage
The app SHALL cover pnpm, pip, npm _cacache, Homebrew, Cargo, Gradle, Xcode DerivedData, and unavailable simulators with the risks, actions, and platforms in the source detector summary. Gradle cleanup SHALL require no running daemon.

#### Scenario: Platform cache rules
- **WHEN** cache fixtures are scanned on each target platform
- **THEN** unsupported detectors are excluded, Cargo and Gradle are Review, and npm cleanup never requires a force flag

### Requirement: Old file scope
Downloads suggestions SHALL include only the source installer extensions unopened and unmodified for 90 days. Windows SHALL support old TEMP files as Review Recycle Bin candidates. Documents and media SHALL not be suggested in v1.

#### Scenario: Downloads filtering
- **WHEN** Downloads contains old .dmg, .pkg, .exe, .msi, .AppImage, .deb, .rpm, .iso, documents, media, and recently used installers
- **THEN** only the qualifying old installers are Review suggestions for Trash
