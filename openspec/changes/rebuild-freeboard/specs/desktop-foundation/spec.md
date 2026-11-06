# Spec Delta

## Purpose

Provide the local desktop shell for Freeboard: storage and retention, a locked-down frontend boundary, settings, first-run setup, and tray and background lifecycle on macOS, Linux and Windows.

Source: `docs/reference/FREEBOARD_SPEC.md`, Technical stack and architecture, Tauri permissions, Local storage, Default settings, Background behavior, Milestone 1.

## ADDED Requirements

### Requirement: Frontend cannot reach the filesystem or shell
The frontend SHALL be granted only Freeboard's own commands plus notification and autostart permissions. It SHALL NOT receive the shell or filesystem plugins. The content security policy SHALL block remote scripts and remote content. No command SHALL accept a path or command line to execute; commands accept IDs, enumerated values and validated settings only.

#### Scenario: Contract test denies direct access
- **WHEN** the contract tests call shell or filesystem APIs from the frontend context, or call any Freeboard command with a path or command string as the target
- **THEN** each call is rejected and nothing on disk changes

#### Scenario: No remote content
- **WHEN** the app loads in a webview with network access available
- **THEN** no request leaves the machine, including fonts, which load from bundled files

### Requirement: Local storage and retention
Freeboard SHALL store data only in a local SQLite database, with these tables and retention rules: settings (permanent); free_space_samples (hourly, 30 days); suggestions and suggestion_parts (replaced each scan); item_decisions (until changed); pile_state (until the next cleanup or reset); map_snapshots (daily, 30 days); blind_spot_decisions (until changed); tidy_rules (permanent) and tidy_runs (1 year); cleanup_runs and cleanup_items (1 year). Upgrading from an existing database SHALL migrate it without losing settings, samples or cleanup history.

#### Scenario: Retention pruning
- **WHEN** pruning runs on a fixture database with rows older than each table's limit
- **THEN** only the expired rows are removed

#### Scenario: Upgrade keeps history
- **WHEN** the app opens a database created by the previous schema
- **THEN** settings, samples and cleanup history are still readable, and the new tables exist

### Requirement: Settings with spec defaults
Settings SHALL default to: Low warning 20 GB, Critical warning 8 GB, goal 50 GB, Where from Everywhere, quick scan every hour, deep scan only on request, inactive project 30 days, old installer 90 days, Keep me tidy rules off, pause deep scans on battery on, start at login on, notifications on, and scan folders chosen during first run. Updates SHALL be validated, and a Critical threshold at or above the Low threshold SHALL be rejected.

#### Scenario: Fresh install defaults
- **WHEN** settings are read from an empty database
- **THEN** every value matches the defaults above

#### Scenario: Inconsistent thresholds
- **WHEN** an update sets Critical to 25 GB while Low is 20 GB
- **THEN** the update is rejected and the stored settings are unchanged

### Requirement: First-run setup
On first launch Freeboard SHALL explain what it reads and what it never does, ask the user to choose scan folders (offering detected folders that contain Git repositories), and request an operating system permission only when the user chooses a feature that needs it. The user SHALL be able to finish without choosing folders.

#### Scenario: Skip folders
- **WHEN** the user finishes first-run setup without choosing folders
- **THEN** monitoring starts, and the disk map shows the unscanned folders as blind spots

#### Scenario: No early permission prompt
- **WHEN** first-run setup completes
- **THEN** no operating system permission dialog has been shown

### Requirement: Tray and background lifecycle
Closing the window SHALL hide it to the tray, and the webview SHALL be destroyed after 60 seconds hidden. Monitoring, scheduled quick scans and enabled rules SHALL continue in the Rust core. The tray menu SHALL offer Open Freeboard, Scan Now, Pause Notifications and Quit. Reopening SHALL show the same state the core holds.

#### Scenario: Hidden state survives
- **WHEN** the window has been hidden for more than 60 seconds and the user opens it from the tray
- **THEN** the pile, decisions and free space match what they were before hiding, and any changes made by background work are included
