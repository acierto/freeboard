# Source references

Replaced on 2026-11-06 with the rebuild sources supplied by the user (`freeboard-prototype.html` and `Freeboard Product and Implementation Specification.md`). The previous versions, which described the hull-gauge interface, remain in git history.

- `FREEBOARD_SPEC.md`: product behavior, safety invariants, detector inventory, disk accounting, pile rules, API contract, milestones and acceptance criteria.
- `freeboard-prototype.html`: visual and interaction reference (Dinghy, the pile, Quick questions, Disk map, Logbook). Its data, cleanup and blind-spot results are simulations.

The spec wins on data, safety and behavior; the prototype wins on layout, flows, labels and tone. Conflicts are recorded in `docs/decisions.md`. The prototype loads Google Fonts (Fredoka, Nunito, DM Mono); production and visual tests must bundle those fonts locally.

Prototype-only parts that must not ship: example data, **Pretend free space**, **Start over**, the forced skip of `old-experiment`, and the simulated **Allow access** result.

## Integrity

- `FREEBOARD_SPEC.md` SHA-256: `98bbfcaca7a0bf7a116bc9a89c27c05739c9566e6ee30d3ff16ff91d9a3458c4`
- `freeboard-prototype.html` SHA-256: `1e0a2102e32695d6f56ec943b46724c7b255deebe87bab81fb72a9b325c26ae9`
