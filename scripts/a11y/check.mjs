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
  { id: "tray-view", query: "view=tray", root: ".tray-window" },
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
      await page.waitForSelector(".page .hero, .tray-window");
      for (const sel of st.click ?? []) await page.click(sel);
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
