import { readFileSync } from "node:fs";

/** The interface strings per language, for reference edits in that language. */
const STR = Object.fromEntries(["en", "nl", "de", "fr", "es"].map((l) => [l, JSON.parse(readFileSync(new URL(`../../src/i18n/${l}.json`, import.meta.url), "utf8"))]));

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
    // A name that only fits by breaking inside a word ("Proje / cts") is
    // hidden too (DiskMapView, found on a real Mac).
    const name = t.querySelector("b");
    let broken = false;
    if (name) {
      name.style.overflowWrap = "normal";
      broken = name.scrollWidth > name.clientWidth + 1;
      name.style.overflowWrap = "";
    }
    if (clipped || broken) t.classList.add("tiny-fit");
  });
}
const NO_REMOVE =
  "A tile label the prototype would cut off is hidden, as the spec requires (`.tile.tiny-fit`).";

function tossNoteContrast() {
  // The Toss button's small line at full opacity (supplemental.css
  // `.tnote`): the prototype's 85% fails WCAG AA contrast in the light theme.
  for (const e of document.querySelectorAll("#cta .tlabel small")) e.classList.add("tnote");
}
const TNOTE = "The small line in the Toss button is fully opaque (`.tnote`): at the prototype's 85% it fails WCAG AA contrast in the light theme.";

/** Disk map entries compare the tab bar (with its coverage badge) and the map. */
const MAP = [".tabs", "#v-map"];
function untracedCause() {
  // The prototype's 15.1 GB of system data has a Measure button that the app
  // cannot offer honestly, so the app shows that part as "Not traced to a
  // folder" with its reason and no fix (decision 2026-11-07).
  const title = "Not traced to a folder";
  const why = "Space in places I do not walk, such as app data in Library and system folders. I can\u2019t say which folder holds it.";
  for (const c of document.querySelectorAll("#why .cause")) {
    if (!c.querySelector('.sw[style*="--t-h"]')) continue;
    c.querySelector(".cause-head b").textContent = title;
    const p = c.querySelector(".cause-head p");
    if (p) p.textContent = why;
    c.querySelector(".fix-list")?.remove();
    c.querySelector(".fixed-list")?.remove();
  }
  for (const r of document.querySelectorAll("#tile-detail .detail-row > span:first-child")) if (r.textContent === "Kept by macOS for itself") r.textContent = title;
  // Not traced has no fix, so the card no longer promises one for each part
  // (decision 2026-11-07, found on a real Mac).
  if (document.querySelector('#why .cause .sw[style*="--t-h"]')) {
    const fixable = !!document.querySelector("#why .fix-list .fix");
    const lede = document.querySelector("#why .lede");
    if (lede) lede.textContent = "None of it is lost. I just can\u2019t see inside it yet." + (fixable ? " The parts with a button below have a fix." : "");
    const foot = document.querySelector("#why .why-foot");
    if (foot) foot.textContent = (fixable ? "Fixing them brings the map closer to 100%. " : "") + "Nothing here is removed, it only gets measured.";
  }
}
const UNTRACED =
  "The prototype's system-data cause has a Measure button the app cannot offer honestly; the app shows that space as \"Not traced to a folder\" with its reason and no fix. Because that part has no fix, the card's lede and footer no longer promise one for each part. A tile label the prototype would cut off is hidden (`.tile.tiny-fit`).";
const mapEntry = (id, state, steps) => ({ id, surface: "Disk map", state, sizes: [DESKTOP, MID, PHONE], query: "", ref: [tab("map"), ...steps, evalRef(hideClippedLabels), evalRef(untracedCause)], impl: [tab("map"), ...steps], clip: MAP, supplemental: true, note: UNTRACED });
const TILES = ["projects", "docker", "caches", "files", "mobile", "apps", "system", "unknown", "browser", "fonts"];
const PLACES = { click: "#places-btn" };

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
  for (const e of document.querySelectorAll(".cf-lines dd.mono")) if (prose.some((p) => e.textContent.startsWith(p))) e.classList.remove("mono");
}
const MONO = "Plan rows use the code font only for commands (decision 2026-11-05); prose action lines lose the prototype's mono class.";

function leaveOutLinks() {
  // Each item of a several-item confirmation can be left out (decision
  // 2026-11-07), so one item needing its own acknowledgment never blocks the
  // rest. The prototype has no such link; it is added after each item's
  // content with the prototype's .link.small button.
  const items = document.querySelectorAll(".cf-item");
  if (items.length < 2) return;
  for (const e of items) {
    const b = document.createElement("button");
    b.className = "link small cf-leave";
    b.textContent = "Leave it out";
    e.append(b);
  }
}
const LEAVE = "Each item has a Leave it out link (decision 2026-11-07); the prototype has none.";


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
function biggestGroupFirst() {
  // "Everything I found" lists the biggest group first (decided by the user
  // 2026-11-07); the prototype keeps its fixed order.
  const gb = (d) => parseFloat(d.querySelector("summary .num").textContent);
  const groups = [...document.querySelectorAll("#v-tidy details.grp")];
  if (!groups.length) return;
  const parent = groups[0].parentNode, after = groups[groups.length - 1].nextSibling;
  groups.sort((a, b) => gb(b) - gb(a)).forEach((g) => parent.insertBefore(g, after));
}
const SCALE = "The risk scale's other levels keep readable text (supplemental.css): the prototype's 50% opacity fails WCAG AA contrast.";
const BIGGEST = "Groups in Everything I found are ordered biggest first (decided by the user); the reference is reordered the same way.";

// ---- first run (prototype Version 17) -----------------------------------------

function stillFlag() {
  // The flag on step 1 waves with SVG animation, which no timer controls:
  // stop it at its first frame on both sides.
  document.querySelectorAll("#ob-root svg").forEach((s) => {
    s.pauseAnimations?.();
    s.setCurrentTime?.(0);
  });
}
function setupAlone() {
  // The app shows first run on its own (nothing is set up behind it yet);
  // the prototype opens it over its demo page.
  document.querySelector(".page").style.display = "none";
  document.querySelectorAll("#sheet-root, .tray-window").forEach((e) => (e.style.visibility = "hidden"));
  // The app's setup sits inside its root element; the same nesting keeps
  // the corners' compositing identical.
  const ob = document.getElementById("ob-root");
  if (ob && ob.parentElement === document.body) {
    const focused = document.activeElement;
    const root = document.createElement("div");
    root.id = "root";
    document.body.appendChild(root);
    root.appendChild(ob);
    if (focused && focused !== document.body) focused.focus();
  }
}
function focusNext() {
  // The prototype re-renders the whole step on a preset click, which drops
  // focus; the app keeps it on Next.
  document.getElementById("ob-next")?.focus();
}
function noDownloadsCard() {
  // The app offers only folders that hold Git repositories; Downloads (which
  // macOS guards with a permission prompt) is chosen in Settings instead.
  const cards = [...document.querySelectorAll("#ob-root .ob-folder")];
  cards.filter((c) => c.textContent.includes("~/Downloads")).forEach((c) => c.remove());
}
const NEXT = { click: "#ob-next" };
const FIRST_RUN_NOTE = "The app shows first run on its own, so the prototype's demo page behind it is hidden. Folders step: the app offers folders that hold Git repositories, not Downloads, so the reference's Downloads card is removed.";
const firstRunEntry = (id, state, steps, extra = {}) => ({
  // Desktop only: at phone width the dialog scrolls past the screen, and the
  // prototype's quirks-mode layout cuts it at a different height.
  id, surface: "First run", state, sizes: [DESKTOP], query: "firstrun=1",
  ref: [act("welcome"), ...steps, evalRef(stillFlag), evalRef(setupAlone), ...(extra.ref ?? [])], impl: steps, clip: [".ob"], supplemental: true, note: FIRST_RUN_NOTE,
});

// ---- settings ------------------------------------------------------------------

function settingsWording(w = { repos: "$1 repositories,", rerun: "Run setup again", deep: "Every file in your folders and the disk map, a few minutes" }) {
  // Folder rows count Git repositories, which is what the scan finds; the
  // prototype calls the first row's "projects". A deep scan also measures the
  // disk map, which takes a few minutes on a full disk. `w` holds these in
  // the language under test (the prototype keeps its example folder rows in
  // English; the app writes them in the language in use).
  for (const e of document.querySelectorAll("#sheet .frow small")) e.textContent = e.textContent.replace(/^(\d+) (projects|repositories),/, w.repos).replace(/ GB$/, " " + (w.unit ?? "GB"));
  // Spec "First-run setup": Settings offers Run setup again.
  const foot = document.querySelector("#sheet .st-foot");
  if (foot && !document.getElementById("rerun-setup")) {
    const b = document.createElement("button");
    b.className = "link small";
    b.id = "rerun-setup";
    b.style.marginRight = "auto";
    b.textContent = w.rerun;
    foot.prepend(b);
  }
  const deep = document.querySelector("#scan-deep small");
  if (deep) deep.textContent = w.deep;
}
const SETTINGS_NOTE =
  "Folder rows say repositories, not projects, and the deep scan line says it measures the disk map and takes a few minutes. Folders are added with the native folder picker, so the prototype's typed-path form is not used.";
const settingsEntry = (id, state, steps, lang) => ({
  id, surface: "Settings", state, sizes: [DESKTOP, PHONE], query: "",
  ref: [act("settings"), ...steps, evalRef(settingsWording, lang && { repos: STR[lang]["ob.m.reposN"].replace("{n}", "$1") + ",", rerun: STR[lang]["s.rerun"], deep: STR[lang]["s.deep.s2"], unit: lang === "fr" ? "Go" : "GB" })], impl: [act("settings"), ...steps], clip: SHEET, supplemental: true, note: SETTINGS_NOTE,
});

// Reference edits run in the page, so they get openSheet as source text.
const OPEN_SHEET = openSheet.toString();

/** @type {Array<{id:string,surface:string,state:string,sizes:number[][],query?:string,ref:any[],impl:any[],supplemental?:boolean,note?:string,motion?:boolean,themes?:string[]}>} */
export const ENTRIES = [
  // Milestone 2: home at the three disk states, and the tray view (task 2.6).
  { id: "home-healthy", surface: "Home", state: "Healthy, 40 GB free", sizes: [DESKTOP, MID, PHONE], query: "free=40", ref: [free(40), evalRef(tossNoteContrast)], impl: [], clip: HOME, supplemental: true, note: TNOTE },
  { id: "home-low", surface: "Home", state: "Low, 12.4 GB free (the prototype's starting state)", sizes: [DESKTOP, MID, PHONE], query: "", ref: [evalRef(tossNoteContrast)], impl: [], clip: HOME, supplemental: true, note: TNOTE },
  { id: "home-critical", surface: "Home", state: "Critical, 4 GB free", sizes: [DESKTOP, MID, PHONE], query: "free=4", ref: [free(4), evalRef(tossNoteContrast)], impl: [], clip: HOME, supplemental: true, note: TNOTE },
  // Slice 3: the Disk map tab (task 5.12).
  mapEntry("map-default", "Disk map, AI models selected (the prototype's default)", []),
  ...TILES.map((t) => mapEntry(`map-tile-${t}`, `Disk map, ${t === "unknown" ? "Not explained" : t} selected`, [tile(t)])),
  mapEntry("map-spot-archive", "Why card: ~/archive scanned, one fixed", [spot("archive")]),
  mapEntry("map-spot-comfy", "Why card: ~/ComfyUI-Installs scanned; its checkpoints appear under AI models", [spot("comfy"), tile("ai")]),
  mapEntry("map-spot-access", "Why card: access granted, then scanned (the app waits for a scan that can read it)", [spot("access")]),
  mapEntry("map-spot-mystery", "Why card: files marked as mine", [spot("mystery")]),
  mapEntry("map-spot-skip", "Why card: ~/vms skipped (counted as your files)", [{ click: "#mine-vms" }]),
  mapEntry("map-why-more", "Why card: Show 4 more folders", [{ click: '[data-why-more="outside"]' }]),
  mapEntry("map-scan-all", "Why card: Scan all 8, the fixed list opened", [{ click: "#all-outside" }, { click: "#why .fixed-list > summary" }]),
  mapEntry("map-spots-all", "Every fixable blind spot resolved, Your files selected", [{ click: "#all-outside" }, spot("access"), spot("mystery"), tile("files")]),
  {
    id: "map-places", surface: "Disk map", state: "Places checked: the See all sheet", sizes: [DESKTOP, PHONE], query: "",
    ...both([tab("map"), PLACES]), clip: SHEET,
  },
  mapEntry("map-place-go", "Places checked: a found item opened on the map, already expanded", [PLACES, { click: '[data-place="d-vol"]' }]),
  {
    id: "risk-question", surface: "Quick questions", state: "What \u201cReview\u201d means here, opened on the question card", sizes: [DESKTOP, PHONE], query: "",
    ...both([{ click: "#v-tidy .q .riskx > summary" }]), clip: ["#v-tidy .q"], supplemental: true, note: SCALE,
  },
  {
    id: "risk-row-risky", surface: "Quick questions", state: "A Risky volume opened, its risk box open with reasons and commands", sizes: [DESKTOP, PHONE], query: "",
    ref: [{ click: 'details[data-open="grp-Docker"] > summary' }, { click: "#l-x-d-vol" }, { click: ".irow:has(#l-x-d-vol) .riskx > summary" }, evalRef(biggestGroupFirst)], impl: [{ click: 'details[data-open="grp-Docker"] > summary' }, { click: "#l-x-d-vol" }, { click: ".irow:has(#l-x-d-vol) .riskx > summary" }], clip: [".irow:has(#l-x-d-vol)"], supplemental: true, note: BIGGEST + " " + SCALE,
  },
  {
    id: "risk-row-blocked", surface: "Quick questions", state: "A protected worktree opened, its risk box open", sizes: [DESKTOP, PHONE], query: "",
    ref: [{ click: 'details[data-open="grp-Projects"] > summary' }, { click: "#l-x-wt-pay" }, { click: ".irow:has(#l-x-wt-pay) .riskx > summary" }, evalRef(biggestGroupFirst)], impl: [{ click: 'details[data-open="grp-Projects"] > summary' }, { click: "#l-x-wt-pay" }, { click: ".irow:has(#l-x-wt-pay) .riskx > summary" }], clip: [".irow:has(#l-x-wt-pay)"], supplemental: true, note: BIGGEST + " " + SCALE,
  },
  mapEntry("map-report", "Disk report shown in the copy-by-hand box", [act("report")]),
  {
    id: "tray-view", surface: "Tray", state: "Menu bar window, Low", sizes: [[362, 560]], query: "view=tray",
    ref: [{ click: "#tray-btn" }, evalRef(trayWindow)], impl: [], clip: [".tray-window"], supplemental: true,
    note: "The prototype's in-page popover, moved into a .tray-window box (supplemental.css) because the app shows it in its own window opened from the native tray icon.",
  },

  // First run, five steps (prototype Version 17).
  firstRunEntry("first-run-hello", "First run 1: Hello, with the disk and three promises", []),
  firstRunEntry("first-run-waterline", "First run 2: Waterline, Balanced chosen", [NEXT]),
  firstRunEntry("first-run-roomy", "First run 2: Waterline, Roomy chosen (Dinghy reacts)", [NEXT, { click: '[data-ob-preset="roomy"]' }], { ref: [evalRef(focusNext)] }),
  firstRunEntry("first-run-folders", "First run 3: Folders with repositories and the tools found", [NEXT, NEXT], { ref: [evalRef(noDownloadsCard)] }),
  firstRunEntry("first-run-promises", "First run 4: what I read, what I never do, and the switches", [NEXT, NEXT, NEXT]),

  // Settings (prototype revision of 2026-11-07).
  settingsEntry("settings-sheet", "Settings: warning zones, folders, always protected, scanning", []),
  settingsEntry("settings-undo", "Settings: a scanned folder removed, with Undo", [{ click: "#scan-rm-1" }]),
  settingsEntry("settings-suggested-added", "Settings: a folder found outside the scanned ones added", [{ click: "#sugg-add-0" }]),

  // Slice 2: cleanup sheets and the Logbook (task 4.7). Sheets at desktop and phone widths.
  {
    id: "cleanup-confirm-ack", surface: "Cleanup", state: "Remove now on a Docker volume, acknowledgment ticked", sizes: [DESKTOP, PHONE], query: "",
    ...both([{ click: "#l-x-d-vol" }, removeNow("d-vol"), sheetAck("d-vol")]), clip: SHEET,
  },
  {
    id: "cleanup-confirm-pile", surface: "Cleanup", state: "Toss: confirmation for the pile (4 crates)", sizes: [DESKTOP, PHONE], query: "",
    ref: [act("confirm"), evalRef(monoOnlyForCommands), evalRef(leaveOutLinks)], impl: [act("confirm")], clip: SHEET, supplemental: true, note: MONO + " " + LEAVE,
  },
  {
    id: "cleanup-confirm-long", surface: "Cleanup", state: "Toss with All I can: 11 crates, the list scrolls and the button stays visible", sizes: [DESKTOP, PHONE], query: "",
    ref: [goal("all"), act("confirm"), evalRef(monoOnlyForCommands), evalRef(leaveOutLinks)], impl: [goal("all"), act("confirm")], clip: SHEET, supplemental: true, note: MONO + " " + LEAVE,
  },
  {
    id: "cleanup-progress", surface: "Cleanup", state: "Tossing crates: the 3D deck, first of 4 tossed, with Stop after this one", sizes: [DESKTOP, PHONE], query: "hold=1",
    ref: [act("confirm"), act("run"), { clockRun: 60 }], impl: [act("confirm"), act("run")], clip: SHEET,
  },
  {
    id: "cleanup-result", surface: "Cleanup", state: "Done, mostly: the Docker build cache could not go (the prototype stages it), with Try 1 again", sizes: [DESKTOP, PHONE], query: "fail=d-cache",
    ...both([act("confirm"), act("run"), finish]), clip: SHEET,
  },
  {
    id: "cleanup-result-open", surface: "Cleanup", state: "Result with Tossed opened", sizes: [DESKTOP], query: "fail=d-cache",
    ...both([act("confirm"), act("run"), finish, { click: "details.res.safe > summary" }]), clip: SHEET,
  },
  {
    id: "cleanup-retry", surface: "Cleanup", state: "Try 1 again: the confirmation for what could not go", sizes: [DESKTOP, PHONE], query: "fail=d-cache",
    ...both([act("confirm"), act("run"), finish, act("retry")]), clip: SHEET,
  },
  {
    id: "cleanup-result-skipped", surface: "Cleanup", state: "Result with a worktree that stayed on board, an item held in the Trash and one that could not go", sizes: [DESKTOP, PHONE], query: "skip=wt-exp&fail=d-cache",
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

// Worktrees (prototype Version 19): its own tab.
const WT_CLIP = [".tabs", "#v-wt"];
const WT_TAB = { click: "#tab-wt" };
function noAckForWork() {
  // Worktrees with uncommitted files stay unremovable in the app: removing
  // them needs `git worktree remove --force`, which invariant 7 bans (decision
  // 2026-11-07). The prototype's in-row acknowledgment is not offered.
  document.querySelectorAll("#v-wt [data-wt-ack]").forEach((i) => i.closest("label")?.remove());
}
function noGitHubCheck() {
  // Freeboard does not ask GitHub whether a pull request merged (invariant
  // 12, no network); those branches are found as squash merges instead, and
  // the squash check runs `git diff`, not `git merge-tree --write-tree`,
  // which writes to the repository (decision 2026-11-07).
  const walk = document.createTreeWalker(document.getElementById("v-wt"), NodeFilter.SHOW_TEXT);
  for (let n = walk.nextNode(); n; n = walk.nextNode()) {
    n.textContent = n.textContent
      .replace(/^Idle (\d+) days, pull request #\d+ merged\.$/, "Idle $1 days, already in main (squash-merged).")
      .replace(/^Pull request #\d+ merged, so the branch can go too$/, "Changes already in main (squash merge, checked with git diff), so the branch can go too")
      .replace("(squash merge, checked with git merge-tree)", "(squash merge, checked with git diff)");
  }
}
function noRefocus() {
  // The prototype re-renders the list when a row opens and focuses the new
  // button, which shows its focus ring after a mouse click; the app keeps the
  // same button, so a mouse click shows no ring (as everywhere else).
  if (document.activeElement?.closest?.("#v-wt")) document.activeElement.blur();
}
const WT_NOTE = "Merged pull requests read as squash merges (no GitHub check, invariant 12), the squash check names git diff, and rows with uncommitted files have no acknowledgment to remove them (invariant 7).";
const wtEntry = (id, state, steps, sizes = [DESKTOP, MID, PHONE], refOnly = []) => ({
  id, surface: "Worktrees", state, sizes, query: "",
  ref: [WT_TAB, ...steps, evalRef(noAckForWork), evalRef(noGitHubCheck), ...refOnly], impl: [WT_TAB, ...steps], clip: WT_CLIP, supplemental: true, note: WT_NOTE,
});
ENTRIES.push(
  wtEntry("wt-default", "Worktrees: 20 in 6 repositories, 15 ready, by repository", []),
  wtEntry("wt-filter-ready", "Worktrees: the Ready tile filters the list", [{ click: '[data-wt-filter="ready"]' }], [DESKTOP, PHONE]),
  wtEntry("wt-filter-agents", "Worktrees: Made by agents", [{ click: '[data-wt-filter="agent"]' }], [DESKTOP]),
  wtEntry("wt-open-ready", "Worktrees: a squash-merged agent worktree opened (checks, after removal, commands)", [{ click: '[data-wt-open="w1"]' }], [DESKTOP, PHONE], [evalRef(noRefocus)]),
  wtEntry("wt-open-kept", "Worktrees: an unmerged one opened (the branch stays, how to reopen it)", [{ click: '[data-wt-open="w5"]' }], [DESKTOP], [evalRef(noRefocus)]),
  wtEntry("wt-open-work", "Worktrees: one with uncommitted files opened (what would be lost, how to save it)", [{ click: '[data-wt-open="w3"]' }], [DESKTOP], [evalRef(noRefocus)]),
  wtEntry("wt-selected", "Worktrees: Select 8 ready in sheltersync, with the dock", [{ click: '[data-wt-selrepo="sheltersync"]' }], [DESKTOP, PHONE]),
);

// Languages (prototype Version 18): the page follows the browser's language,
// as the app follows the system's, so each capture runs in that locale.
const LOCALES = { de: "de-DE", fr: "fr-FR", nl: "nl-NL", es: "es-ES" };
const NAMES = { de: "German", fr: "French", nl: "Dutch", es: "Spanish" };
const byId = (id) => ENTRIES.find((e) => e.id === id);
const inLang = (e, lang, sizes = e.sizes) => ({ ...e, id: `${e.id}-${lang}`, state: `${e.state} (${NAMES[lang]})`, locale: LOCALES[lang], sizes });
const LANG_ENTRIES = [
  ...["de", "fr", "nl", "es"].map((l) => inLang(byId("home-low"), l, [DESKTOP, PHONE])),
  inLang(byId("home-critical"), "de", [DESKTOP, PHONE]),
  ...["de", "fr", "nl", "es"].map((l) => inLang(byId("tray-view"), l)),
  ...["de", "fr", "nl", "es"].map((l) => inLang(byId("first-run-hello"), l)),
  ...["de", "fr"].flatMap((l) => ["first-run-waterline", "first-run-folders", "first-run-promises"].map((id) => inLang(byId(id), l))),
  ...["de", "fr"].map((l) => inLang(settingsEntry("settings-sheet", "Settings: Language first, then warning zones, folders, scanning", [], l), l)),
];
ENTRIES.push(...LANG_ENTRIES);

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
