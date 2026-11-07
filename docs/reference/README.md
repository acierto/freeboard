# Source references

Replaced on 2026-11-06 with the rebuild sources supplied by the user (`freeboard-prototype.html` and `Freeboard Product and Implementation Specification.md`). The previous versions, which described the hull-gauge interface, remain in git history.

On 2026-11-07 the user supplied a revised prototype (`Freeboard Prototype.html`) that changes only the Settings sheet; it replaced `freeboard-prototype.html` without the page wrapper the file was saved with.

Later on 2026-11-07 a second revision (`freeboard-prototype.html`) redesigned the helm: a Make room panel (segmented How much, From checkboxes with Safe sizes, a pile meter) and a crate-shaped Toss button with a crates toggle.

A third revision the same day (`freeboard-prototype-1.html`) redesigned the item rows (one line each, details on demand), the confirmation, the cleanup run (a 3D deck of crates) and its result (grouped, with Try again).

A fourth revision (`freeboard-prototype-2.html`) redesigned the disk map's right column (a Why-not-explained card grouping blind spots by cause, each with a fix) and listed areas under 3% as chips under the map.

A fifth revision (`freeboard-prototype-3.html`, the prototype's Version 15) added the “What Risky means here” box and folded Growing fastest, Places checked (with a See all sheet) and the second opinion into one card.

The user's Version 17 (`freeboard-prototype (1).html`) added the five-step first-run setup and redesigned the menu bar window, with the matching spec sections ("First-run setup", "Menu bar window"), which replaced `FREEBOARD_SPEC.md`.

- `FREEBOARD_SPEC.md`: product behavior, safety invariants, detector inventory, disk accounting, pile rules, API contract, milestones and acceptance criteria.
- `freeboard-prototype.html`: visual and interaction reference (Dinghy, the pile, Quick questions, Disk map, Logbook). Its data, cleanup and blind-spot results are simulations.

The spec wins on data, safety and behavior; the prototype wins on layout, flows, labels and tone. Conflicts are recorded in `docs/decisions.md`. The prototype loads Google Fonts (Fredoka, Nunito, DM Mono); production and visual tests must bundle those fonts locally.

Prototype-only parts that must not ship: example data, **Pretend free space**, **Start over**, the forced skip of `old-experiment`, and the simulated **Allow access** result.

## Integrity

- `FREEBOARD_SPEC.md` SHA-256: `32eabec179aadd301563cd1f7e7b0c73a146251824ef0a40f4e9c7731c75c2bb` (updated 2026-11-07, Version 19: worktree rules by what can be lost, squash-merge check, Worktrees view)
- `freeboard-prototype.html` SHA-256: `e4e14a41ea9d80541a355c731f1c77e49c00c1563aeff2c49dc779ff680d6028`
