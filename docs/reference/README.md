# Source references

Replaced on 2026-11-06 with the rebuild sources supplied by the user (`freeboard-prototype.html` and `Freeboard Product and Implementation Specification.md`). The previous versions, which described the hull-gauge interface, remain in git history.

On 2026-11-07 the user supplied a revised prototype (`Freeboard Prototype.html`) that changes only the Settings sheet; it replaced `freeboard-prototype.html` without the page wrapper the file was saved with.

Later on 2026-11-07 a second revision (`freeboard-prototype.html`) redesigned the helm: a Make room panel (segmented How much, From checkboxes with Safe sizes, a pile meter) and a crate-shaped Toss button with a crates toggle.

A third revision the same day (`freeboard-prototype-1.html`) redesigned the item rows (one line each, details on demand), the confirmation, the cleanup run (a 3D deck of crates) and its result (grouped, with Try again).

A fourth revision (`freeboard-prototype-2.html`) redesigned the disk map's right column (a Why-not-explained card grouping blind spots by cause, each with a fix) and listed areas under 3% as chips under the map.

A fifth revision (`freeboard-prototype-3.html`, the prototype's Version 15) added the “What Risky means here” box and folded Growing fastest, Places checked (with a See all sheet) and the second opinion into one card.

The user's Version 17 (`freeboard-prototype (1).html`) added the five-step first-run setup and redesigned the menu bar window, with the matching spec sections ("First-run setup", "Menu bar window"), which replaced `FREEBOARD_SPEC.md`.

The user's Version 21 (`Freeboard Prototype.html`, 2026-11-09) redesigned the menu bar window: a version chip with the update flow, a whole-disk bar at 60 GB free or more, the pace from this week's rate, a This week row, and a fixed header and footer around a scrolling middle. It also carries Version 20's worktree removal queue. The spec's matching revisions ("Menu bar window" revision v21, "Removal queue", "Installers") came with it.

- `FREEBOARD_SPEC.md`: product behavior, safety invariants, detector inventory, disk accounting, pile rules, API contract, milestones and acceptance criteria.
- `freeboard-prototype.html`: visual and interaction reference (Dinghy, the pile, Quick questions, Disk map, Logbook). Its data, cleanup and blind-spot results are simulations.

The spec wins on data, safety and behavior; the prototype wins on layout, flows, labels and tone. Conflicts are recorded in `docs/decisions.md`. The prototype loads Google Fonts (Fredoka, Nunito, DM Mono); production and visual tests must bundle those fonts locally.

Prototype-only parts that must not ship: example data, **Pretend free space**, **Start over**, the forced skip of `old-experiment`, and the simulated **Allow access** result.

## Integrity

- `FREEBOARD_SPEC.md` SHA-256: `38312dc20a8ea4adfffb6cd25ce0b0fd84f8b2487d0ba4bf21b02f24d35c796c` (updated 2026-11-09, Version 21: menu bar window, updates, removal queue)
- `freeboard-prototype.html` SHA-256: `3a6629f3f0016fd420e0d2c0b3befd024612c4a9aac752353c87f49130c8892e`
