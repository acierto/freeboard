# Visual verification

Freeboard's interface must match the supplied design, `docs/reference/freeboard-prototype.html`. Functional tests do not prove that. The visual verifier renders the original prototype and the implementation in the same states and compares their pixels.

## Commands

| Command | What it does |
| --- | --- |
| `npm run verify:visual` | Builds `dist/` and the fixture bundle `dist-visual/`, then compares every matrix capture with its baseline. It exits 1 on any failure, 2 when evidence is incomplete, and 0 when all browser comparisons pass. Add `-- --require-native` to exit 2 while native evidence is missing. |
| `npm run verify:visual -- --only '<regex>'` | Runs only the matching capture keys. The report is marked PARTIAL. |
| `node scripts/visual/verify.mjs --live` | Compares against a fresh render of the prototype instead of the stored baselines. Use it while iterating. |
| `npm run verify:visual:selftest` | Proves that the verifier fails when the font, gauge shape, colour or spacing changes, and reports a missing font as INCOMPLETE. |
| `npm run verify:visual:baseline -- --accept "<rationale>"` | Regenerates the baselines from the prototype, never from the app. It refuses to run in CI, requires a rationale, and appends the rationale to `docs/decisions.md` under "Visual baselines". |

Outputs:

- `visual/baseline/*.png` and `visual/baseline/manifest.json` are committed. The manifest records the prototype SHA-256, engine and version, OS, fonts, viewport, theme, crop, fixture, the supplemental flag and each image hash.
- `visual/actual/`, `visual/diff/` and `visual/reference-live/` are generated on each run.
- `visual/report/report.html`, `report.md` and `result.json` give a PASS, FAIL or INCOMPLETE result for each row and overall, with links to the reference, actual and diff images.
- `visual/review.json` holds the human or visual-capable review disposition for each matrix entry. The report shows these dispositions.

## How it works

1. **Reference.** `scripts/visual/lib.mjs` serves the prototype file unchanged on disk, through an adapter. The adapter:
   - swaps the Google Fonts link for the bundled fonts in `src/styles/fonts.css`;
   - sets `data-theme`;
   - hides only the prototype chrome: `.proto`, `.menubar` and `#notice`.

   Steps then drive the prototype's own controls: the simulator slider, navigation, chips, checkboxes, `<details>` and the simulated cleanup. Playwright's clock is paused so that cleanup states can be held.
2. **Actual.** `dist-visual/` is the app built with `--mode visual`, which installs `src/lib/fixtureBackend.ts`. Query parameters select the fixture data, such as `?free=4&theme=dark` or `?run=hold`. Most steps are shared with the reference, because the app keeps the prototype's DOM structure and class names.
3. **Capture.** Both sides use pinned Chromium (Playwright 1.64.1) at device scale 1, with reduced motion, the en-US locale, the UTC timezone and a frozen clock (2026-11-04T12:00Z). Every request that leaves 127.0.0.1 is blocked. The verifier waits until the app's backend calls are idle (`html[data-busy="0"]`), confirms that each required font face actually loaded, and then crops `.app`.
4. **Compare.** pixelmatch runs with `threshold: 0` and `includeAA: true`, so the default tolerance is zero differing pixels. On each run the verifier also re-renders the reference and fails if it no longer matches the stored baseline, so a stale baseline cannot pass. It also fails if `dist/` contains fixture code.
5. **Behaviour checks.** The verifier reads computed styles on both sides. With reduced motion, the gauge waves and the waterline transition must not animate. With motion allowed, both must animate.

### Rendering settings

Chromium runs with `--disable-partial-raster`. Without it, a partial tile re-raster after DOM changes antialiases the window's rounded corners differently. The effect was 7 to 27 pixels with a colour difference of 1, and it depended on the order of clicks rather than on the design. This was measured during development: the same final DOM produced two corner variants on both sides. With full raster, captures are deterministic, and no pixel tolerance is needed.

### Tolerances

None are in use. `visual/tolerances.json`, which does not exist yet, can define `{ "version": 1, "entries": { "<capture key or id>": { "threshold": 0, "maxDiffPixels": N, "rationale": "..." } } }`. Any entry needs a recorded experiment and a review. Never use a blanket allowance.

## Matrix

The matrix lives in `scripts/visual/matrix.mjs`: 40 entries and 191 captures, each in light and dark unless noted.

| Surface | States | Sizes |
| --- | --- | --- |
| Overview | Healthy 40 GB, Low 12.4 GB, Critical 4 GB; gauge bounds at 0, 120 and 150 GB; Trash held; Trash confirmation; no forecast; thresholds changed in Settings; motion allowed (light only) | 1120×900, 760×900, 390×844, plus 761×900 |
| Suggestions | All, Safe, Review and Risky filters; expanded details; blocked worktree; changed selection; empty; Windows; Linux | 1120×900, 390×844, plus 761/760 and 481/480 |
| Plan review | No selection; default; mixed (immediate, Trash and unrecoverable) with the acknowledgment unchecked and checked; expired | 1120×900, 390×844, plus 761/760 and 481/480 |
| Cleanup run | Checking; success with the measured result; skipped change; Trash confirmation; failure; canceled | 1120×900, 390×844 |
| History | Populated, empty, run details | 1120×900, 390×844 |
| Settings | Defaults, changed thresholds, Windows paths | 1120×900, 390×844, plus 761/760 |
| First run | Welcome screen with no folders chosen | 1120×900, 390×844 |
| Keyboard | Focus ring on a navigation item | 1120×900 |

**Supplemental entries** cover states the prototype lacks: errors, cancellation, onboarding, run details, folder controls, the empty and expired states, and the gauge bounds beyond the simulator range. They are flagged in the manifest and the report. Their reference is the prototype plus documented DOM edits that reuse its own components. The only new CSS is `src/styles/supplemental.css`, which adds new class names and never overrides a prototype rule. The verifier injects that file into the reference only for supplemental entries. These designs are additions and need product-owner acceptance.

## Native evidence (not automated)

Browser comparisons cannot establish native acceptance. The report lists these rows as INCOMPLETE until someone attaches evidence:

- **macOS menu bar, Linux GNOME and KDE status area, Windows system tray.** Capture the tray icon and menu in the Healthy, Low and Critical states, plus a notification and the result of clicking it.
- **Tauri webviews.** WKWebView on macOS, WebKitGTK on Linux, and WebView2 on Windows.

To capture a webview:

1. Run `npm run tauri dev` against the fixture build. Point `devUrl` at `vite --mode visual`, or open the window on `?free=12.4`.
2. Set the window content to the matrix size.
3. Take a screenshot with the platform tool:
   - macOS: `screencapture -l <windowid>`.
   - GNOME: `gnome-screenshot -w`. KDE: `spectacle -a`.
   - Windows: Snipping Tool, or `Win+Shift+S`.
4. Crop the screenshot to the app content.
5. Compare it with the browser baseline. Use per-engine references with documented tolerances, because WebKit and Chromium rasterize text differently, and review each image by eye.

Store the evidence under `visual/native/<platform>/` and record the review in `visual/review.json`.
