# Spec Delta

## Purpose

Find space the user could reclaim, read-only, across every detector family in the specification. Report each finding as one uniform suggestion with an accurate size, a risk level from fixed rules, and splittable parts where the item has them.

Source: `docs/reference/FREEBOARD_SPEC.md`, Data model and risk rules, Measuring size, Scans and detectors (all subsections), Safety invariants 1, 6, 8, 9, 10, 11, 12, Milestone 3.

## ADDED Requirements

### Requirement: Scans are read-only
Detectors SHALL only read files and run read-only commands, through the guarded runner, as an executable plus an argument array with a 20-second default timeout. Scanning SHALL never follow symlinks or junctions. Scans SHALL make no network calls of their own.

#### Scenario: Fixture unchanged
- **WHEN** a quick scan and a deep scan run over the fixture tree with fake tool executables
- **THEN** a before-and-after hash of every fixture file, link and directory is identical

#### Scenario: Symlink trap
- **WHEN** a scanned folder contains a symlink or junction pointing outside it
- **THEN** the link target is not read or counted

### Requirement: Quick and deep scans
A quick scan SHALL read tool reports and known cache locations, every hour by default. A deep scan SHALL walk the configured folders to find projects, worktrees and files, on request or daily if enabled, at low priority, and pause on battery by default. Each detector SHALL fail independently, and a timeout or missing tool SHALL mark only that detector unavailable. Scans SHALL emit `scan_progress` and `scan_finished` events.

#### Scenario: One tool fails
- **WHEN** the fake `docker` executable times out during a scan
- **THEN** Docker detectors are reported unavailable and every other detector still returns results

### Requirement: Uniform suggestion model
Every detector SHALL return suggestions with: a stable ID derived from the detector and resource identity, detector, group, map area, title, location, size of selected parts, full size, last-used time and label, risk, action, exact action label, reason, recovery, recoverable flag, blocked reason, confirmation flag, parts and fingerprint. Groups SHALL be Docker, Projects, Caches, AI models, Phones and emulators, Old tools and apps, Your files, and System.

#### Scenario: Every suggestion is explained
- **WHEN** any detector returns a suggestion
- **THEN** it has a non-empty size, risk, reason, action label and recovery

#### Scenario: Stable identity
- **WHEN** the same fixture is scanned twice
- **THEN** each resource has the same suggestion ID both times

### Requirement: Risk rules
Risk SHALL come from the detector's rules, not from size or age alone. Safe means fully regenerable by the owning tool; Review means probably unneeded but context-dependent; Risky means it may hold unique work or data. If a status check fails or times out, risk SHALL be raised, never assumed safe. Uncommitted or unpushed Git work SHALL be Risky and blocked. Docker volumes SHALL be Risky and require individual confirmation.

#### Scenario: Unknown status
- **WHEN** `git status` fails for a worktree
- **THEN** the worktree is Risky and blocked with a reason

### Requirement: Detector coverage
Freeboard SHALL implement every detector in the specification's detector summary, on the listed platforms, with the listed default risk and action. The detectors are: docker.build_cache, docker.dangling_images, docker.disk_slack, docker.unused_images, docker.stopped_containers, docker.volumes, git.worktrees, project.dependencies, project.build_output, cache.pnpm, cache.pip, cache.npm, cache.homebrew, cache.jetbrains, cache.electron_apps, cache.playwright, cache.cargo, cache.gradle, cache.xcode, ai.ollama, ai.huggingface, ai.lm_studio, mobile.ios_backups, mobile.android_avd, mobile.simulators, tools.node_versions, tools.rust_toolchains, apps.unused, files.duplicates, files.large_media, files.installers, files.windows_temp, system.snapshots and system.stale_temp. Before relying on a CLI flag or output format, the implementation SHALL check it against the installed tool. If a tool has no safe, supported way to act, the item SHALL be shown for information and kept out of cleanup.

#### Scenario: Detector fixture
- **WHEN** each detector runs against its fixture, including fake `docker`, `ollama`, `nvm`, `rustup`, `xcrun` and `tmutil` executables
- **THEN** it returns the expected suggestions, risk and action, or a clean or unavailable result

#### Scenario: No supported compaction
- **WHEN** Docker disk slack is found on a platform with no supported compaction method
- **THEN** the item shows its size for information and cannot be added to the pile or removed

### Requirement: Docker evidence
Docker detectors SHALL run only after `docker version` succeeds. An image SHALL count as used if any container references it, including stopped ones. A volume SHALL count as attached if any container mounts it, including stopped ones. Containers SHALL be suggested only when stopped for 14 days or more.

#### Scenario: Stopped container keeps image
- **WHEN** a tagged image is referenced only by a stopped container
- **THEN** it is not suggested as an unused image

### Requirement: Git worktree states
Freeboard SHALL discover repositories under the configured folders, read linked worktrees, and never suggest the main worktree. Clean and merged worktrees, and clean, pushed but unmerged worktrees, SHALL be Review. Worktrees with uncommitted or untracked files, unpushed commits or no upstream, a lock, a merge or rebase in progress, an open process, or an unknown status SHALL be Risky and blocked. Squash-merged branches SHALL show as pushed, not merged.

#### Scenario: Worktree fixture states
- **WHEN** fixture repositories cover each state above
- **THEN** each worktree gets the listed risk, and every Risky one is blocked with a reason

### Requirement: Project ownership evidence
Generated folders SHALL be suggested only with evidence: `node_modules` with `package.json` and a lockfile (Safe); `target` with `Cargo.toml` (Safe); `.venv` with `pyproject.toml` or `requirements.txt` (Review); `dist`, `build` or `.next` with a project manifest and the folder ignored by Git (Safe). A project SHALL count as inactive when no source file changed for 30 days. Projects with a running process SHALL be skipped. Freeboard SHALL NOT scan inside generated folders. Dependencies SHALL be split into one part per project.

#### Scenario: Unrelated build folder
- **WHEN** a folder named `build` has no manifest beside it
- **THEN** it is not suggested

### Requirement: Files and system detectors
Installers SHALL be suggested only for `.dmg`, `.pkg`, `.exe`, `.msi`, `.AppImage`, `.deb`, `.rpm` and `.iso` files in Downloads not opened or modified for 90 days. Media SHALL be video files of 500 MB or more not opened for 6 months, never photos or documents. Duplicates SHALL be found by size, then exact content hash, keeping one copy, and SHALL exclude hidden folders, app bundles and repositories. Apps SHALL be suggested only when last-opened data exists. Snapshots SHALL be suggested only when the newest backup can be confirmed.

#### Scenario: Near duplicates
- **WHEN** two files have the same size but different contents
- **THEN** they are not reported as duplicates

#### Scenario: Unconfirmed backup
- **WHEN** the newest Time Machine backup cannot be confirmed
- **THEN** the snapshots detector reports unavailable and suggests nothing

### Requirement: Splittable parts
Items with natural sub-units SHALL expose parts: projects for dependencies, models for Ollama and LM Studio, backups, emulators, apps, Node versions and toolchains. The item's size SHALL equal the sum of its selected parts. Part selections SHALL persist by stable resource ID.

#### Scenario: Deselect a part
- **WHEN** the user deselects a 26 GB model from a 41.2 GB Ollama item
- **THEN** the item's size becomes 15.2 GB and every total that includes it updates

### Requirement: On-disk size
Sizes SHALL be actual on-disk sizes (allocated blocks on macOS and Linux, compressed size on Windows). Hard-linked files SHALL be counted once. For pnpm-managed `node_modules`, only space that removal would free SHALL be reported. When a tool cannot report a size exactly, the size SHALL be shown with "about".

#### Scenario: Hard links
- **WHEN** a fixture holds one 1 GB file and three hard links to it
- **THEN** the reported size is 1 GB
