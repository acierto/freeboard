## Purpose

Present disk pressure and cleanup decisions through the supplied Freeboard design with accessible, consistent desktop screens.

Sources: `docs/reference/FREEBOARD_SPEC.md`, Interface and app contract; `docs/reference/freeboard-prototype.html`, visual tokens, styles, SVGs, and screen flow. Sample numbers are fixtures, not production requirements.

## ADDED Requirements

### Requirement: Prototype fidelity
The interface SHALL reproduce the reference layout, spacing, typography, colors, borders, controls, risk shapes, charts, and light/dark themes. Prototype-only simulation controls and mock OS chrome SHALL not appear in the production app. Rendered screenshots SHALL be compared against the reference as specified by visual-verification.

#### Scenario: Screen comparison
- **WHEN** each implemented screen is rendered with reference fixture data and matching viewport/theme
- **THEN** its app content matches the corresponding prototype region, with any intentional difference explicitly documented and reviewed

### Requirement: Hull gauge
Overview SHALL retain the ship hull gauge showing the top 120 GB, deck line at zero free, rising water as space runs out, and labeled Low/Critical markers. Free space and status SHALL appear in text as well as color.

#### Scenario: Gauge pressure states
- **WHEN** fixture free space changes from Healthy through Low to Critical and reaches zero
- **THEN** water moves toward the deck, thresholds and text remain correct, and the hull graphic is not replaced by a generic progress indicator

### Requirement: Overview information
Overview SHALL show status, free and total capacity, conditional forecast, Safe-item review action, scan state, category/risk bars, 14-day trend, and held Trash space.

#### Scenario: Insufficient forecast data
- **WHEN** the monitor has insufficient history
- **THEN** Overview omits the forecast while retaining the gauge, status, scan action, and valid resource totals

### Requirement: Suggestions interaction
Suggestions SHALL provide risk filters, expandable reason/action/recovery details, category totals, size ordering, blocked reasons, and a selection summary leading to plan review.

#### Scenario: Blocked selection
- **WHEN** a user filters to Risky and expands a blocked worktree
- **THEN** its reason and recovery details are visible and its checkbox cannot be selected

### Requirement: Plan and run presentation
Plan review SHALL show the three source totals, expected resulting space, exact actions, recovery, and acknowledgments. Run view SHALL show per-item checks/results, skip reasons, cancellation, measured final results, and separately confirmed Trash emptying.

#### Scenario: Required acknowledgment
- **WHEN** an unrecoverable item needing acknowledgment is in the plan
- **THEN** confirmation remains disabled until acknowledgment and backend validation both permit execution

### Requirement: History and settings screens
History SHALL show date, items, measured freed space, Trash amount, skipped items, and run details. Settings SHALL expose thresholds, scheduling, folders, exclusions, login startup, and notifications.

#### Scenario: Persistent settings
- **WHEN** settings change and the window is recreated
- **THEN** saved values are restored and status and gauge threshold labels reflect them

### Requirement: Responsive accessible states
The app SHALL preserve the prototype responsive transitions at 760 px and 480 px, keyboard navigation, visible focus, non-color risk labels, and reduced-motion behavior. Empty, loading, unavailable, failure, expired-plan, and canceled states SHALL remain usable.

#### Scenario: Narrow reduced-motion view
- **WHEN** a 390 px window uses reduced motion
- **THEN** navigation remains reachable, main grids stack, no content is clipped, and gauge waves and transitions do not animate

### Requirement: Backend contract
The interface SHALL use the commands and events enumerated in the source app contract, with typed inputs/results and no direct tool or filesystem access. Backend state SHALL be authoritative.

#### Scenario: Cleanup update
- **WHEN** cleanup_item_updated or cleanup_finished arrives
- **THEN** the matching run reflects backend item states, skips, and measured bytes instead of prototype arithmetic
