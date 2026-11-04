## Purpose

Ensure the verification tool checks that implemented visuals match the supplied Freeboard design and produces reviewable evidence before declaring completion.

Source: explicit user requirement to verify implementation against the design; `docs/reference/freeboard-prototype.html`. Capture procedures and matrix are specified in this change's design.md.

## ADDED Requirements

### Requirement: Design-based verification
The verification tool SHALL render both the supplied prototype and implemented interface, compare matching screen states, and inspect layout, spacing, typography, colors, borders, iconography, hull geometry, charts, and control states. DOM assertions or functional tests alone SHALL NOT satisfy visual verification.

#### Scenario: Functionally correct visual mismatch
- **WHEN** navigation and cleanup tests pass but the gauge, font, spacing, color, or layout differs from the design
- **THEN** verification reports the affected screen and mismatch and fails until corrected or explicitly accepted as a design change

### Requirement: Deterministic reference capture
Reference captures SHALL derive from the preserved prototype, not from the implementation under test. Captures SHALL pin fixture data, viewport, theme, fonts, clock, motion, rendering engine, and device scale. Only documented prototype chrome and nondeterministic regions SHALL be excluded.

#### Scenario: Missing font or baseline
- **WHEN** the reference font cannot load or a required reference screenshot is absent
- **THEN** verification reports incomplete evidence and does not silently accept fallback fonts or generate a passing baseline from the implementation

### Requirement: Screen and state coverage
The verification matrix SHALL cover Overview, Suggestions, plan review, running/completed cleanup, History, Settings, and tray/status surfaces. It SHALL cover light/dark themes, desktop/narrow layouts, Healthy/Low/Critical, risk filters, expanded/blocked rows, acknowledgments, skips, and Trash confirmation.

#### Scenario: Required matrix entry omitted
- **WHEN** a required screen, theme, viewport, or state has no comparison
- **THEN** the verification report identifies it as missing and the visual gate fails

### Requirement: Reviewable visual evidence
Each comparison SHALL retain reference, actual, and diff or overlay images plus screen/state, viewport, OS, engine, theme, source revision, tolerance, and result. A human or visual-capable verifier SHALL review the design fidelity alongside automated differences.

#### Scenario: Verification report
- **WHEN** the visual suite finishes
- **THEN** a report links every comparison and review result, lists deviations and resolutions, and gives an explicit pass, fail, or incomplete status

### Requirement: Controlled tolerances and baseline changes
Visual comparison SHALL use explicit versioned tolerances with evidence for rendering noise. Baselines SHALL NOT be auto-updated merely to make a failing implementation pass. Intentional changes SHALL record rationale, affected screens, and explicit design acceptance before changing the reference.

#### Scenario: Regression accepted as baseline
- **WHEN** a developer tries to replace a failing baseline with the current application screenshot without accepted design rationale
- **THEN** the verification gate remains failed

### Requirement: Milestone visual gates
Each milestone delivering UI SHALL pass visual verification for its available screens. Final release verification SHALL cover the full matrix and actual Tauri rendering on macOS, Linux, and Windows. Browser-only checks SHALL not establish native platform acceptance.

#### Scenario: Unavailable target platform
- **WHEN** browser comparisons pass but native evidence for a target platform is missing
- **THEN** the release gate remains incomplete and reports the missing platform
