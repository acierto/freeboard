// npm run verify:visual:baseline -- --accept "<rationale>"
//
// Regenerates reference-derived baselines from the ORIGINAL prototype (never
// from the implementation). Refuses to run in CI and requires an explicit,
// recorded design-acceptance rationale, appended to docs/decisions.md.
import { appendFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { BASELINE_DIR, CROP, PROTOTYPE, PROTOTYPE_ONLY, PROTOTYPE_SHA256, REQUIRED_FONTS, ROOT, capture, engineInfo, launch, sha256, startServer } from "./lib.mjs";
import { captures } from "./matrix.mjs";

const args = process.argv.slice(2);
if (process.env.CI) {
  console.error("Refusing to update baselines in CI. Baselines change only with an explicit, reviewed design acceptance.");
  process.exit(1);
}
const i = args.indexOf("--accept");
const rationale = i >= 0 ? args[i + 1] : null;
if (!rationale || rationale.trim().length < 15) {
  console.error('Usage: npm run verify:visual:baseline -- --accept "<design acceptance rationale, affected screens>"');
  process.exit(1);
}
const protoSha = sha256(readFileSync(PROTOTYPE));
if (protoSha !== PROTOTYPE_SHA256) {
  console.error(`Prototype SHA-256 ${protoSha} differs from the recorded ${PROTOTYPE_SHA256}; the reference must be unchanged.`);
  process.exit(1);
}

rmSync(BASELINE_DIR, { recursive: true, force: true });
mkdirSync(BASELINE_DIR, { recursive: true });
const { server, base } = await startServer();
const browser = await launch();
const engine = engineInfo(browser);
const entries = {};
const failures = [];
const queue = captures();
await Promise.all(
  Array.from({ length: 6 }, async () => {
    while (queue.length) {
      const cap = queue.shift();
      const r = await capture(browser, base, cap, "ref");
      if (!r.png || r.missingFonts.length) {
        failures.push(`${cap.key}: ${r.error ?? "missing fonts " + r.missingFonts.join(", ")}`);
        continue;
      }
      writeFileSync(join(BASELINE_DIR, cap.key + ".png"), r.png);
      entries[cap.key] = {
        id: cap.id,
        surface: cap.surface,
        state: cap.state,
        theme: cap.theme,
        viewport: { width: cap.width, height: cap.height },
        deviceScaleFactor: 1,
        reducedMotion: !cap.motion,
        crop: CROP,
        fixture: cap.query ?? "",
        supplemental: !!cap.supplemental,
        note: cap.note ?? null,
        sha256: sha256(r.png),
      };
      process.stdout.write(".");
    }
  }),
);
await browser.close();
server.close();
if (failures.length) {
  console.error("\nBaseline capture failed:\n" + failures.join("\n"));
  process.exit(1);
}
const sorted = Object.fromEntries(Object.keys(entries).sort().map((k) => [k, entries[k]]));
const manifest = {
  version: 1,
  source: "docs/reference/freeboard-prototype.html",
  sourceSha256: protoSha,
  engine,
  fonts: REQUIRED_FONTS.map(([f, w]) => `${f} ${w} (src/fonts, SIL OFL 1.1)`),
  clock: "2026-11-04T12:00:00Z (frozen)",
  locale: "en-US",
  timezone: "UTC",
  adapter: `scripts/visual/lib.mjs referenceHtml(): local fonts, data-theme, host reset (src/styles/reset.css), prototype-only controls hidden (${PROTOTYPE_ONLY})`,
  acceptedAt: new Date().toISOString(),
  rationale,
  entries: sorted,
};
writeFileSync(join(BASELINE_DIR, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");

const decisions = join(ROOT, "docs/decisions.md");
const header = existsSync(decisions) && readFileSync(decisions, "utf8").includes("## Visual baselines") ? "" : "\n## Visual baselines\n\n";
appendFileSync(
  decisions,
  `${header}- ${new Date().toISOString().slice(0, 10)}: Baselines regenerated from \`docs/reference/freeboard-prototype.html\` (SHA-256 \`${protoSha.slice(0, 12)}…\`, ${engine.engine} ${engine.version}, ${Object.keys(sorted).length} captures). Rationale: ${rationale}\n`,
);
console.log(`\nWrote ${Object.keys(sorted).length} baselines and visual/baseline/manifest.json; recorded the acceptance in docs/decisions.md.`);
