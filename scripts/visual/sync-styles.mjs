// Copies the prototype's stylesheet into the app, unchanged.
//
//   node scripts/visual/sync-styles.mjs          write src/styles/tokens.css and harbor.css
//   node scripts/visual/sync-styles.mjs --check  exit 1 if either file differs from the prototype
//
// tokens.css holds the :root color, font and theme tokens (light, and the dark
// night scene for both the media query and data-theme="dark"); harbor.css holds
// every other rule. Together they are the prototype's single <style> block,
// byte for byte, so the app and the reference share one source of truth.
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { ROOT, PROTOTYPE } from "./lib.mjs";

const HEADER = (what) => `/* ${what} copied verbatim from docs/reference/freeboard-prototype.html by
   scripts/visual/sync-styles.mjs. Do not edit: run the script instead. */\n`;

export function splitPrototypeStyles(html) {
  const m = html.match(/<style>\n([\s\S]*?)<\/style>/);
  if (!m) throw new Error("prototype <style> block not found");
  const css = m[1];
  // Tokens: from the first ":root {" through the closing brace of the
  // :root[data-theme="dark"] block.
  const start = css.indexOf(":root {");
  const darkStart = css.indexOf(':root[data-theme="dark"] {');
  if (start < 0 || darkStart < 0) throw new Error("token blocks not found");
  const end = css.indexOf("}\n", darkStart) + 2;
  return { tokens: css.slice(start, end), rest: css.slice(0, start) + css.slice(end), css };
}

const { tokens, rest, css } = splitPrototypeStyles(readFileSync(PROTOTYPE, "utf8"));
const files = {
  [join(ROOT, "src/styles/tokens.css")]: HEADER("Design tokens") + tokens,
  [join(ROOT, "src/styles/harbor.css")]: HEADER("Layout and component styles") + rest,
};
// Self-check: the split loses nothing.
if (tokens.length + rest.length !== css.length) throw new Error("split lost characters");

if (process.argv.includes("--check")) {
  const bad = Object.entries(files).filter(([f, want]) => !existsSync(f) || readFileSync(f, "utf8") !== want);
  for (const [f] of bad) console.error("differs from the prototype: " + f);
  if (bad.length) process.exit(1);
  console.log("tokens.css and harbor.css match the prototype exactly");
} else {
  for (const [f, body] of Object.entries(files)) writeFileSync(f, body);
  console.log("wrote", Object.keys(files).join(", "));
}
