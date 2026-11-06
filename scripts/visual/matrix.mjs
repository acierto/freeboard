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

/** @type {Array<{id:string,surface:string,state:string,sizes:number[][],query?:string,ref:any[],impl:any[],supplemental?:boolean,note?:string,motion?:boolean,themes?:string[]}>} */
export const ENTRIES = [];

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
