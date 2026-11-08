// Automated accessibility checks (task 5.2) with axe-core on every screen and
// state the interface has, in light and dark, desktop and 390 px widths.
// Also checks keyboard reachability of every interactive control.
//
// Usage: npm run verify:a11y  (writes visual/report/a11y.json; exits 1 on violations)
import { AxeBuilder } from "@axe-core/playwright";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { ROOT, launch, startServer } from "../visual/lib.mjs";

// Views and states that exist so far; each milestone adds its own (task 6.6 runs all).
const STATES = [
  { id: "home-low", query: "" },
  { id: "home-healthy", query: "free=40" },
  { id: "home-critical", query: "free=4" },
  { id: "home-pile-open", query: "", click: ["#pile-btn"] },
  { id: "settings-sheet", query: "", click: ['[data-act="settings"]'] },
  { id: "map-default", query: "", click: ["#tab-map"] },
  { id: "map-not-explained", query: "", click: ["#tab-map", '[data-tile="unknown"]'] },
  { id: "map-spots-resolved", query: "", click: ["#tab-map", '[data-spot="archive"]', '[data-spot="mystery"]'] },
  { id: "map-places-open", query: "", click: ["#tab-map", "#places-btn"], wait: ".pl-sum" },
  { id: "map-report", query: "", click: ["#tab-map", '[data-act="report"]'] },
  { id: "tray-view", query: "view=tray", root: ".tray-window" },
  // Slice 2: cleanup sheets and the Logbook.
  { id: "cleanup-confirm", query: "", click: ["#cta"], wait: "#go" },
  { id: "cleanup-confirm-ack", query: "", click: ['details[data-open="grp-Docker"] > summary', "#l-x-d-vol", '[data-now="d-vol"]'], wait: "#sheet-ack-d-vol" },
  { id: "rows-open", query: "", click: ['details[data-open="grp-Projects"] > summary', "#l-x-b-node", "#l-x-wt-pay"] },
  { id: "risk-box-open", query: "", click: ['details[data-open="grp-Docker"] > summary', "#l-x-d-vol", '[data-risk-box="d-vol"] > summary'] },
  { id: "cleanup-progress", query: "hold=1", click: ["#cta", "#go"], wait: "#run-bar" },
  { id: "cleanup-result", query: "skip=wt-exp&trash=1&fail=d-cache", click: ["#cta", "#go"], wait: '#go[data-act="close"]' },
  { id: "trash-confirm", query: "trash=2.4", click: ['[data-act="empty"]'], wait: "#go" },
  { id: "logbook", query: "", click: ["#tab-log"], wait: ".log" },
  { id: "logbook-run-details", query: "", click: ["#tab-log", '[data-run="run-2"]'], wait: "#sheet .plan-list" },
  // Worktrees (prototype Version 19): the tab, an opened row, the dock.
  { id: "worktrees", query: "", click: ["#tab-wt"] },
  { id: "worktrees-open", query: "", click: ["#tab-wt", '[data-wt-open="w1"]', '[data-wt-open="w3"]'] },
  { id: "worktrees-selected", query: "", click: ["#tab-wt", '[data-wt-selrepo="sheltersync"]'] },
  // Languages: longer German and French text, and the language controls.
  { id: "home-low-de", query: "lang=de" },
  { id: "settings-sheet-fr", query: "lang=fr", click: ['[data-act="settings"]'] },
  { id: "tray-view-de", query: "view=tray&lang=de", root: ".tray-window" },
  { id: "first-run-hello-fr", query: "firstrun=1&lang=fr", root: "#ob-root" },
  { id: "first-run-promises-de", query: "firstrun=1&lang=de", click: ["#ob-next", "#ob-next", "#ob-next"], root: "#ob-root" },
  // Task 6.6: every view and sheet added since, and the empty and error states.
  { id: "questions", query: "", click: ["#tab-tidy"] },
  { id: "questions-parts-open", query: "", click: ["#tab-tidy", 'details[data-open="grp-AI-models"] > summary', "#l-x-ai-ollama", ".irow:has(#l-x-ai-ollama) details.parts > summary"] },
  { id: "questions-finished", query: "", click: ["#tab-tidy", ...Array(19).fill('[data-q="later"]')] },
  { id: "keep-tidy-ask", query: "", click: ["#tab-tidy", "#auto-caches"], wait: ".tidy-ask" },
  { id: "settings-background", query: "", click: ['[data-act="settings"]'], wait: "#st-bg" },
  { id: "state-unchecked", query: "unavail=1", click: ["#tab-tidy"] },
  { id: "state-new-install", query: "fresh=1", click: ["#tab-tidy"] },
  { id: "state-nothing-found", query: "nothing=1", click: ["#tab-tidy"] },
  { id: "state-logbook-empty", query: "fresh=1", click: ["#tab-log"] },
  { id: "state-scanning", query: "scanning=1" },
  { id: "state-windows", query: "platform=windows&trash=2.4" },
  { id: "first-run-folders", query: "firstrun=1", click: ["#ob-next", "#ob-next"], root: "#ob-root" },
];
const THEMES = ["light", "dark"];
const SIZES = [
  [1120, 900],
  [390, 844],
];

const { server, base } = await startServer();
const browser = await launch();
const results = [];
let violations = 0;
for (const st of STATES)
  for (const theme of THEMES)
    for (const [w, h] of SIZES) {
      const ctx = await browser.newContext({ viewport: { width: w, height: h }, colorScheme: theme, reducedMotion: "reduce" });
      const page = await ctx.newPage();
      await page.goto(`${base}/?theme=${theme}&${st.query}`);
      const root = st.root ?? "body";
      await page.waitForSelector(".page .hero, .tray-window, #ob-root");
      for (const sel of st.click ?? []) await page.click(sel);
      if (st.wait) await page.waitForSelector(st.wait);
      await page.waitForFunction(() => document.documentElement.dataset.busy === "0");
      await page.waitForTimeout(50);
      const r = await new AxeBuilder({ page }).include(root).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"]).analyze();
      // Keyboard: every enabled control must be reachable with Tab.
      const unreachable = await page.evaluate(async (root) => {
        const ctrls = [...document.querySelector(root).querySelectorAll("button:not([disabled]), input:not([disabled]), select, summary, a[href]")].filter(
          (e) => e.offsetParent !== null,
        );
        return ctrls.filter((e) => e.tabIndex < 0).map((e) => e.outerHTML.slice(0, 80));
      }, root);
      const key = `${st.id}--${theme}--${w}x${h}`;
      violations += r.violations.length + unreachable.length;
      results.push({
        key,
        violations: r.violations.map((v) => ({ id: v.id, impact: v.impact, help: v.help, nodes: v.nodes.map((n) => n.target.join(" ")).slice(0, 5) })),
        unreachable,
        passes: r.passes.length,
      });
      console.log(`${r.violations.length + unreachable.length ? "FAIL" : "pass"}  ${key}${r.violations.map((v) => "  " + v.id).join("")}`);
      await ctx.close();
    }
await browser.close();
server.close();
mkdirSync(join(ROOT, "visual/report"), { recursive: true });
writeFileSync(join(ROOT, "visual/report/a11y.json"), JSON.stringify({ engine: "axe-core 4.14.0", results }, null, 2));
console.log(violations ? `\nAccessibility: FAIL (${violations} issues)` : `\nAccessibility: PASS (${results.length} checks)`);
process.exit(violations ? 1 : 0);
