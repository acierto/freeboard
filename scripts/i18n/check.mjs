// Checks the translations (spec "Languages": CI fails if a key exists in
// English but not in another language).
//
// - Every key in src/i18n/en.json exists in nl, de, fr and es, with the same
//   {placeholders}.
// - Every backend template (catalog/templates.json and catalog/extra.json) has
//   a translation in each language, as a string or { one, other }. It keeps
//   every placeholder, except a plural ending glued to a word ("file{0}",
//   which holds "s" or ""): the translation handles plurals itself.
// - No translation file holds keys English does not have.
//
// Usage: node scripts/i18n/check.mjs   (exits 1 on any problem)
import { readFileSync } from "node:fs";

const ROOT = new URL("../..", import.meta.url).pathname;
const read = (p) => JSON.parse(readFileSync(ROOT + p, "utf8"));
const LANGS = ["nl", "de", "fr", "es"];
const HOLE = /\{([A-Za-z_][\w.]*|\d+)\}/g;
const holes = (s) => new Set([...s.matchAll(HOLE)].map((m) => m[1]));
/** Placeholders glued to a word on their left: plural endings. */
const endings = (s) => new Set([...s.matchAll(/[A-Za-z]\{([A-Za-z_][\w.]*|\d+)\}/g)].map((m) => m[1]));

const problems = [];
const en = read("src/i18n/en.json");
const templates = [...read("src/i18n/catalog/templates.json"), ...read("src/i18n/catalog/extra.json")];

for (const l of LANGS) {
  const ui = read(`src/i18n/${l}.json`);
  for (const [k, v] of Object.entries(en)) {
    if (!(k in ui)) problems.push(`${l}: missing key ${k}`);
    else if ([...holes(v)].sort().join() !== [...holes(ui[k])].sort().join()) problems.push(`${l}: ${k} has {${[...holes(ui[k])]}} but English has {${[...holes(v)]}}`);
  }
  for (const k of Object.keys(ui)) if (!(k in en)) problems.push(`${l}: key ${k} is not in English`);

  const cat = read(`src/i18n/catalog/${l}.json`);
  const known = new Set(templates);
  for (const tpl of templates) {
    const out = cat[tpl];
    if (out === undefined) {
      problems.push(`${l}: catalog lacks "${tpl.slice(0, 70)}"`);
      continue;
    }
    const forms = typeof out === "string" ? [out] : [out.one, out.other];
    if (forms.some((f) => typeof f !== "string" || !f.trim())) problems.push(`${l}: empty translation for "${tpl.slice(0, 70)}"`);
    const need = [...holes(tpl)].filter((h) => !endings(tpl).has(h));
    for (const f of forms.filter((x) => typeof x === "string")) {
      for (const h of need) if (!holes(f).has(h) && !(typeof out !== "string" && f === out.one && /^\d+$|^n$|^count$/.test(h))) problems.push(`${l}: "${tpl.slice(0, 60)}" lost {${h}}`);
      for (const h of holes(f)) if (!holes(tpl).has(h)) problems.push(`${l}: "${tpl.slice(0, 60)}" has unknown {${h}}`);
    }
  }
  for (const k of Object.keys(cat)) if (!known.has(k)) problems.push(`${l}: catalog has a template no longer in Rust: "${k.slice(0, 70)}"`);
}

if (problems.length) {
  for (const p of problems.slice(0, 80)) console.error("FAIL  " + p);
  console.error(`${problems.length} translation problem(s).`);
  process.exit(1);
}
console.log(`Translations complete: ${Object.keys(en).length} interface strings and ${templates.length} backend templates in ${LANGS.length + 1} languages.`);
