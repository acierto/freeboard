// npm run verify:visual:selftest
//
// Proves the verifier detects real design drift (task 1.8): deliberate font,
// gauge-shape, colour and spacing mutations injected into the implementation
// must FAIL against the committed baselines, the unmutated implementation must
// PASS, and a missing font must be reported INCOMPLETE (never a silent pass).
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { BASELINE_DIR, capture, launch, startServer } from "./lib.mjs";
import { comparePng } from "./compare.mjs";
import { captures } from "./matrix.mjs";

const KEY = "overview-low--light--1120x900";
const cap = captures().find((c) => c.key === KEY);
const baseFile = join(BASELINE_DIR, KEY + ".png");
if (!existsSync(baseFile)) {
  console.error(`Missing baseline ${baseFile}; generate baselines first.`);
  process.exit(1);
}
const baseline = readFileSync(baseFile);

const MUTATIONS = {
  none: null,
  font: () => {
    const s = document.createElement("style");
    s.textContent = "h3 { font-family: 'IBM Plex Sans', sans-serif !important; }";
    document.head.append(s);
  },
  gaugeShape: () => {
    // Replace the hull outline with a plain rectangle.
    document.querySelectorAll('#gauge path[fill="var(--hull)"], #hull-clip path').forEach((p) => p.setAttribute("d", "M56 40H402V288H56Z"));
  },
  color: () => {
    document.documentElement.style.setProperty("--accent", "#0B6574");
  },
  spacing: () => {
    const s = document.createElement("style");
    s.textContent = ".view { gap: 19px !important; }";
    document.head.append(s);
  },
};

const { server, base } = await startServer();
const browser = await launch();
const results = [];
for (const [name, mutate] of Object.entries(MUTATIONS)) {
  const r = await capture(browser, base, cap, "impl", { mutate: mutate ?? undefined });
  const cmp = comparePng(baseline, r.png);
  const failed = !!cmp.sizeMismatch || cmp.diffPixels > 0;
  const expectFail = name !== "none";
  results.push({ name, diffPixels: cmp.diffPixels, sizeMismatch: cmp.sizeMismatch, verdict: failed ? "fail" : "pass", ok: failed === expectFail });
}
// Missing font: block the woff2 files; the verifier must report incomplete.
const nf = await capture(browser, base, cap, "impl", { blockFonts: true });
results.push({ name: "missingFont", missingFonts: nf.missingFonts, verdict: nf.missingFonts.length ? "incomplete" : "pass", ok: nf.missingFonts.length > 0 });
await browser.close();
server.close();

for (const r of results)
  console.log(`${r.ok ? "OK  " : "BAD "} ${r.name.padEnd(12)} verifier verdict: ${r.verdict}${r.diffPixels !== undefined ? ` (${r.diffPixels} px${r.sizeMismatch ? ", size " + r.sizeMismatch : ""})` : ""}${r.missingFonts ? " missing: " + r.missingFonts.join(", ") : ""}`);
const ok = results.every((r) => r.ok);
console.log(ok ? "\nSelftest PASSED: the verifier fails on font, gauge shape, colour and spacing changes and on missing fonts." : "\nSelftest FAILED: the verifier missed a deliberate change.");
process.exit(ok ? 0 : 1);
