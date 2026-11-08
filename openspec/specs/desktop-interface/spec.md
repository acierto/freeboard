# desktop-interface Specification

## Purpose
Present Freeboard as the approved prototype shows it: one harbor home with Dinghy and the pile, Quick questions, Disk map and Logbook sections, sheets for confirmation and settings, and a tray view. Include the production states the prototype lacks, in its visual style.

Source: `docs/reference/FREEBOARD_SPEC.md`, Interface and app contract (Screen structure, Dinghy, Visual system, Disk map rules, Copy, Accessibility, Required behavior missing from the prototype); `docs/reference/freeboard-prototype.html`.

## Requirements

### Requirement: Screen structure
The app SHALL have one home view with three sections (Quick questions, Disk map, Logbook) selected by tabs, with Settings behind a gear button. The home SHALL show Dinghy, a speech bubble, free space, status, forecast, goal, Where from, the pile total, the main Toss button, See what is inside, and the Trash note with Empty it. The Quick questions tab SHALL show a count of open questions; the Disk map tab SHALL show the coverage percentage while space is unexplained. The tray view SHALL show status, free space, the pile total and a button to review the pile. Prototype-only controls (Pretend free space, Start over) SHALL NOT ship.

#### Scenario: Home layout matches the prototype
- **WHEN** the home view is captured at 1040 px content width with the reference data set, in light and dark
- **THEN** the screenshot matches the prototype capture of the same state, per the visual-verification capability

### Requirement: Dinghy shows disk health
Dinghy SHALL be an original code- or SVG-drawn boat. Its face SHALL be happy, worried or panicked with a sweat drop for Healthy, Low or Critical. Its deck height SHALL move in a straight line from 0 GB to 60 GB free, then stay still. It SHALL carry up to 9 crates, largest first, each labeled with its rounded GB. During cleanup crates SHALL leave the boat and the boat SHALL rise as measured free space changes. Status SHALL also appear as text. The scene SHALL respect reduced motion.

#### Scenario: Mood and height by status
- **WHEN** free space is 40, 12.4 and 4 GB with default thresholds
- **THEN** Dinghy is happy, worried and panicked respectively, sits at the height the 0–60 GB rule gives, and the status pill reads Healthy, Low and Critical

#### Scenario: Reduced motion
- **WHEN** the OS requests reduced motion
- **THEN** bobbing, waves, the boat transition and crate animation are off, and status changes still show instantly

### Requirement: Visual system
The interface SHALL use the prototype's color tokens in light mode and its night scene in dark mode, Fredoka for headings, Nunito for interface text and DM Mono for paths and commands, all bundled locally. Safe, Review and Risky SHALL use semantic colors and always include their text label. Corners SHALL be soft and spacing generous, with one primary action in view.

#### Scenario: Night scene
- **WHEN** the system theme is dark
- **THEN** the sky, stars, moon, sea and cards use the prototype's dark tokens, matching its dark capture

### Requirement: Disk map presentation
The disk map SHALL be a squarified treemap drawn to scale, with a coverage ring and "N% explained" text. Each area SHALL be a real button with a text label for assistive tools. Not explained SHALL use the striped, dashed style. Labels SHALL wrap or hide cleanly and never clip. The map SHALL sit beside blind spots, growth, places checked and the disk report, and stack on narrow widths.

#### Scenario: Small tiles
- **WHEN** an area's tile is too small for its label
- **THEN** the label is hidden visually but the button still has its accessible name and size

### Requirement: Sheets and confirmation
Confirmation, progress, result and Settings SHALL appear as sheets. The confirmation SHALL show the three totals, each item with its exact action, per-item acknowledgments, and keep the confirm button visible while a long list scrolls. Progress SHALL show the current item and a Cancel button. Emptying Trash SHALL ask for confirmation.

#### Scenario: Long plan
- **WHEN** the confirmation lists 30 items in a 390×844 window
- **THEN** the list scrolls and the confirm button stays visible

### Requirement: Copy
Dinghy MAY speak in the first person in the bubble, blind spots and disk map. Controls SHALL stay plain and use these labels: Toss, Add to pile, Remove now, Keep, Stop keeping, Keep, don't ask again, Ask me later, Choose which, Remove all here and Add all to pile. Windows and Linux SHALL use their own words, including Recycle Bin and the system area name.

#### Scenario: Windows labels
- **WHEN** the app runs on Windows
- **THEN** Trash notes say Recycle Bin and the system area is named Windows

### Requirement: Accessibility
Every control SHALL work by keyboard with a visible focus style. Sheets SHALL trap focus, close with Esc unless a cleanup is running, and return focus to the control that opened them. The tray view SHALL close with its close button, Esc and a click outside. Re-rendering a list SHALL keep open groups and keyboard focus in place. Nothing SHALL rely on color or animation alone.

#### Scenario: Focus kept on re-render
- **WHEN** the user ticks an item's checkbox inside an open group with the keyboard
- **THEN** the group stays open and focus stays on that checkbox

#### Scenario: Tray closes three ways
- **WHEN** the tray view is open
- **THEN** its close button, Esc, and a click outside each close it

### Requirement: States missing from the prototype
The interface SHALL include, in the prototype's style: first-run setup and the permission flow; Empty Trash confirmation; Cancel during cleanup; scan progress, scan failure and detector-unavailable states; empty states for a new install, no suggestions and an empty Logbook; Logbook run details; adding and excluding scan folders in Settings; and Windows and Linux labels.

#### Scenario: New install
- **WHEN** the app opens before any scan has finished
- **THEN** the home shows scan progress, Quick questions explains that the first scan is running, and the Logbook says no cleanups yet
