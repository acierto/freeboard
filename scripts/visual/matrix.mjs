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

function noRemoveNow() {
  // Remove now and Remove all here open the confirmation sheet, which arrives
  // with cleanup (slice 2, tasks 4.3–4.4). Until then the app's rows and area
  // actions have Keep, the checkbox, Choose which and Add all to pile only;
  // the reference drops the two buttons too.
  document.querySelectorAll("#v-map [data-now], #v-map [data-clean]").forEach((b) => b.remove());
}

function hideClippedLabels() {
  // Spec "Disk map rules": labels wrap or hide cleanly and never clip. On some
  // short tiles the prototype cuts the size label off at the tile's edge
  // (Not explained at 390 px after Allow access). The app hides a label that
  // does not fit, as on the prototype's small tiles (supplemental.css
  // `.tile.tiny-fit`); the reference applies the same rule.
  document.querySelectorAll("#v-map .tile:not(.tiny)").forEach((t) => {
    const box = t.firstElementChild.getBoundingClientRect();
    const clipped = [...t.firstElementChild.children].some((c) => {
      const r = c.getBoundingClientRect();
      return r.bottom > box.bottom - 2 + r.height / 4 || r.right > box.right - 2 + 1;
    });
    if (clipped) t.classList.add("tiny-fit");
  });
}
const NO_REMOVE =
  "Remove now and Remove all here are removed from the reference until cleanup (slice 2) brings the confirmation sheet. A tile label the prototype would cut off is hidden, as the spec requires (`.tile.tiny-fit`).";

/** Disk map entries compare the tab bar (with its coverage badge) and the map. */
const MAP = [".tabs", "#v-map"];
const mapEntry = (id, state, steps) => ({ id, surface: "Disk map", state, sizes: [DESKTOP, MID, PHONE], query: "", ref: [tab("map"), ...steps, evalRef(noRemoveNow), evalRef(hideClippedLabels)], impl: [tab("map"), ...steps], clip: MAP, supplemental: true, note: NO_REMOVE });
const TILES = ["projects", "docker", "caches", "files", "mobile", "apps", "system", "unknown"];
const PLACES = { click: "#v-map .card > details > summary" };

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

/** @type {Array<{id:string,surface:string,state:string,sizes:number[][],query?:string,ref:any[],impl:any[],supplemental?:boolean,note?:string,motion?:boolean,themes?:string[]}>} */
export const ENTRIES = [
  // Milestone 2: home at the three disk states, and the tray view (task 2.6).
  { id: "home-healthy", surface: "Home", state: "Healthy, 40 GB free", sizes: [DESKTOP, MID, PHONE], query: "free=40", ref: [free(40)], impl: [], clip: HOME },
  { id: "home-low", surface: "Home", state: "Low, 12.4 GB free (the prototype's starting state)", sizes: [DESKTOP, MID, PHONE], query: "", ref: [], impl: [], clip: HOME },
  { id: "home-critical", surface: "Home", state: "Critical, 4 GB free", sizes: [DESKTOP, MID, PHONE], query: "free=4", ref: [free(4)], impl: [], clip: HOME },
  // Slice 3: the Disk map tab (task 5.12).
  mapEntry("map-default", "Disk map, AI models selected (the prototype's default)", []),
  ...TILES.map((t) => mapEntry(`map-tile-${t}`, `Disk map, ${t === "unknown" ? "Not explained" : t} selected`, [tile(t)])),
  mapEntry("map-spot-archive", "Blind spot resolved: folder scanned", [spot("archive")]),
  mapEntry("map-spot-access", "Blind spot resolved: access granted, then scanned (the app waits for a scan that can read it)", [spot("access")]),
  mapEntry("map-spot-mystery", "Blind spot resolved: files marked as mine", [spot("mystery")]),
  mapEntry("map-spots-all", "Every blind spot resolved: Not explained is gone, Your files selected", [spot("archive"), spot("access"), spot("mystery"), tile("files")]),
  mapEntry("map-places", "Places I checked opened", [PLACES]),
  mapEntry("map-report", "Disk report shown in the copy-by-hand box", [act("report")]),
  {
    id: "tray-view", surface: "Tray", state: "Tray view, Low", sizes: [[312, 260]], query: "view=tray",
    ref: [{ click: "#tray-btn" }, evalRef(trayWindow)], impl: [], clip: [".tray-window"], supplemental: true,
    note: "The prototype's in-page popover, moved into a .tray-window box (supplemental.css) because the app shows it in its own window opened from the native tray icon.",
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
