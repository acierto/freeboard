# tidy-rules Specification

## Purpose
Offer optional rules that keep the disk tidy without asking, limited to named Safe detectors and held to the same checks and records as a confirmed cleanup.

Source: `docs/reference/FREEBOARD_SPEC.md`, Safety invariant 2 exception, Keep me tidy rules, Milestone 6.

## Requirements

### Requirement: Rules are off by default and explicit
Freeboard SHALL offer two rules, both off by default: "Clear package caches over 30 days old when space is Low", which covers only named Safe cache detectors; and "Clear unused Docker build cache every week", which covers only `docker.build_cache`. Turning a rule on SHALL state exactly what it will remove.

#### Scenario: Fresh install
- **WHEN** settings are read from an empty database
- **THEN** both rules are off and no rule has run

#### Scenario: Turning on
- **WHEN** the user turns on the package cache rule
- **THEN** the interface lists the detectors it covers before saving

### Requirement: Rules remove only Safe, unkept items
A rule SHALL act only on Safe items from the detectors it names. It SHALL never act on Review or Risky items, or on anything the user marked Keep. It SHALL pass the same pre-cleanup checks as a confirmed plan and stop when one fails.

#### Scenario: Kept cache
- **WHEN** the package cache rule runs and the user marked the pip cache Keep
- **THEN** the pip cache is untouched

#### Scenario: Review item in scope
- **WHEN** the package cache rule runs while a Review cargo cache exists
- **THEN** the cargo cache is untouched

### Requirement: Rules run quietly and leave a record
Rules SHALL run at low priority in the background, write the same Logbook record as a cleanup, and emit `tidy_rule_ran` with the rule, result and bytes freed. Rule runs SHALL be retained for 1 year.

#### Scenario: Weekly Docker rule
- **WHEN** a fake clock passes one week with the Docker rule on and Docker available
- **THEN** one build-cache prune runs, and the Logbook shows it as a tidy-rule run
