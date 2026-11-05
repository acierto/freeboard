// Visual verification matrix (design.md "Visual verification tool").
//
// Each entry is rendered twice:
//   reference: the ORIGINAL prototype (docs/reference/freeboard-prototype.html)
//              served through the test adapter and driven with `ref` steps;
//   actual:    the implementation (dist-visual, fixture backend) loaded with
//              `query` and driven with `impl` steps.
// Steps shared by both sides drive the same DOM controls (nav buttons, chips,
// checkboxes, details) because the implementation keeps the prototype's
// structure. `supplemental: true` marks states the prototype does not contain;
// their reference is the prototype plus documented DOM edits that reuse its
// own components (see `note`).

const DESKTOP = [1120, 900];
const MID = [760, 900];
const PHONE = [390, 844];

const SAFE_MAC = ["d-cache", "d-dangling", "b-node", "b-target", "c-derived", "c-pnpm", "c-pip", "c-brew"];

// ---- step helpers -------------------------------------------------------
const nav = (v) => ({ click: `nav [data-view="${v}"]` });
const chip = (f) => ({ click: `[data-filter="${f}"]` });
const check = (id, on = true) => ({ check: [`#sel-${id}`, on] });
const open = (id) => ({ click: `#sel-${id} + details > summary` });
const free = (gb) => ({ range: ["#sim-free", String(gb)] });
const uncheckSafe = SAFE_MAC.map((id) => check(id, false));
const confirm = { click: "#confirm-plan" };
const finish = { clockRun: 30000 }; // reference only; implementation waits for the backend
const os = (o) => ({ click: `[data-os="${o}"]` });
const evalRef = (fn, arg) => ({ eval: fn.toString(), arg });

// ---- reference DOM edits for supplemental states ------------------------
function settingsAdditions(win) {
  const fold = win ? ["C:\\Users\\you\\projects", "C:\\Users\\you\\work", "C:\\Users\\you\\Downloads"] : ["~/projects", "~/work", "~/Downloads"];
  const sets = document.querySelectorAll("#v-settings .set");
  const folders = sets[4];
  folders.querySelector("ul").innerHTML = fold
    .map((f) => '<li class="folder-row"><span>' + f + '</span><button class="chip small" aria-label="Remove ' + f + '">Remove</button></li>')
    .join("");
  folders.insertAdjacentHTML("beforeend", '<div class="actions"><button class="btn" data-action="add-scanFolders">Add folder</button></div>');
  document.querySelector("#v-settings .settings").insertAdjacentHTML(
    "beforeend",
    '<div class="set"><label for="deep-freq">Deep scan</label><select id="deep-freq"><option value="on_request">Only when I ask</option><option value="daily">Every day</option></select><div class="actions"><button class="btn" data-action="deep-scan">Run deep scan now</button></div></div>' +
      '<div class="set"><span style="font-weight:500">Excluded folders</span><ul class="mono"><li class="muted">Nothing excluded</li></ul><div class="actions"><button class="btn" data-action="add-exclusions">Add folder</button></div></div>',
  );
}

function addCancel() {
  document.querySelector("#v-plan").insertAdjacentHTML("beforeend", '<div class="actions"><button class="btn" data-action="cancel">Cancel</button></div>');
}

function runEdit(a) {
  const rows = document.querySelectorAll("#v-plan .plan-row");
  document.querySelector("#v-plan h2").textContent = a.title;
  document.querySelector("#v-plan p.num").innerHTML = a.head;
  for (const [i, label, cls, note] of a.rows) {
    const r = rows[i];
    const span = r.querySelector(".result");
    span.textContent = label;
    span.className = cls;
    if (note) r.firstElementChild.insertAdjacentHTML("beforeend", '<div class="sub">' + note + "</div>");
  }
}

function removeForecast() {
  document.getElementById("forecast").remove();
}

function emptyGroups() {
  // Supplemental: empty-results sentence plus the prototype's own .actions /
  // .btn components (Scan now, Run deep scan now), reviewed 2026-11-04.
  document.getElementById("groups").innerHTML =
    '<p class="muted">No suggestions yet. Run a scan to look for space you can reclaim.</p>' +
    '<div class="actions"><button class="btn primary">Scan now</button><button class="btn">Run deep scan now</button></div>';
}

function dockerUnavailable() {
  // Supplemental: one note per distinct detector failure, using the
  // prototype's .group-note style (five Docker detectors share one message).
  // With Docker unavailable the backend returns no Docker items, so the
  // Docker group is removed (its Safe items were unchecked first).
  const g = document.getElementById("groups");
  [...g.querySelectorAll(".group")].find((x) => x.querySelector("h3")?.textContent === "Docker")?.remove();
  g.insertAdjacentHTML("afterbegin", '<div class="notes"><p class="group-note">Docker is installed but not running.</p></div>');
}

function emptyHistory() {
  document.getElementById("history").innerHTML = '<p class="muted">No cleanups yet. Each cleanup you confirm is recorded here.</p>';
}

function historyDetail() {
  const row = (t, cmd, label, cls) =>
    '<div class="plan-row"><div><b>' + t + '</b><div class="sub mono">' + cmd + '</div></div><span class="' + cls + '">' + label + "</span></div>";
  document.getElementById("v-history").innerHTML =
    '<div><h2>Cleanup on Oct 28, 2026</h2><p class="muted num">Freed 21.4 GB, measured on the disk. 1.2 GB sent to the Trash. 0 skipped, 0 failed.</p></div>' +
    "<div>" +
    row("Docker build cache", "docker builder prune", "Cleaned", "result ok") +
    row("node_modules in 9 inactive projects", "Delete the 9 folders", "Cleaned", "result ok") +
    row(".venv in scratch", "Move to Trash", "Moved to Trash", "result ok") +
    "</div>" +
    '<div class="actions"><button class="btn" data-action="history-back">Back to history</button></div>';
}

function planExpired() {
  const actions = document.querySelector("#v-plan > .actions");
  actions.insertAdjacentHTML("beforebegin", '<p class="blocked">This plan expired after 30 minutes. Make a new plan to check the items again.</p>');
  actions.firstElementChild.outerHTML = '<button class="btn primary">Make a new plan</button>';
  document.querySelector("#ack-d-vol").disabled = true;
}

function onboarding() {
  document.querySelectorAll("main > .view").forEach((v) => (v.hidden = true));
  document.querySelectorAll("nav button").forEach((b) => b.removeAttribute("aria-current"));
  document.querySelector("main").insertAdjacentHTML(
    "beforeend",
    '<section class="view" id="v-onboarding"><div><h2>Welcome to Freeboard</h2><p class="muted">Freeboard warns you before your disk fills up and shows what you can remove safely.</p></div>' +
      '<div class="settings">' +
      '<div class="set"><span style="font-weight:500">What Freeboard reads</span><ul><li>Free space on your disks, every 5 minutes</li><li>Reports from Docker, Git, pnpm, pip and other developer tools</li><li>Sizes and dates of files in the folders you choose</li></ul></div>' +
      '<div class="set"><span style="font-weight:500">What it never does</span><ul><li>Remove anything without a plan you confirm</li><li>Send your data anywhere. Everything stays on this computer.</li><li>Ask for system permissions before a feature needs them</li></ul></div>' +
      '<div class="set"><span style="font-weight:500">Folders to scan</span><ul class="mono"><li class="muted">No folders chosen yet</li></ul><div class="actions"><button class="btn" data-action="add-scanFolders">Add folder</button></div></div>' +
      "</div>" +
      '<div class="actions"><button class="btn primary" data-action="onboarding-done" disabled>Start monitoring</button><button class="btn" data-action="onboarding-skip">Skip for now</button></div></section>',
  );
}

function widen([attr, v]) {
  document.getElementById("sim-free").setAttribute(attr, v);
}

// ---- entries --------------------------------------------------------------
const both = (steps) => ({ ref: steps, impl: steps });

/** @type {Array<{id:string,surface:string,state:string,sizes:number[][],query?:string,ref:any[],impl:any[],supplemental?:boolean,note?:string,motion?:boolean,themes?:string[]}>} */
export const ENTRIES = [
  // Overview
  { id: "overview-healthy", surface: "Overview", state: "Healthy, 40 GB free", sizes: [DESKTOP, MID, PHONE], query: "free=40", ref: [free(40)], impl: [] },
  { id: "overview-low", surface: "Overview", state: "Low, 12.4 GB free", sizes: [DESKTOP, [761, 900], MID, PHONE], query: "", ref: [], impl: [] },
  { id: "overview-critical", surface: "Overview", state: "Critical, 4 GB free", sizes: [DESKTOP, MID, PHONE], query: "free=4", ref: [free(4)], impl: [] },
  {
    id: "overview-gauge-0", surface: "Overview", state: "Gauge bound: 0 GB free (deck line)", sizes: [DESKTOP, MID, PHONE], query: "free=0",
    ref: [evalRef(widen, ["min", "0"]), free(0)], impl: [], supplemental: true,
    note: "The simulator slider stops at 1 GB; its min attribute is lowered so the prototype's own code renders 0 GB.",
  },
  { id: "overview-gauge-120", surface: "Overview", state: "Gauge bound: 120 GB free", sizes: [DESKTOP, MID, PHONE], query: "free=120", ref: [free(120)], impl: [] },
  {
    id: "overview-gauge-over", surface: "Overview", state: "Gauge bound: more than 120 GB free (150 GB)", sizes: [DESKTOP, MID, PHONE], query: "free=150",
    ref: [evalRef(widen, ["max", "200"]), free(150)], impl: [], supplemental: true,
    note: "The simulator slider stops at 120 GB; its max attribute is raised so the prototype's own code renders 150 GB.",
  },
  {
    id: "overview-trash", surface: "Overview", state: "Trash held after moving .venv to the Trash", sizes: [DESKTOP, MID, PHONE], query: "",
    ...both([nav("suggestions"), ...uncheckSafe, check("b-venv"), nav("plan"), confirm, finish, nav("overview")]),
  },
  {
    id: "overview-trash-confirm", surface: "Overview", state: "Trash empty confirmation", sizes: [DESKTOP, PHONE], query: "",
    ...both([nav("suggestions"), ...uncheckSafe, check("b-venv"), nav("plan"), confirm, finish, nav("overview"), { click: '[data-action="empty-ask"]' }]),
  },
  {
    id: "overview-no-forecast", surface: "Overview", state: "No forecast (insufficient history)", sizes: [DESKTOP, MID, PHONE], query: "forecast=none",
    ref: [evalRef(removeForecast)], impl: [], supplemental: true, note: "The forecast sentence is removed; everything else is the prototype.",
  },
  { id: "overview-motion", surface: "Overview", state: "Motion allowed (animations frozen at start)", sizes: [DESKTOP], themes: ["light"], motion: true, query: "", ref: [], impl: [] },

  // Suggestions
  { id: "suggestions-all", surface: "Suggestions", state: "All filter, default selection", sizes: [DESKTOP, [761, 900], MID, PHONE], query: "", ...both([nav("suggestions")]) },
  { id: "suggestions-safe", surface: "Suggestions", state: "Safe filter", sizes: [DESKTOP, PHONE], query: "", ...both([nav("suggestions"), chip("Safe")]) },
  { id: "suggestions-review", surface: "Suggestions", state: "Review filter", sizes: [DESKTOP, PHONE], query: "", ...both([nav("suggestions"), chip("Review")]) },
  { id: "suggestions-risky", surface: "Suggestions", state: "Risky filter", sizes: [DESKTOP, PHONE], query: "", ...both([nav("suggestions"), chip("Risky")]) },
  {
    id: "suggestions-expanded", surface: "Suggestions", state: "Expanded details", sizes: [DESKTOP, [481, 900], [480, 900], PHONE], query: "",
    ...both([nav("suggestions"), open("d-cache"), open("b-venv")]),
  },
  {
    id: "suggestions-blocked", surface: "Suggestions", state: "Risky filter, blocked worktree expanded", sizes: [DESKTOP, PHONE], query: "",
    ...both([nav("suggestions"), chip("Risky"), open("wt-pay")]),
  },
  {
    id: "suggestions-selection", surface: "Suggestions", state: "Changed selection summary", sizes: [DESKTOP, PHONE], query: "",
    ...both([nav("suggestions"), check("d-cache", false), check("d-pg"), check("b-venv")]),
  },
  {
    id: "suggestions-empty", surface: "Suggestions", state: "Empty results", sizes: [DESKTOP, PHONE], query: "suggestions=empty",
    ref: [nav("suggestions"), ...uncheckSafe, evalRef(emptyGroups)], impl: [nav("suggestions")], supplemental: true,
    note: "Group list replaced by the empty-results sentence and Scan now / Run deep scan now buttons built from the prototype's .actions and .btn; selection summary is the prototype's own 0-selected state.",
  },
  {
    id: "suggestions-docker-unavailable", surface: "Suggestions", state: "Docker unavailable (one note for five detectors)", sizes: [DESKTOP, PHONE], query: "unavailable=docker",
    ref: [nav("suggestions"), check("d-cache", false), check("d-dangling", false), evalRef(dockerUnavailable)], impl: [nav("suggestions")], supplemental: true,
    note: "No Docker group (Docker items are not returned) and a single .group-note above the groups; the backend reports the same cause for all five Docker detectors and the interface shows it once.",
  },
  { id: "suggestions-windows", surface: "Suggestions", state: "Windows labels and paths", sizes: [DESKTOP, PHONE], query: "platform=windows", ref: [os("win"), nav("suggestions")], impl: [nav("suggestions")] },
  { id: "suggestions-linux", surface: "Suggestions", state: "Linux (no Docker Desktop note)", sizes: [DESKTOP, PHONE], query: "platform=linux", ref: [os("linux"), nav("suggestions")], impl: [nav("suggestions")] },

  // Plan review
  { id: "plan-empty", surface: "Plan review", state: "No selection", sizes: [DESKTOP, PHONE], query: "", ...both([nav("suggestions"), ...uncheckSafe, nav("plan")]) },
  { id: "plan-default", surface: "Plan review", state: "Default Safe selection", sizes: [DESKTOP, PHONE], query: "", ...both([nav("plan")]) },
  {
    id: "plan-mixed", surface: "Plan review", state: "Immediate, Trash and unrecoverable items; acknowledgment unchecked", sizes: [DESKTOP, [761, 900], MID, [481, 900], [480, 900], PHONE], query: "",
    ...both([nav("suggestions"), check("b-venv"), check("d-vol"), nav("plan")]),
  },
  {
    id: "plan-mixed-acked", surface: "Plan review", state: "Acknowledgment checked", sizes: [DESKTOP, PHONE], query: "",
    ...both([nav("suggestions"), check("b-venv"), check("d-vol"), nav("plan"), { check: ["#ack-d-vol", true] }]),
  },
  {
    id: "plan-expired", surface: "Plan review", state: "Expired plan", sizes: [DESKTOP, PHONE], query: "plan=expired",
    ref: [nav("suggestions"), check("b-venv"), check("d-vol"), nav("plan"), evalRef(planExpired)],
    impl: [nav("suggestions"), check("b-venv"), check("d-vol"), nav("plan")], supplemental: true,
    note: "Adds the expiry sentence (.blocked) and replaces Confirm with 'Make a new plan'.",
  },

  // Cleanup run
  {
    id: "run-checking", surface: "Cleanup run", state: "Checking first item", sizes: [DESKTOP, PHONE], query: "run=hold",
    ref: [nav("plan"), confirm, evalRef(addCancel)], impl: [nav("plan"), confirm], supplemental: true,
    note: "Prototype running view plus the Cancel button (.btn in .actions) required by the product spec.",
  },
  { id: "run-success", surface: "Cleanup run", state: "Success with measured completion", sizes: [DESKTOP, PHONE], query: "", ...both([nav("plan"), confirm, finish]) },
  {
    id: "run-skipped", surface: "Cleanup run", state: "Skipped change and Trash item", sizes: [DESKTOP, PHONE], query: "",
    ...both([nav("suggestions"), ...uncheckSafe, check("wt-exp"), check("b-venv"), nav("plan"), confirm, finish]),
  },
  {
    id: "run-trash-confirm", surface: "Cleanup run", state: "Trash confirmation after cleanup", sizes: [DESKTOP, PHONE], query: "",
    ...both([nav("suggestions"), ...uncheckSafe, check("wt-exp"), check("b-venv"), nav("plan"), confirm, finish, { click: '[data-action="empty-ask"]' }]),
  },
  {
    id: "run-failed", surface: "Cleanup run", state: "One item failed", sizes: [DESKTOP, PHONE], query: "fail=b-target",
    ref: [
      nav("plan"), confirm, finish,
      evalRef(runEdit, {
        title: "Cleanup finished",
        head: "Freed <b>61.3 GB</b>. Free space is now <b>73.7 GB</b>. 1 item failed.",
        rows: [[3, "Failed", "result fail", "The command exited with an error. Nothing was removed."]],
      }),
    ],
    impl: [nav("plan"), confirm], supplemental: true,
    note: "Prototype finished view with one row changed to Failed (supplemental .result.fail colour) and totals recomputed.",
  },
  {
    id: "run-canceled", surface: "Cleanup run", state: "Canceled after three items", sizes: [DESKTOP, PHONE], query: "cancelAfter=3",
    ref: [
      nav("plan"), confirm, finish,
      evalRef(runEdit, {
        title: "Cleanup canceled",
        head: "Freed <b>39.7 GB</b>. Free space is now <b>52.1 GB</b>. 5 items did not start.",
        rows: [3, 4, 5, 6, 7].map((i) => [i, "Canceled", "result", null]),
      }),
    ],
    impl: [nav("plan"), confirm], supplemental: true,
    note: "Prototype finished view retitled 'Cleanup canceled'; unstarted rows read Canceled.",
  },

  // History
  { id: "history-populated", surface: "History", state: "Populated", sizes: [DESKTOP, PHONE], query: "", ...both([nav("history")]) },
  {
    id: "history-empty", surface: "History", state: "Empty", sizes: [DESKTOP, PHONE], query: "history=empty",
    ref: [nav("history"), evalRef(emptyHistory)], impl: [nav("history")], supplemental: true, note: "Table replaced by the empty-history sentence.",
  },
  {
    id: "history-detail", surface: "History", state: "Run details", sizes: [DESKTOP, PHONE], query: "",
    ref: [nav("history"), evalRef(historyDetail)], impl: [nav("history"), { click: '[data-run="run-2"]' }], supplemental: true,
    note: "Run details reuse the plan-row and result components.",
  },

  // Settings
  {
    id: "settings-defaults", surface: "Settings", state: "Defaults", sizes: [DESKTOP, [761, 900], MID, PHONE], query: "",
    ref: [nav("settings"), evalRef(settingsAdditions, false)], impl: [nav("settings")], supplemental: true,
    note: "Prototype settings plus folder Remove/Add controls, Deep scan and Excluded folders blocks.",
  },
  {
    id: "settings-changed", surface: "Settings", state: "Changed thresholds (30 / 5 GB)", sizes: [DESKTOP, PHONE], query: "",
    ref: [nav("settings"), { range: ["#warn-gb", "30"] }, { range: ["#crit-gb", "5"] }, evalRef(settingsAdditions, false)],
    impl: [nav("settings"), { range: ["#warn-gb", "30"] }, { range: ["#crit-gb", "5"] }], supplemental: true,
    note: "As settings-defaults, thresholds changed through the sliders.",
  },
  {
    id: "settings-windows", surface: "Settings", state: "Windows labels and paths", sizes: [DESKTOP, PHONE], query: "platform=windows",
    ref: [os("win"), nav("settings"), evalRef(settingsAdditions, true)], impl: [nav("settings")], supplemental: true,
    note: "As settings-defaults with Windows folder paths.",
  },
  {
    id: "overview-thresholds", surface: "Overview", state: "Gauge markers follow changed thresholds", sizes: [DESKTOP], query: "",
    ...both([nav("settings"), { range: ["#warn-gb", "30"] }, { range: ["#crit-gb", "5"] }, nav("overview")]),
  },

  // First run
  {
    id: "onboarding", surface: "First run", state: "Welcome, no folders chosen", sizes: [DESKTOP, PHONE], query: "onboarding=1",
    ref: [evalRef(onboarding)], impl: [], supplemental: true, note: "Built from .view, .settings/.set, lists and buttons of the prototype.",
  },

  // Keyboard focus
  { id: "focus-nav", surface: "Keyboard", state: "Focus ring on the second navigation item", sizes: [DESKTOP], query: "", ...both([{ key: "Tab" }, { key: "Tab" }]) },
];

/** Native surfaces that browser comparison cannot establish. */
export const NATIVE = [
  { id: "tray-macos", surface: "Tray/status", state: "Healthy, Low, Critical; menu; notification", platform: "macOS menu bar" },
  { id: "tray-linux-gnome", surface: "Tray/status", state: "Healthy, Low, Critical; menu; notification", platform: "Linux GNOME" },
  { id: "tray-linux-kde", surface: "Tray/status", state: "Healthy, Low, Critical; menu; notification", platform: "Linux KDE" },
  { id: "tray-windows", surface: "Tray/status", state: "Healthy, Low, Critical; menu; notification", platform: "Windows system tray" },
  { id: "webview-macos", surface: "All screens", state: "Actual Tauri WKWebView rendering", platform: "macOS" },
  { id: "webview-linux", surface: "All screens", state: "Actual Tauri WebKitGTK rendering", platform: "Linux" },
  { id: "webview-windows", surface: "All screens", state: "Actual Tauri WebView2 rendering", platform: "Windows" },
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
