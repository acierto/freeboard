# Design

## Context

See proposal.md for motivation and scope. The repository was empty. The ZIP supplied a 26 KB product specification and 48 KB standalone interactive prototype, preserved byte-for-byte in `docs/reference/`. No application, tests, packaging, or measured performance exists yet. The six capability specs describe the proposed behavior.

The product specification governs safety and behavior; the prototype governs visual appearance and interaction structure. Its data and cleanup simulation are not production logic. Source references are relative to the repository root.

## Goals / Non-Goals

**Goals:** isolate privileged operations in Rust, make destructive actions auditable, ship read-only functionality first, and make design fidelity independently verifiable.

**Non-goals:** redesigning the supplied interface, implementing prototype simulation controls in the app, and claiming performance or platform verification before measurements exist. The proposal lists product exclusions.

## Decisions

### Backend authority and contracts

Use the supplied Tauri 2 / Rust stable / Tokio architecture with React, TypeScript, and Vite. Retain the source module layout: monitor, scan, detectors, cleanup, platform, commands, runner, guard, store, and tray. Centralize all filesystem access in guarded adapters and command spawning in the runner. Use executable/argument arrays, timeouts, allowlisted actions, and bounded output. This makes the safety boundary testable; frontend filesystem/shell plugins would defeat it.

Use the source Suggestion and CleanupPlan contracts without dropping fields. Additional backend-only state stores immutable plan snapshots, acknowledgments, scan generation, execution state, and scoped Trash identities. IDs resolve to backend-owned paths and commands. Expose all source commands and events through typed wrappers; reject unknown IDs and arbitrary path/command parameters. Notifications and autostart receive only the required permissions; CSP blocks remote content.

### Persistence and execution

Use SQLite migrations through rusqlite for settings, samples, suggestions, cleanup_runs, and cleanup_items with source retention. Persist confirmation and per-item transitions before/after action; record interrupted executions conservatively without replaying deletion automatically. Use a single cleanup lock. Cancellation checks occur between items. Measure volume space before and after runs rather than summing item estimates.

Use jwalk, sysinfo, trash, and tracing as the proposed source stack, subject to fixture and platform validation. Pin chosen stable versions and lockfiles at implementation time. Avoid speculative package versions now. Logs rotate locally and contain no file contents. Platform adapters handle allocated size, identity, process detection, links, permissions, and Trash tracking. A generic recursive delete without identity and protected-path validation is unsuitable.

### Detectors and native action scope

Implement the full detector summary and detailed rules in `docs/reference/FREEBOARD_SPEC.md`. Validate installed CLI formats and flags before relying on them; fake executables test missing tools, timeout, malformed output, and resource changes. Never run real cleanup as a test. Preserve uncertainty as increased risk or blocked actions.

Aggregate commands such as builder prune, image prune, pnpm store prune, pip cache purge, and brew cleanup need resource-set previews and scope revalidation. A backend cannot claim an exact plan while allowing an aggregate command to remove newly discovered resources. If a tool cannot bound its action to the confirmed snapshot, block that action and document the conflict; do not weaken confirmation.

### Source conflicts and safe behavior

The source protects repository roots but also permits native removal of linked worktrees. Preserve both requirements in planning and keep linked-worktree deletion disabled until the user clarifies the protected-root boundary; read-only discovery and risk presentation can proceed. Record the decision before implementing this destructive path. Do not silently exempt linked roots.

The prototype uses remote Google fonts, simplified thresholds/forecast, in-memory cleanup, and expected-size arithmetic. Bundle licensed Barlow Condensed, IBM Plex Sans, and IBM Plex Mono fonts locally for offline runtime and deterministic comparison. Implement actual hysteresis, forecast eligibility, expiry, backend acknowledgments, cancellation, persistence, and measured changes from the product spec. The prototype's mock menu bar and simulator are reference tooling, not app chrome. Native tray and notifications use platform UI with the supplied status-ring visual language.

### Visual verification tool

Provide a documented `verify:visual` package script backed by browser automation (proposed Playwright) for deterministic app-content comparisons, plus native Tauri capture instructions/adapters for the three OS webviews. Browser comparison is an early feedback tool, not a substitute for native platform evidence. The verification tool must actually check that visuals were implemented as in the supplied design.

Render the original prototype through a test-only adapter that serves local font assets, sets theme/data/state, freezes time and motion, and crops to `.app` or the corresponding component. Preserve the original reference file unchanged. Capture the implementation with the same fixture content, content viewport, fonts, engine, device scale 1, and reduced-motion setting. Wait for fonts and layout to settle. For running cleanup, use a controlled fake backend to hold a specific item state. Do not mask the hull gauge, risk markers, typography, or layout to suppress failures.

Capture reference-derived baselines before judging implementation. Store baseline images and a manifest with source hash, crop, fixture, engine, OS, fonts, viewport, and theme. Retain actual and diff/overlay images with a machine-readable result and linked HTML/Markdown review report. Start with zero differing pixels in the pinned browser environment; any antialiasing tolerance adjustment requires a recorded experiment and review, not an arbitrary blanket allowance. Native comparisons can use per-engine references and documented tolerances, plus visual review for design fidelity.

Minimum matrix (both light and dark for every listed row):

| Surface | Required states | Capture sizes |
| --- | --- | --- |
| Overview | Healthy 40 GB, Low 12.4 GB, Critical 4 GB; 0/120/>120 GB gauge bounds; Trash held; no forecast | 1120×900, 760×900, 390×844 |
| Suggestions | All/Safe/Review/Risky filters; expanded details; blocked row; selection summary; empty results | 1120×900, 390×844 |
| Plan review | No selection; mixed immediate/Trash/unrecoverable items; acknowledgment unchecked/checked | 1120×900, 390×844 |
| Cleanup run | Checking; success; skipped change; failure; canceled; measured completion; Trash confirmation | 1120×900, 390×844 |
| History | Populated and empty; run details | 1120×900, 390×844 |
| Settings | Defaults and changed thresholds; platform-specific labels | 1120×900, 390×844 |
| Tray/status | Healthy, Low, Critical; open menu and notification | Native macOS, Linux GNOME/KDE, Windows |

Additionally capture 761/760 and 481/480 px boundary pairs for the affected navigation, grids, and totals. The dimensions refer to the test viewport; manifest crops must align app content on both sides. Add keyboard focus and reduced-motion checks. Unsupported prototype states (errors, cancellation, onboarding, run details) use reviewed supplemental designs derived from its existing components; label them as additions rather than pretending the source contains them.

The verifier checks geometry, layout, spacing, font family/weight/size, palette, borders/radii, risk shapes, hull waterline/markers, chart styling, and control states. Automated diff results require human or visual-capable verifier review. Any unaccepted mismatch or missing required capture fails the gate. Baseline updates require explicit design acceptance with rationale in `docs/decisions.md`. CI must not update baselines automatically.

### Milestone and source traceability

| Source sections | Capability | Milestone |
| --- | --- | --- |
| Stack, permissions, storage, defaults, platform notes | desktop-foundation | 1 and 5 |
| Disk status, forecast, background | disk-monitoring | 2 |
| Data model, size, scans, all detectors | resource-detection | 3 |
| All 13 invariants, recovery policy, plans, Trash, history | safe-cleanup | 1–4 |
| Interface, command/event contract, prototype | desktop-interface | 1–5 |
| User-requested design comparison | visual-verification | 1–5 |

Preserve the full source as the detailed normative inventory, including extension lists, detector action tables, and all command/event names. Tests map to both capability scenarios and source acceptance criteria. Each milestone owns its tests and documentation and stops after its exit criteria pass on all three platforms.

## Risks / Trade-offs

- Path replacement races → use platform identity checks and safe handle-relative operations where available; fail closed where safety cannot be established.
- Tool-level cleanup races or broad scope → validate native action sets and block unbounded cleanup.
- Repository-root/worktree conflict → keep destructive worktree removal blocked pending explicit clarification.
- Font licensing or capture differences → bundle licensed assets, preserve license files, pin capture environment, and document legitimate platform differences.
- Trash tracking limitations → never empty unrelated items; fail an item when scoped identity cannot be established.
- Performance targets may be missed → measure installer ≤15 MB, hidden memory ≤40 MB, idle CPU <1% over five minutes, quick scan <10 seconds, deep scan of one million SSD files <60 seconds; report actual results and resolve misses before final acceptance.
- Platform runners/signing unavailable → mark evidence incomplete and keep release gate open rather than report success.

## Migration Plan

Greenfield implementation has no user-data migration initially. Introduce versioned SQLite migrations before persistence ships and test upgrades with fixture databases. Land foundation, monitoring, read-only detection, cleanup, and release readiness sequentially. Cleanup remains unavailable until its safety gate passes. Rollback must preserve audit data and never replay pending destructive actions.

## Open Questions

Deferrable release decisions from the source: open-source license, first signed release OS, and final product name. Resolve before public distribution. The destructive worktree ambiguity is explicitly blocked above, not deferred as permission to implement it.
