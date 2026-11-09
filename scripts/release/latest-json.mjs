// Writes the updater manifest (latest.json) for a release, from the signed
// update files the build jobs collected (docs/decisions.md, "Updates only when
// asked").
//
//   NOTES="one line" node scripts/release/latest-json.mjs <dir> <version> <download base URL>
//
// The app asks for `{os}-{arch}-{installer}` first, then `{os}-{arch}`:
// macOS replaces the .app from the .app.tar.gz; Windows runs the setup .exe
// (an MSI install gets the .msi); Linux updates the AppImage in place, or a
// .deb install through dpkg. Exits 1 if a platform is missing, so a release
// never ships a manifest that strands some installs.
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const [dir, version, base] = process.argv.slice(2);
if (!dir || !version || !base) {
  console.error("usage: latest-json.mjs <dir> <version> <download base URL>");
  process.exit(2);
}

/** platform key → file suffix after "Freeboard-<version>-" */
const ENTRIES = {
  "darwin-aarch64": "macOS-arm64.app.tar.gz",
  "darwin-aarch64-app": "macOS-arm64.app.tar.gz",
  "darwin-x86_64": "macOS-x64.app.tar.gz",
  "darwin-x86_64-app": "macOS-x64.app.tar.gz",
  "windows-x86_64": "windows-x64-setup.exe",
  "windows-x86_64-nsis": "windows-x64-setup.exe",
  "windows-x86_64-msi": "windows-x64.msi",
  "windows-aarch64": "windows-arm64-setup.exe",
  "windows-aarch64-nsis": "windows-arm64-setup.exe",
  "linux-x86_64": "linux-x86_64.AppImage",
  "linux-x86_64-appimage": "linux-x86_64.AppImage",
  "linux-x86_64-deb": "linux-x86_64.deb",
  "linux-aarch64": "linux-arm64.AppImage",
  "linux-aarch64-appimage": "linux-arm64.AppImage",
  "linux-aarch64-deb": "linux-arm64.deb",
};

const platforms = {};
const missing = [];
for (const [key, suffix] of Object.entries(ENTRIES)) {
  const name = `Freeboard-${version}-${suffix}`;
  const sig = join(dir, name + ".sig");
  if (!existsSync(join(dir, name)) || !existsSync(sig)) {
    missing.push(`${key}: ${name}`);
    continue;
  }
  platforms[key] = { signature: readFileSync(sig, "utf8").trim(), url: `${base}/${name}` };
}
if (missing.length) {
  console.error("update files missing or unsigned:\n  " + missing.join("\n  "));
  process.exit(1);
}
const notes = (process.env.NOTES ?? "").trim();
process.stdout.write(JSON.stringify({ version, notes, pub_date: new Date().toISOString(), platforms }, null, 2) + "\n");
