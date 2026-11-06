// Visual verification matrix for the rebuild (openspec change
// `rebuild-freeboard`, capability `visual-verification`).
//
// Each entry is rendered twice:
//   reference: the ORIGINAL prototype (docs/reference/freeboard-prototype.html)
//              served through the adapter in lib.mjs and driven with `ref` steps;
//   actual:    the app (dist-visual, fixture backend) loaded with `query` and
//              driven with `impl` steps.
// The app keeps the prototype's DOM structure and data attributes, so most
// steps are shared (`both`). `supplemental: true` marks states the prototype
// does not contain; their reference is the prototype plus documented DOM edits
// that reuse its own components (see `note`).
//
// Entries are added milestone by milestone, as each part of the interface is
// built (tasks 2.6, 3.13, 4.6, 5.7, 6.2–6.5, 6.7). The verifier refuses to pass
// while the matrix is empty.

export const DESKTOP = [1120, 900];
export const MID = [760, 900];
export const PHONE = [390, 844];

// ---- step helpers (prototype controls) ------------------------------------
export const goal = (g) => ({ click: `[data-goal="${g}"]` });
export const area = (a) => ({ click: `[data-area="${a}"]` });
export const tab = (t) => ({ click: `[data-tab="${t}"]` });
export const tile = (t) => ({ click: `[data-tile="${t}"]` });
export const question = (q) => ({ click: `[data-q="${q}"]` });
export const spot = (id) => ({ click: `[data-spot="${id}"]` });
export const act = (a) => ({ click: `[data-act="${a}"]` });
export const pile = (id, on = true) => ({ check: [`#l-pile-${id}`, on] });
export const takeOut = (id) => ({ click: `[data-out="${id}"]` });
export const openGroup = (key) => ({ click: `details[data-open="${key}"] > summary` });
export const ack = (id) => ({ check: [`#ack-${id}`, true] });
export const finish = { clockRun: 30000 }; // reference only; the app waits for its backend
export const both = (steps) => ({ ref: steps, impl: steps });
export const free = (gbFree) => ({ range: ["#sim-free", String(gbFree)] }); // reference only: Pretend free space
export const evalRef = (fn, arg) => ({ eval: fn.toString(), arg });

/** Until the section contents land (milestones 3–5), home entries compare the
 * top bar, the harbor and the helm, and the tab bar. */
const HOME = [".top", ".hero", ".tabs"];

// ---- reference DOM edits for supplemental states ---------------------------

function noCoverageBadge() {
  // The Disk map tab's coverage badge needs the real disk map, which arrives
  // with slice 3 (task 5.12). Until then the app shows no badge rather than a
  // made-up number; the reference drops it too.
  document.querySelector("#tab-map .dot")?.remove();
}
const NO_MAP = "Disk map coverage badge removed until slice 3 (task 5.12) brings the real disk map.";

function trayWindow() {
  // The tray view runs in its own small window (decision 2026-11-06). Move the
  // prototype's popover into a .tray-window box (supplemental.css) and hide the
  // page, so the reference shows the popover as the app's tray window does.
  const pop = document.getElementById("popover");
  const w = document.createElement("div");
  w.className = "tray-window";
  document.querySelector(".page").style.display = "none";
  w.appendChild(pop);
  document.body.prepend(w);
}

// ---- cleanup (slice 2, task 4.7) --------------------------------------------

export const removeNow = (id) => ({ click: `[data-now="${id}"]` });
export const sheetAck = (id) => ({ check: [`#sheet-ack-${id}`, true] });
/** Sheets are compared on their own: the page under the scrim is covered by
 * the home entries. */
const SHEET = [".sheet"];

function monoOnlyForCommands() {
  // Plan rows use the code font only for commands (decision 2026-11-05); the
  // prototype sets it on every action line. Prose actions lose `mono`.
  const prose = ["Delete the folders", "Delete the folder", "Move to Trash", "Delete the caches while the apps are closed", "Delete old versions", "Move extra copies to Trash"];
  for (const e of document.querySelectorAll(".plan-list .sub.mono")) if (prose.some((p) => e.textContent.startsWith(p))) e.classList.remove("mono");
}
const MONO = "Plan rows use the code font only for commands (decision 2026-11-05); prose action lines lose the prototype's mono class.";

function addCancel() {
  // Cancel during cleanup is required behavior the prototype lacks (spec):
  // a .row with a .btn under the progress bar.
  const row = document.createElement("div");
  row.className = "row";
  row.innerHTML = '<button class="btn" data-act="cancel">Cancel</button>';
  document.getElementById("sheet").appendChild(row);
}

function openSheet(html) {
  // The prototype's own openSheet(): a .scrim with a focused .sheet dialog.
  document.getElementById("sheet-root").innerHTML =
    '<div class="scrim" id="scrim"><div class="sheet" role="dialog" aria-modal="true" aria-labelledby="sheet-title" tabindex="-1" id="sheet">' + html + "</div></div>";
  document.getElementById("sheet").focus();
}

function trashSheet(open) {
  // Emptying the Trash needs its own confirmation (spec). Built from the
  // prototype's sheet, .muted, .note, .row and .btn components. Under it, the
  // helm shows the prototype's own Trash line (renderHero) for 2.4 GB held.
  document.getElementById("trash").innerHTML =
    '<div class="row between note"><span class="num">2.4 GB is in the Trash and not free yet.</span><button class="link" data-act="empty">Empty it</button></div>';
  (0, eval)("(" + open + ")")(
    '<h2 id="sheet-title">Empty 2.4 GB from the Trash?</h2>' +
      '<p class="muted">Only the items I moved there are deleted. Everything else in your Trash stays where it is.</p>' +
      '<p class="note">This cannot be undone.</p>' +
      '<div class="row"><button class="btn primary" id="go" data-act="empty-confirm">Empty it</button><button class="btn" data-act="close">Not now</button></div>',
  );
}

function runDetailsSheet(open) {
  // Run details in the Logbook (spec). Built from the prototype's sheet,
  // .sum3, .plan-list and .pile-row components, for the fixture's Oct 28 run.
  const rows = [
    ["Docker build cache", "docker builder prune", true, "Removed", "9.6"],
    ["Xcode DerivedData", "Delete the folder", false, "Removed", "5.1"],
    ["Old Playwright browsers", "Delete old versions", false, "Removed", "2.4"],
    ["pip download cache", "pip cache purge", true, "Removed", "1.8"],
    ["Old Homebrew downloads", "brew cleanup", true, "Removed", "1.5"],
    [".venv in data-tools", "Move to Trash", false, "Moved to the Trash", "1.2"],
    ["Untagged Docker images", "docker image prune", true, "Removed", "1.0"],
  ];
  (0, eval)("(" + open + ")")(
    '<h2 id="sheet-title">Tossed 7 crates</h2><p class="muted">Oct 28, 10:00</p>' +
      '<div class="sum3"><div><b class="num">21.4 GB</b>Freed, measured</div><div><b class="num">1.2 GB</b>Held in the Trash</div><div><b class="num">0</b>Skipped</div></div>' +
      '<p class="note">Docker Desktop may take a while to return its space, so the measured change can be lower than expected.</p>' +
      '<div class="plan-list">' +
      rows
        .map(
          ([t, a, cmd, r, g]) =>
            '<div class="pile-row" style="grid-template-columns:minmax(0,1fr) auto"><span>' + t + '<span class="sub' + (cmd ? " mono" : "") + '">' + a + '</span><span class="sub">' + r + '</span></span><span class="num">' + g + " GB</span></div>",
        )
        .join("") +
      "</div>" +
      '<div class="row"><button class="btn primary" id="go" data-act="close">Done</button></div>',
  );
}
// Reference edits run in the page, so they get openSheet as source text.
const OPEN_SHEET = openSheet.toString();

/** @type {Array<{id:string,surface:string,state:string,sizes:number[][],query?:string,ref:any[],impl:any[],supplemental?:boolean,note?:string,motion?:boolean,themes?:string[]}>} */
export const ENTRIES = [
  // Milestone 2: home at the three disk states, and the tray view (task 2.6).
  { id: "home-healthy", surface: "Home", state: "Healthy, 40 GB free", sizes: [DESKTOP, MID, PHONE], query: "free=40", ref: [free(40), evalRef(noCoverageBadge)], impl: [], clip: HOME, supplemental: true, note: NO_MAP },
  { id: "home-low", surface: "Home", state: "Low, 12.4 GB free (the prototype's starting state)", sizes: [DESKTOP, MID, PHONE], query: "", ref: [evalRef(noCoverageBadge)], impl: [], clip: HOME, supplemental: true, note: NO_MAP },
  { id: "home-critical", surface: "Home", state: "Critical, 4 GB free", sizes: [DESKTOP, MID, PHONE], query: "free=4", ref: [free(4), evalRef(noCoverageBadge)], impl: [], clip: HOME, supplemental: true, note: NO_MAP },
  {
    id: "tray-view", surface: "Tray", state: "Tray view, Low", sizes: [[312, 260]], query: "view=tray",
    ref: [{ click: "#tray-btn" }, evalRef(trayWindow)], impl: [], clip: [".tray-window"], supplemental: true,
    note: "The prototype's in-page popover, moved into a .tray-window box (supplemental.css) because the app shows it in its own window opened from the native tray icon.",
  },

  // Slice 2: cleanup sheets and the Logbook (task 4.7). Sheets at desktop and phone widths.
  {
    id: "cleanup-confirm-ack", surface: "Cleanup", state: "Remove now on a Docker volume, acknowledgment ticked", sizes: [DESKTOP, PHONE], query: "",
    ...both([removeNow("d-vol"), sheetAck("d-vol")]), clip: SHEET,
  },
  {
    id: "cleanup-confirm-pile", surface: "Cleanup", state: "Toss: confirmation for the pile (4 crates)", sizes: [DESKTOP, PHONE], query: "",
    ref: [act("confirm"), evalRef(monoOnlyForCommands)], impl: [act("confirm")], clip: SHEET, supplemental: true, note: MONO,
  },
  {
    id: "cleanup-confirm-long", surface: "Cleanup", state: "Toss with All I can: 11 crates, the list scrolls and the button stays visible", sizes: [DESKTOP, PHONE], query: "",
    ref: [goal("all"), act("confirm"), evalRef(monoOnlyForCommands)], impl: [goal("all"), act("confirm")], clip: SHEET, supplemental: true, note: MONO,
  },
  {
    id: "cleanup-progress", surface: "Cleanup", state: "Tossing crates: first of 4 done, with Cancel", sizes: [DESKTOP, PHONE], query: "hold=1",
    ref: [act("confirm"), act("run"), { clockRun: 60 }, evalRef(addCancel)], impl: [act("confirm"), act("run")], clip: SHEET, supplemental: true,
    note: "The prototype's progress sheet one item in (its clock advanced 60 ms), plus the Cancel button the spec requires (a .row with a .btn).",
  },
  {
    id: "cleanup-result", surface: "Cleanup", state: "Much better! after tossing the pile, with the Docker Desktop note", sizes: [DESKTOP, PHONE], query: "",
    ...both([act("confirm"), act("run"), finish]), clip: SHEET,
  },
  {
    id: "cleanup-result-skipped", surface: "Cleanup", state: "Result with a skipped worktree and an item held in the Trash", sizes: [DESKTOP, PHONE], query: "skip=wt-exp",
    ...both([pile("wt-exp"), pile("b-venv"), act("confirm"), act("run"), finish]), clip: SHEET,
  },
  {
    id: "trash-confirm", surface: "Cleanup", state: "Empty it: confirmation limited to Freeboard's Trash items", sizes: [DESKTOP, PHONE], query: "trash=2.4",
    ref: [evalRef(trashSheet, OPEN_SHEET)], impl: [act("empty")], clip: SHEET, supplemental: true,
    note: "Emptying the Trash needs its own confirmation (spec); built from the prototype's sheet, .muted, .note, .row and .btn components.",
  },
  {
    id: "logbook", surface: "Logbook", state: "Logbook with the prototype's two runs", sizes: [DESKTOP, PHONE], query: "",
    ...both([tab("log")]), clip: ["#v-log"],
  },
  {
    id: "logbook-run-details", surface: "Logbook", state: "Run details for Oct 28: each item with its action and result", sizes: [DESKTOP, PHONE], query: "",
    ref: [tab("log"), evalRef(runDetailsSheet, OPEN_SHEET)], impl: [tab("log"), { click: '[data-run="run-2"]' }], clip: SHEET, supplemental: true,
    note: "Run details are required behavior the prototype lacks (spec); built from its sheet, .sum3, .plan-list and .pile-row components.",
  },
];

/** Native surfaces that browser comparison cannot establish. */
export const NATIVE = [
  { id: "tray-macos", surface: "Tray", state: "Healthy, Low, Critical rings; tray view; menu; notification", platform: "macOS menu bar" },
  { id: "tray-linux-gnome", surface: "Tray", state: "Healthy, Low, Critical rings; tray view; menu; notification", platform: "Linux GNOME" },
  { id: "tray-linux-kde", surface: "Tray", state: "Healthy, Low, Critical rings; tray view; menu; notification", platform: "Linux KDE" },
  { id: "tray-windows", surface: "Tray", state: "Healthy, Low, Critical rings; tray view; menu; notification", platform: "Windows system tray" },
  { id: "webview-macos", surface: "All views", state: "Actual Tauri WKWebView rendering", platform: "macOS" },
  { id: "webview-linux", surface: "All views", state: "Actual Tauri WebKitGTK rendering", platform: "Linux" },
  { id: "webview-windows", surface: "All views", state: "Actual Tauri WebView2 rendering", platform: "Windows" },
];

export const THEMES = ["light", "dark"];

/** Expand entries into individual captures. */
export function captures() {
  const out = [];
  for (const e of ENTRIES)
    for (const theme of e.themes ?? THEMES)
      for (const [w, h] of e.sizes) out.push({ ...e, theme, width: w, height: h, key: `${e.id}--${theme}--${w}x${h}` });
  return out;
}
