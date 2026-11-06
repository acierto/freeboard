# Visual verification

Freeboard's interface must match the supplied design, `docs/reference/freeboard-prototype.html` (the harbor prototype with Dinghy, replaced 2026-11-06). Functional tests do not prove that. The visual verifier renders the original prototype and the app in the same states and compares their pixels.

## Commands

| Command | What it does |
| --- | --- |
| `npm run verify:styles` | Checks that `src/styles/tokens.css` and `src/styles/harbor.css` are byte-for-byte the prototype's stylesheet. `node scripts/visual/sync-styles.mjs` regenerates them. |
| `npm run verify:visual` | Runs the style check, builds `dist/` and the fixture bundle `dist-visual/`, then compares every matrix capture with its baseline. Exits 1 on any failure, 2 when evidence is incomplete, and 0 when all browser comparisons pass. Add `-- --require-native` to exit 2 while native evidence is missing. It fails while the matrix is empty. |
| `npm run verify:visual -- --only '<regex>'` | Runs only the matching capture keys. The report is marked PARTIAL. |
| `node scripts/visual/verify.mjs --live` | Compares against a fresh render of the prototype instead of the stored baselines. CI uses it, because baselines are specific to the machine that rendered them. |
| `npm run verify:visual:selftest` | Proves the verifier fails when the font, Dinghy's shape, a colour or spacing changes in the app's render of `home-low`, and reports a missing font as INCOMPLETE. |
| `npm run verify:ui` | Behaviour checks against the fixture build: keyboard order and visible focus, sheets (focus trap, Esc, focus return), the tray view's close paths, and the pile responding to the helm. |
| `npm run test:ui` | Unit tests for Dinghy's deck height, mood and crates (Node's built-in test runner). |
| `npm run verify:visual:baseline -- --accept "<rationale>"` | Regenerates the baselines from the prototype, never from the app. It refuses to run in CI, requires a rationale, and appends it to `docs/decisions.md` under "Visual baselines". |

Outputs:

- `visual/baseline/*.png` and `visual/baseline/manifest.json` are committed. The manifest records the prototype SHA-256, engine and version, OS, fonts, viewport, theme, crop, fixture, the supplemental flag and each image hash. The old hull-gauge baselines were deleted on 2026-11-06; new ones are added as each milestone's matrix entries land.
- `visual/actual/`, `visual/diff/` and `visual/reference-live/` are generated on each run.
- `visual/report/report.html`, `report.md` and `result.json` give PASS, FAIL or INCOMPLETE for each row and overall, with links to the reference, actual and diff images.
- `visual/review.json` holds the review disposition for each matrix entry.

## How it works

1. **Reference.** `scripts/visual/lib.mjs` serves the prototype file unchanged on disk, through an adapter that:
   - swaps the Google Fonts link for the bundled Fredoka, Nunito and DM Mono in `src/styles/fonts.css`;
   - adds the artifact host's base reset, `src/styles/reset.css`. The prototype is written for a host page that supplies it; without it, the prototype's `hidden` sections (the Disk map tab and the tray popover) stay visible;
   - sets `data-theme`;
   - hides only the prototype-only controls: **Pretend free space** (`.sim`), **Start over**, and the "example data" note under the brand (spec "Build reference"), plus the in-page **Menu bar** button, which stands in for the native tray icon (decision 2026-11-06). Entries that need free space other than 12.4 GB still drive the hidden slider.

   Steps then drive the prototype's own controls: goal and Where from chips, tabs, map tiles, question buttons, checkboxes, `<details>`, blind-spot actions and the simulated cleanup. Playwright's clock is paused so cleanup states can be held.
2. **Actual.** `dist-visual/` is the app built with `--mode visual`, which installs `src/lib/fixtureBackend.ts`. That backend serves the prototype's example data through the real command contract. Query parameters select fixture states.
3. **Capture.** Both sides use pinned Chromium (Playwright 1.64.1) at device scale 1, with reduced motion, the en-US locale, the UTC timezone and a frozen clock (2026-11-04T12:00Z). Every request that leaves 127.0.0.1 is blocked. The verifier waits until the app's backend calls are idle (`html[data-busy="0"]`), confirms that each required font face loaded, and captures the full page (the prototype has no app frame to crop to). An entry may set `clip` to compare only some elements; home entries compare `.top`, `.hero` and `.tabs` until the section contents arrive.
4. **Compare.** pixelmatch runs with `threshold: 0` and `includeAA: true`, so the default tolerance is zero differing pixels. On each run the verifier re-renders the reference and fails if it no longer matches the stored baseline. It also fails if `dist/` contains fixture code, or if the matrix is empty.
5. **Behaviour checks.** With reduced motion, Dinghy's bob, the waves and the boat's rise must not animate; with motion allowed, all three must.

Chromium runs with `--disable-partial-raster`, so captures do not depend on paint history and no pixel tolerance is needed. Any tolerance would go in `visual/tolerances.json` per entry, with a recorded experiment; there is no blanket allowance.

## Matrix

The matrix lives in `scripts/visual/matrix.mjs`. It is built up milestone by milestone, and the required states are those listed in the `visual-verification` spec of the `rebuild-freeboard` change:

| Milestone | Entries added |
| --- | --- |
| 2 Monitoring and home | Home at Healthy, Low and Critical, at 1120, 760 and 390 px; the tray view (supplemental: the prototype's popover in its own window box). 20 captures, all pixel-identical (task 2.6) |
| 3 Disk understanding | Disk map: tiles, area details, Not explained, blind spots open and resolved, growth, places checked, report (task 3.13) |
| 4 Selection | Goals, Where from, manual mode, empty pile, pile open, question cards, finished state, groups and parts, protected list (task 4.6) |
| 5 Cleanup | Confirmation, long list, progress, result, Trash confirmation, Logbook, run details (task 5.7) |
| 6 Release | Keep me tidy, first run, Settings, scan and empty states, platform labels, full run (tasks 6.2–6.7) |

Each entry is captured in light and dark, at the widths the spec names. **Supplemental entries** cover states the prototype lacks; their reference is the prototype plus documented DOM edits that reuse its own components, and they need product-owner acceptance.

## Native evidence (not automated)

Browser comparisons cannot establish native acceptance. The report lists these rows as INCOMPLETE until evidence is attached under `visual/native/<platform>/` and reviewed in `visual/review.json`:

- Tray rings and tray view on the macOS menu bar, Linux GNOME and KDE, and the Windows system tray, in the Healthy, Low and Critical states, plus a notification and the result of clicking it.
- The real webviews: WKWebView on macOS, WebKitGTK on Linux and WebView2 on Windows.

To capture a webview, run `npm run tauri dev` against the fixture build, size the window to the matrix size, take a screenshot with the platform tool (`screencapture -l <windowid>`, `gnome-screenshot -w`, `spectacle -a`, or Snipping Tool), crop it to the app content, and compare it with the browser baseline by eye, with per-engine tolerances documented.
