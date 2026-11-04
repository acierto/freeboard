# Proposal

## Why

Developer workstations accumulate Docker data, worktrees, dependencies, builds, and caches. Freeboard warns before disk space runs out and explains what can be reclaimed safely, using the supplied product specification and interactive design.

## What Changes

- Build the local desktop app in five gated milestones: foundation, monitoring, read-only detection, cleanup, and release readiness.
- Preserve the original specification and prototype under `docs/reference/` as traceable sources.
- Add monitoring, conservative suggestions, confirmed cleanup plans, scoped Trash handling, and local history.
- Reproduce the prototype's hull gauge, screen structure, typography, colors, themes, and responsive layouts.
- Require the verification tool to compare rendered implementation screenshots to the supplied design and report mismatches. Functional checks alone cannot pass visual acceptance.

## Capabilities

### New Capabilities

- `desktop-foundation`: Local storage, permissions, settings, desktop lifecycle, and platform support.
- `disk-monitoring`: Sampling, status transitions, notifications, forecast, and background monitoring.
- `resource-detection`: Read-only scans, detector rules, risk classification, and accurate size reporting.
- `safe-cleanup`: Confirmed plans, guarded execution, revalidation, Trash, and audit history.
- `desktop-interface`: Prototype-derived screens, hull gauge, selection flows, and accessibility.
- `visual-verification`: Repeatable reference comparisons, evidence, and a blocking fidelity gate.

### Modified Capabilities

None; this repository has no existing application or specifications.

## Impact

Introduces the React/TypeScript frontend, Tauri 2 Rust backend, SQLite persistence, local tool integrations, fixture tests, packaging, and cross-platform CI. No automatic deletion, cloud accounts, analytics, remote management, duplicate detection, treemap, WSL filesystem scanning, or Docker virtual-disk compaction is included. This change is planning only until implementation is requested.
