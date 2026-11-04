## Purpose

Provide a private and restricted desktop foundation that persists local state and supports all three target operating systems.

Source: `docs/reference/FREEBOARD_SPEC.md`, Technical stack, Local storage, Background behavior, Default settings, Platform notes.

## ADDED Requirements

### Requirement: Restricted frontend
The frontend SHALL have no filesystem or shell plugin access and SHALL request cleanup using backend-owned IDs only. Content policy SHALL forbid remote scripts and content.

#### Scenario: Untrusted command input
- **WHEN** a frontend request supplies a path or executable instead of a supported ID
- **THEN** the backend rejects it and no filesystem mutation or command execution occurs

### Requirement: Local persistence and privacy
The app SHALL keep settings, samples, suggestions, plans, and history locally and SHALL make no network calls of its own. Developer tools can access the network only when run on request.

#### Scenario: Retention
- **WHEN** stored data reaches its retention limit
- **THEN** hourly samples older than 30 days and cleanup history older than one year are removed; settings persist and each scan replaces the latest suggestions

### Requirement: Settings and onboarding
The app SHALL persist thresholds, scan schedule, folders, exclusions, startup, and notifications using the defaults in the source specification. First run SHALL explain reads, request folders, and request OS permissions only when needed.

#### Scenario: Fresh installation
- **WHEN** a user launches a fresh installation
- **THEN** Low is 20 GB, Critical is 8 GB, quick scans are hourly, deep scans are on request, inactivity is 30 days, installers age at 90 days, and battery pause, login startup, and notifications are enabled

### Requirement: Background lifecycle
Closing the window SHALL hide it to the tray and destroy the webview after 60 seconds hidden while monitoring and scheduled scans continue. The tray SHALL offer Open Freeboard, Scan Now, Pause Notifications, and Quit.

#### Scenario: Reopen after destruction
- **WHEN** a user opens Freeboard after the hidden webview was destroyed
- **THEN** the interface is recreated from backend state without losing monitoring or cleanup state

### Requirement: Platform behavior
The app SHALL support macOS, Linux, and Windows, including platform Trash, permissions, fixed volumes, and native tray behavior. WSL filesystems SHALL be excluded.

#### Scenario: Platform acceptance
- **WHEN** the platform acceptance suite runs
- **THEN** macOS reports APFS purgeable space and snapshots separately; Linux respects XDG locations and works on GNOME and KDE; Windows supports long paths and rejects junction traversal
