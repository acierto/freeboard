# Spec Delta

## Purpose

Prove that the rebuilt interface matches the approved prototype by rendering both in the same states and comparing screenshots, with evidence a reviewer can inspect and a gate that blocks on any mismatch or missing evidence.

Source: `docs/reference/FREEBOARD_SPEC.md`, Acceptance criteria (visual match), Visual system; `docs/reference/freeboard-prototype.html`; `openspec/config.yaml` rules.

## ADDED Requirements

### Requirement: Same-state rendering of reference and app
The verifier SHALL render the unmodified prototype, served through an adapter that swaps remote fonts for the bundled ones, sets the theme and hides prototype-only controls, and the app built against a fixture backend that mirrors the prototype's example data. Both SHALL be driven into the same states through their own controls, in pinned Chromium with fixed viewport, device scale, locale, timezone, clock and motion settings, and no network access.

#### Scenario: Prototype-only controls hidden
- **WHEN** the reference is captured
- **THEN** Pretend free space and Start over are not visible, and the prototype file on disk is unchanged

### Requirement: Required matrix
The matrix SHALL cover, in light and dark, at desktop, middle and phone widths: home at Healthy, Low and Critical; the pile open; goals 20, 50, 100 GB and All I can; a Where from limit; manual mode after a take-out; an empty pile; Quick questions with a Review card, a Risky card needing a tick and the finished state; Everything I found with a group open and Choose which open; the protected list; the Keep me tidy card; the disk map with each blind spot open and resolved; area details including Not explained; places checked open; the disk report; the Logbook; the confirmation sheet with three totals and an acknowledgment; the progress and result sheets; Settings; and the tray view. States absent from the prototype SHALL be marked supplemental, built only from prototype components, and listed for product-owner review.

#### Scenario: Matrix complete
- **WHEN** the verifier runs
- **THEN** every matrix entry produces reference, actual and diff images or is reported as missing

### Requirement: Blocking comparison
Captures SHALL be compared pixel by pixel with zero tolerance by default. Any tolerance SHALL be per entry, with a recorded experiment and rationale. The run SHALL fail on any mismatch and report incomplete when evidence or a required font is missing. A self-test SHALL prove the verifier fails when the font, Dinghy's shape, a color or spacing changes.

#### Scenario: Deliberate change
- **WHEN** the self-test changes the heading font
- **THEN** the verifier reports failure for the affected entries

### Requirement: Evidence and review
Each run SHALL produce a report with PASS, FAIL or INCOMPLETE per entry and overall, linking reference, actual and diff images, and showing each entry's review disposition. Native evidence (tray on each platform, and each platform's real webview) SHALL stay INCOMPLETE until captured and reviewed.

#### Scenario: Native evidence missing
- **WHEN** no macOS tray capture has been attached
- **THEN** the report lists the macOS tray row as INCOMPLETE and the gate does not pass
