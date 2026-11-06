# Spec Delta

## Purpose

Account for all used space on the monitored volume, show what Freeboard cannot explain, give a working fix for each blind spot, and let the user see what was checked, what is growing, and a shareable report.

Source: `docs/reference/FREEBOARD_SPEC.md`, Disk accounting, Blind spots, Places checked and growth, Disk map rules, Milestone 3.

## ADDED Requirements

### Requirement: Top-down accounting
Each scan SHALL start from used bytes of the monitored volume and assign every byte to one area: Projects, Docker, AI models, Caches, Your files, Phones and emulators, Apps and tools, or the operating system area (named macOS, Windows or Linux by platform). Each file SHALL be counted once, and a more specific detector SHALL take priority over its parent folder. Everything else SHALL go to Not explained. Coverage SHALL be explained bytes divided by used bytes.

#### Scenario: Nested detector priority
- **WHEN** a `node_modules` folder sits inside a scanned project folder
- **THEN** its bytes are counted once, in the Projects area as the dependency item, not also as project files

#### Scenario: Totals match
- **WHEN** a scan of the accounting fixture finishes
- **THEN** the sum of all areas, including Not explained, equals used bytes within 1%

### Requirement: Incomplete map is visible
If area totals differ from used bytes by more than 1%, the map SHALL say it is incomplete and show the difference. It SHALL NOT hide or redistribute it.

#### Scenario: Unreadable folder
- **WHEN** a folder in the fixture cannot be read
- **THEN** its space is in Not explained or reported as a blind spot, and coverage drops accordingly

### Requirement: Area details
Selecting an area SHALL show its suggestions, largest first, and the space to leave alone in that area, with a plain description. Not explained SHALL list the open blind spots instead.

#### Scenario: Leave-alone space
- **WHEN** the AI models area is 61.6 GB and its suggestions total 53.6 GB
- **THEN** the detail shows the suggestions and 8.0 GB of models used recently to leave alone

### Requirement: Blind spots with working fixes
Freeboard SHALL report blind spots of four kinds, each with an action: a folder outside the scan list (Scan this folder); a protected location (open the operating system permission flow); unknown files (mark them as mine, or inspect the folder); a tool not available (show how to start it, then scan again). A blind spot SHALL stay visible until resolved or marked as the user's. Sizes SHALL be shown as far as the system allows, and "unknown" otherwise.

#### Scenario: Scan this folder
- **WHEN** the user chooses Scan this folder on `~/archive`
- **THEN** the folder is added to the scan list, scanned, its findings appear, and coverage rises

#### Scenario: Mark as mine
- **WHEN** the user marks `~/tools/sim-data` as theirs
- **THEN** the blind spot is resolved, its bytes count as explained under Your files, and it is not raised again

#### Scenario: Permission not granted
- **WHEN** the user opens the permission flow for a protected location and does not grant access
- **THEN** the blind spot stays open; the result is never simulated

### Requirement: Places checked
Freeboard SHALL list every detector that ran as found, clean or unavailable. An unavailable detector SHALL never be shown as clean.

#### Scenario: Unavailable is distinct
- **WHEN** Docker is not running
- **THEN** Docker detectors appear as unavailable, with different styling and text from clean ones

### Requirement: Growth
Freeboard SHALL store daily area totals and show the fastest-growing areas over 7 days.

#### Scenario: Growth from snapshots
- **WHEN** fixture snapshots show Docker growing 9.3 GB and AI models 7.0 GB in 7 days
- **THEN** Docker is listed first, then AI models

### Requirement: Disk report
Freeboard SHALL produce a copyable JSON report with free space, coverage, areas, suggestions and blind spots. It SHALL contain sizes, folder names and risk levels, never file contents, and SHALL stay on the computer until the user pastes it somewhere.

#### Scenario: No contents
- **WHEN** a report is created over a fixture whose files contain a marker string
- **THEN** the marker string does not appear in the report
