// npm run verify:visual
//
// Compares the implementation (dist-visual, fixture backend) with
// reference-derived baselines of the supplied prototype, for every row of the
// matrix. Fails on any unaccepted pixel difference, missing capture, missing
// font, stale baseline, or production bundle containing fixture code.
//
// Options:
//   --only <regex>   limit to capture keys matching regex (report marks the run partial)
//   --live           compare against a fresh reference render instead of baselines
//                    (iteration aid, and CI: baselines are machine-specific)
//   --require-native exit 2 when native evidence is missing (release gate)
import { mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { execSync } from "node:child_process";
import { BASELINE_DIR, PROTOTYPE, PROTOTYPE_SHA256, ROOT, TOLERANCE_FILE, capture, engineInfo, launch, loadJson, motionChecks, sha256, startServer } from "./lib.mjs";
import { comparePng } from "./compare.mjs";
import { NATIVE, captures } from "./matrix.mjs";
import { writeReport } from "./report.mjs";

const args = process.argv.slice(2);
const only = args.includes("--only") ? new RegExp(args[args.indexOf("--only") + 1]) : null;
const live = args.includes("--live");
const requireNative = args.includes("--require-native");

const OUT = join(ROOT, "visual");
for (const d of ["actual", "diff", "report", "reference-live"]) {
  rmSync(join(OUT, d), { recursive: true, force: true });
  mkdirSync(join(OUT, d), { recursive: true });
}

const problems = [];
const protoSha = sha256(readFileSync(PROTOTYPE));
if (protoSha !== PROTOTYPE_SHA256) problems.push(`Prototype SHA-256 ${protoSha} does not match the recorded ${PROTOTYPE_SHA256}.`);

// The production bundle must not contain the fixture backend or sample data.
const distAssets = join(ROOT, "dist/assets");
if (!existsSync(distAssets)) problems.push("Production bundle dist/ is missing; cannot prove it excludes fixtures.");
else
  for (const f of readdirSync(distAssets).filter((f) => f.endsWith(".js")))
    if (/billing_pgdata|createFixtureBackend|fixtureBackend/.test(readFileSync(join(distAssets, f), "utf8")))
      problems.push(`Production bundle file dist/assets/${f} contains fixture code.`);

const manifest = loadJson(join(BASELINE_DIR, "manifest.json"), null);
if (!manifest && !live) problems.push("visual/baseline/manifest.json is missing.");
if (manifest && manifest.sourceSha256 !== protoSha) problems.push("Baselines were captured from a different prototype revision.");
const tolerances = loadJson(TOLERANCE_FILE, { version: 1, entries: {} });

const caps = captures().filter((c) => !only || only.test(c.key));
if (!captures().length) problems.push("The visual matrix has no entries yet; nothing has been compared with the prototype.");
const { server, base } = await startServer();
const browser = await launch();
const engine = engineInfo(browser);
if (manifest && !live && manifest.engine?.version !== engine.version)
  problems.push(`Baselines were captured with ${manifest.engine?.engine} ${manifest.engine?.version}, this run uses ${engine.version}.`);

let revision = "unknown";
try {
  revision = execSync("git rev-parse --short HEAD", { cwd: ROOT }).toString().trim() + (execSync("git status --porcelain", { cwd: ROOT }).toString().trim() ? "+dirty" : "");
} catch {}

const rows = [];
async function one(cap) {
  const row = { key: cap.key, id: cap.id, surface: cap.surface, state: cap.state, theme: cap.theme, viewport: `${cap.width}x${cap.height}`, supplemental: !!cap.supplemental, note: cap.note ?? null };
  const tol = tolerances.entries[cap.key] ?? tolerances.entries[cap.id] ?? { threshold: 0, maxDiffPixels: 0 };
  row.tolerance = tol;
  const baseFile = join(BASELINE_DIR, cap.key + ".png");
  let refPng;
  const ref = await capture(browser, base, cap, "ref");
  if (ref.png) writeFileSync(join(OUT, "reference-live", cap.key + ".png"), ref.png);
  if (live) {
    refPng = ref.png;
    if (!refPng) return Object.assign(row, { result: "incomplete", reason: "Reference capture failed: " + ref.error });
  } else {
    if (!existsSync(baseFile) || !manifest?.entries?.[cap.key]) return Object.assign(row, { result: "fail", reason: "Missing baseline (required matrix entry has no reference)." });
    refPng = readFileSync(baseFile);
    // The baseline must still be what the prototype renders.
    if (!ref.png) return Object.assign(row, { result: "incomplete", reason: "Reference re-render failed: " + ref.error });
    const drift = comparePng(refPng, ref.png);
    if (drift.diffPixels || drift.sizeMismatch) return Object.assign(row, { result: "fail", reason: `Baseline is stale: prototype now renders ${drift.diffPixels} different pixels${drift.sizeMismatch ? " (" + drift.sizeMismatch + ")" : ""}.` });
  }
  if (ref.missingFonts.length) return Object.assign(row, { result: "incomplete", reason: "Reference fonts missing: " + ref.missingFonts.join(", ") });
  const act = await capture(browser, base, cap, "impl");
  if (!act.png) return Object.assign(row, { result: "fail", reason: "Implementation capture failed: " + act.error });
  writeFileSync(join(OUT, "actual", cap.key + ".png"), act.png);
  if (act.missingFonts.length) return Object.assign(row, { result: "incomplete", reason: "Implementation fonts missing: " + act.missingFonts.join(", ") });
  const cmp = comparePng(refPng, act.png, tol);
  writeFileSync(join(OUT, "diff", cap.key + ".png"), cmp.diffPng);
  Object.assign(row, { diffPixels: cmp.diffPixels, totalPixels: cmp.total, sizeMismatch: cmp.sizeMismatch });
  if (cmp.sizeMismatch) return Object.assign(row, { result: "fail", reason: "Size differs: " + cmp.sizeMismatch });
  if (cmp.diffPixels > (tol.maxDiffPixels ?? 0)) return Object.assign(row, { result: "fail", reason: `${cmp.diffPixels} pixels differ (allowed ${tol.maxDiffPixels ?? 0}).` });
  return Object.assign(row, { result: "pass", reason: cmp.diffPixels ? `${cmp.diffPixels} pixels differ within the recorded tolerance.` : "Pixel-identical." });
}

const queue = [...caps];
const workers = Array.from({ length: 6 }, async () => {
  while (queue.length) {
    const cap = queue.shift();
    const row = await one(cap);
    rows.push(row);
    process.stdout.write(`${row.result.padEnd(10)} ${row.key}${row.result === "pass" ? "" : "  " + row.reason}\n`);
  }
});
await Promise.all(workers);
const motion = only ? [] : await motionChecks(browser, base);
for (const m of motion)
  if (!m.ok) problems.push(`Reduced-motion check failed on ${m.side} (${m.motion}): ${m.error ?? `bob ${m.bob}, waves ${m.wave}, boat transition ${m.boat}`}.`);
await browser.close();
server.close();

rows.sort((a, b) => a.key.localeCompare(b.key));
// Every required capture must have a row.
for (const c of caps) if (!rows.find((r) => r.key === c.key)) rows.push({ key: c.key, result: "fail", reason: "Missing capture." });

const native = NATIVE.map((n) => ({ ...n, result: "incomplete", reason: "Native evidence must be captured on the target platform (see docs/visual-verification.md)." }));
const review = loadJson(join(OUT, "review.json"), { reviews: {} });
const failed = rows.filter((r) => r.result === "fail").length;
const incomplete = rows.filter((r) => r.result === "incomplete").length;
const browserStatus = problems.length || failed ? "fail" : incomplete ? "incomplete" : "pass";
const overall = browserStatus === "pass" ? "incomplete" : browserStatus;
const result = {
  generatedAt: new Date().toISOString(),
  revision,
  sourceSha256: protoSha,
  engine,
  mode: live ? "live-reference" : "baseline",
  partial: !!only,
  problems,
  counts: { pass: rows.filter((r) => r.result === "pass").length, fail: failed, incomplete, native: native.length },
  browserStatus,
  overallStatus: overall,
  overallNote: overall === "incomplete" ? "Browser comparisons pass; native tray and Tauri webview evidence is still required on macOS, Linux and Windows." : undefined,
  rows,
  motionChecks: motion,
  native,
  reviews: review.reviews,
};
writeReport(OUT, result);
console.log(`\nBrowser comparisons: ${browserStatus.toUpperCase()} (${result.counts.pass} pass, ${failed} fail, ${incomplete} incomplete)`);
for (const p of problems) console.log("PROBLEM: " + p);
console.log(`Overall visual gate: ${overall.toUpperCase()} — report: visual/report/report.html`);
process.exit(browserStatus === "fail" ? 1 : browserStatus === "incomplete" ? 2 : requireNative ? 2 : 0);
