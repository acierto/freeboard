// Report installer sizes against the 15 MB target (source "Performance targets").
// Usage: node scripts/perf/installer-size.mjs [bundleDir]
import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const TARGET = 15 * 1000 * 1000;
const dir = process.argv[2] ?? "src-tauri/target/release/bundle";
const exts = [".dmg", ".deb", ".AppImage", ".exe", ".msi"];
const found = [];
function walk(d) {
  let entries = [];
  try { entries = readdirSync(d, { withFileTypes: true }); } catch { return; }
  for (const e of entries) {
    const p = join(d, e.name);
    if (e.isDirectory()) walk(p);
    else if (exts.some((x) => e.name.endsWith(x))) found.push({ file: p, bytes: statSync(p).size });
  }
}
walk(dir);
console.log("| Installer | Size | Target 15 MB |\n| --- | --- | --- |");
for (const f of found) {
  console.log(`| ${f.file} | ${(f.bytes / 1e6).toFixed(2)} MB | ${f.bytes <= TARGET ? "met" : "MISSED"} |`);
}
if (!found.length) {
  console.log("| (no installers found) | — | not measured |");
  process.exitCode = 1;
}
