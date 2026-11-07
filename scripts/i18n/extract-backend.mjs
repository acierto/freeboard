// Collects the English text the Rust core writes for people to read (item
// reasons, recovery, labels, blind spots, risk notes, errors) into
// src/i18n/catalog/templates.json. The interface translates that text when it
// shows it (src/i18n/backend.ts), so a language change applies at once, also
// to items found before it.
//
// Rust format strings become templates: `{}` and `{:.1}` turn into {0}, {1}…
// by position, named `{n}` keep their name. Commands, paths, SQL and log text
// are left out by rule; src/i18n/catalog/exclude.json lists the rest that is
// not for people (reviewed by hand).
//
// Usage: node scripts/i18n/extract-backend.mjs [--check]
//   --check  fail when templates.json is out of date (CI)
import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = new URL("../..", import.meta.url).pathname;
const SRC = join(ROOT, "src-tauri/src");
const OUT = join(ROOT, "src/i18n/catalog/templates.json");
const EXCLUDE = new Set(JSON.parse(readFileSync(join(ROOT, "src/i18n/catalog/exclude.json"), "utf8")));

const TEST_MOD = /#\[cfg\((?:all\()?test[^\n]*\n\s*mod \w+ \{/;
// Text before the literal that marks it as not for people.
const NOT_FOR_PEOPLE = /(Command::new|\.arg\(|\.args\(|run_tool|tracing::|debug!|info!|warn!|error!|expect\(|panic!|assert|log::|strip_prefix\(|starts_with\(|ends_with\(|trim_end_matches\(|split\(|contains\(|== $|meta_set\(|meta_get\(|query_row|execute\(|prepare\(|env::var|join\($|header|"--)/;
// Text that is a command line, a path, an id or SQL.
// Text that is a command line: a tool and one of its subcommands.
const COMMAND = /^(docker (builder|image|container|volume|system|version|run|pull|compose)|git (worktree|status|-C|push|branch|log|rev-parse)|pnpm (store|install)|npm (cache|install)|pip3? (cache|freeze|install)|brew (cleanup|--cache)|xcrun simctl|cargo (build|clean)|ollama (rm|list)|gradle --stop|(rm|tar|du|ls|sudo|tmutil|diskutil|powershell|wsl|cmd|reg|xdg-open|explorer) )/;
// SQL is written in capitals here; sentences start with "Create", "Delete"…
const SQL = /^\s*(SELECT|INSERT|UPDATE|DELETE|CREATE|PRAGMA|ALTER|DROP|WITH|BEGIN)\b/;

function files(dir) {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    if (statSync(p).isDirectory()) return f === "tests" ? [] : files(p);
    return p.endsWith(".rs") && !/tests?\.rs$|contract_tests/.test(p) ? [p] : [];
  });
}

/** Rust format string → template: `{}` → {0}…, `{:.1}` → {k}, `{{` → `{`. */
export function template(rust) {
  let i = 0;
  return rust
    .replace(/\\n/g, "\n")
    .replace(/\\"/g, '"')
    .replace(/\\u\{([0-9a-fA-F]+)\}/g, (_, h) => String.fromCodePoint(parseInt(h, 16)))
    .replace(/\{\{|\}\}|\{([A-Za-z_][\w.]*)?(?::[^}]*)?\}/g, (m, name) => (m === "{{" ? "\u0000" : m === "}}" ? "\u0001" : `{${name ?? i++}}`))
    .replace(/\u0000/g, "{")
    .replace(/\u0001/g, "}");
}

const found = new Map();
for (const f of files(SRC)) {
  let s = readFileSync(f, "utf8");
  const m0 = s.match(TEST_MOD);
  if (m0) s = s.slice(0, m0.index);
  // Raw strings (SQL, scripts) and comments hold nothing for people.
  s = s.replace(/(?<![\w"])r(#+)"[\s\S]*?"\1/g, '""').replace(/(?<![\w"])r"[^"]*"/g, '""').replace(/'(?:\\.|[^'\\\n])'/g, "' '").replace(/^\s*\/\/.*$/gm, "").replace(/([^:"])\/\/[^"\n]*$/gm, "$1");
  for (const m of s.matchAll(/(?<!b|r#)"((?:[^"\\]|\\.)*)"/g)) {
    const raw = m[1];
    const before = s.slice(Math.max(0, m.index - 90), m.index).split("\n").pop();
    if (NOT_FOR_PEOPLE.test(before)) continue;
    const t = template(raw).trim();
    // People read sentences and labels: at least two letters-words, not code.
    if (!((/\s/.test(t) && /[A-Za-z]{3,}/.test(t)) || /^[A-Z][a-z]{2,}\.?$/.test(t))) continue;
        if (COMMAND.test(t) || SQL.test(t) || /^[\w.:/\\{}~-]+$/.test(t) || /^(\/|~\/|[A-Z]:\\|\$)/.test(t)) continue;
    if (EXCLUDE.has(t)) continue;
    if (!found.has(t)) found.set(t, relative(ROOT, f));
  }
}

const list = [...found.keys()].sort((a, b) => a.localeCompare(b));
const json = JSON.stringify(list, null, 2) + "\n";
if (process.argv.includes("--check")) {
  const now = readFileSync(OUT, "utf8");
  if (now !== json) {
    const old = new Set(JSON.parse(now));
    const added = list.filter((t) => !old.has(t));
    const gone = [...old].filter((t) => !found.has(t));
    console.error(`src/i18n/catalog/templates.json is out of date: ${added.length} new, ${gone.length} gone. Run node scripts/i18n/extract-backend.mjs.`);
    for (const t of added.slice(0, 20)) console.error("  + " + t);
    for (const t of gone.slice(0, 20)) console.error("  - " + t);
    process.exit(1);
  }
  console.log(`Backend text catalog is current (${list.length} templates).`);
} else {
  writeFileSync(OUT, json);
  if (process.argv.includes("--where")) for (const t of list) console.log(`${found.get(t)}\t${t}`);
  console.log(`Wrote ${list.length} templates.`);
}
