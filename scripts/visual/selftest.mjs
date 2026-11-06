// npm run verify:visual:selftest
//
// Proves the verifier detects real design drift: deliberate font, Dinghy
// shape, colour and spacing mutations must FAIL the pixel comparison, an
// unmutated render must PASS, and a missing font must be reported INCOMPLETE
// (never a silent pass).
//
// Until the rebuilt home view exists (task 2.6 switches this to the app), the
// mutations are applied to the reference render itself: a fresh capture of the
// prototype is the expected image, and mutated captures of the same state must
// differ from it. This exercises the same capture, adapter, font check and
// comparison code the verifier uses for every matrix entry.
import { capture, launch, startServer } from "./lib.mjs";
import { comparePng } from "./compare.mjs";

const cap = { id: "selftest-home", theme: "light", width: 1120, height: 900, query: "", ref: [], impl: [], supplemental: false };
const SIDE = "ref";

const MUTATIONS = {
  none: null,
  font: () => {
    const s = document.createElement("style");
    s.textContent = "h2, .brand, .big-free { font-family: 'Nunito', sans-serif !important; }";
    document.head.append(s);
  },
  dinghyShape: () => {
    // Replace Dinghy's rounded hull with a plain box.
    document.querySelector('#boat path[fill="var(--hull)"]').setAttribute("d", "M-118 -6H134V66H-118Z");
  },
  color: () => {
    document.documentElement.style.setProperty("--accent", "#0C7787");
  },
  spacing: () => {
    const s = document.createElement("style");
    s.textContent = ".page { gap: 15px !important; }";
    document.head.append(s);
  },
};

const { server, base } = await startServer();
const browser = await launch();
const results = [];
const expected = await capture(browser, base, cap, SIDE);
if (!expected.png) {
  console.error("Reference render failed: " + expected.error);
  process.exit(1);
}
for (const [name, mutate] of Object.entries(MUTATIONS)) {
  const r = await capture(browser, base, cap, SIDE, { mutate: mutate ?? undefined });
  if (!r.png) {
    results.push({ name, verdict: "error: " + r.error, ok: false });
    continue;
  }
  const cmp = comparePng(expected.png, r.png);
  const failed = !!cmp.sizeMismatch || cmp.diffPixels > 0;
  const expectFail = name !== "none";
  results.push({ name, diffPixels: cmp.diffPixels, sizeMismatch: cmp.sizeMismatch, verdict: failed ? "fail" : "pass", ok: failed === expectFail });
}
// Missing font: block the woff2 files; the verifier must report incomplete.
const nf = await capture(browser, base, cap, SIDE, { blockFonts: true });
results.push({ name: "missingFont", missingFonts: nf.missingFonts, verdict: nf.missingFonts.length ? "incomplete" : "pass", ok: nf.missingFonts.length > 0 });
await browser.close();
server.close();

for (const r of results)
  console.log(`${r.ok ? "OK  " : "BAD "} ${r.name.padEnd(12)} verifier verdict: ${r.verdict}${r.diffPixels !== undefined ? ` (${r.diffPixels} px${r.sizeMismatch ? ", size " + r.sizeMismatch : ""})` : ""}${r.missingFonts ? " missing: " + r.missingFonts.join(", ") : ""}`);
const ok = results.every((r) => r.ok);
console.log(ok ? "\nSelftest PASSED: the verifier fails on font, Dinghy shape, colour and spacing changes and on missing fonts." : "\nSelftest FAILED: the verifier missed a deliberate change.");
process.exit(ok ? 0 : 1);
