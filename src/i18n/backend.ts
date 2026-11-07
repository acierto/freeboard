// Text the Rust core writes (item reasons, recovery, labels, blind spots, risk
// notes, errors) arrives in English. `tb` shows it in the language in use,
// from the catalog that scripts/i18n/extract-backend.mjs collects from the
// Rust source (templates.json) and the translations per language
// (catalog/<code>.json). Translating on display keeps a language change
// instant, also for items found before it.
//
// A template's placeholders ({0}, {n}…) match any text; what they captured is
// translated in turn (an age, a reason) and sizes are written the local way.
// A translation is a string, or { one, other } chosen by the first number.
import nl from "./catalog/nl.json" with { type: "json" };
import de from "./catalog/de.json" with { type: "json" };
import fr from "./catalog/fr.json" with { type: "json" };
import es from "./catalog/es.json" with { type: "json" };
import templates from "./catalog/templates.json" with { type: "json" };
import extra from "./catalog/extra.json" with { type: "json" };
import { fx, lang, tf, unitGB, type Lang } from "./index.ts";

type Plural = { one: string; other: string };
type Table = Record<string, string | Plural>;
const TABLES: Record<Exclude<Lang, "en">, Table> = { nl, de, fr, es } as Record<Exclude<Lang, "en">, Table>;

interface Compiled {
  re: RegExp;
  names: string[];
  out: string | Plural;
}

const HOLE = /\{([A-Za-z_][\w.]*|\d+)\}/g;
const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** Templates whose fixed text is this short would match nearly anything. */
const fixedLength = (t: string) => t.replace(HOLE, "").replace(/[^A-Za-z]/g, "").length;
const ALL: string[] = [...(templates as string[]), ...(extra as string[])];
const EXTRA = new Set(extra as string[]);

const cache = new Map<Lang, { exact: Map<string, string | Plural>; patterns: Compiled[] }>();

function compiled(l: Exclude<Lang, "en">) {
  let c = cache.get(l);
  if (c) return c;
  const table = TABLES[l];
  const exact = new Map<string, string | Plural>();
  const patterns: Compiled[] = [];
  for (const en of ALL) {
    const out = table[en];
    if (out === undefined) continue;
    if (!en.match(HOLE)) {
      exact.set(en, out);
      continue;
    }
    if (fixedLength(en) < 4 && !EXTRA.has(en)) continue;
    const names: string[] = [];
    let src = "";
    let last = 0;
    for (const m of en.matchAll(HOLE)) {
      src += escape(en.slice(last, m.index));
      // A placeholder at the very start or end may be empty ("{0}" for an
      // optional sentence); inside the text it holds at least one character.
      // A plural ending glued to a word ("file{0}") is empty in the singular.
      const atEdge = m.index === 0 || m.index! + m[0].length === en.length || /[A-Za-z]$/.test(en.slice(0, m.index));
      src += atEdge ? "([\\s\\S]*?)" : "([\\s\\S]+?)";
      names.push(m[1]);
      last = m.index! + m[0].length;
    }
    src += escape(en.slice(last));
    patterns.push({ re: new RegExp("^" + src + "$"), names, out });
  }
  // The most specific template first: the one with the most fixed text.
  patterns.sort((a, b) => b.re.source.replace(/\(\[\\s\\S\]\*?\+?\?\)/g, "").length - a.re.source.replace(/\(\[\\s\\S\]\*?\+?\?\)/g, "").length);
  c = { exact, patterns };
  cache.set(l, c);
  return c;
}

/** "12.4 GB" → "12,4 GB" / "12,4 Go" inside any text. */
export function localSizes(s: string, l: Lang = lang()): string {
  if (l === "en") return s;
  return s.replace(/(\d+)\.(\d+) GB\b/g, (_, a, b) => `${fx(Number(`${a}.${b}`), l)} ${unitGB(l)}`).replace(/(\d+) GB\b/g, (_, a) => `${a} ${unitGB(l)}`);
}

function pick(out: string | Plural, values: string[]): string {
  if (typeof out === "string") return out;
  const n = values.map((v) => v.trim()).find((v) => /^\d+$/.test(v));
  return n === "1" ? out.one : out.other;
}

function translate(text: string, l: Exclude<Lang, "en">, depth: number): string | null {
  const { exact, patterns } = compiled(l);
  const hit = exact.get(text);
  if (hit !== undefined) return pick(hit, []);
  for (const p of patterns) {
    const m = p.re.exec(text);
    if (!m) continue;
    const values = m.slice(1);
    const filled: Record<string, string> = {};
    p.names.forEach((n, i) => (filled[n] = depth < 3 ? tbIn(values[i], l, depth + 1) : values[i]));
    return pick(p.out, values).replace(HOLE, (h, n) => (n in filled ? filled[n] : h));
  }
  return null;
}

function tbIn(text: string, l: Exclude<Lang, "en">, depth: number): string {
  if (!text || !/[A-Za-z]/.test(text)) return localSizes(text, l);
  const lead = text.match(/^\s*/)![0];
  const trail = text.match(/\s*$/)![0];
  const core = text.trim();
  const whole = translate(core, l, depth);
  if (whole !== null) return lead + localSizes(whole, l) + trail;
  // A sentence the core put in lowercase inside another ("Newest: last
  // commit today"): match it capitalized, then lowercase the result's first
  // letter again (not in German, where it may be a noun).
  if (/^[a-z]/.test(core)) {
    const up = translate(core[0].toUpperCase() + core.slice(1), l, depth);
    if (up !== null) return lead + localSizes(l === "de" ? up : up[0].toLowerCase() + up.slice(1), l) + trail;
  }
  // Several sentences put together: translate each one.
  const sentences = core.split(/(?<=[.!?])\s+(?=[A-Z"“(])/);
  if (sentences.length > 1) return lead + sentences.map((s) => tbIn(s, l, depth)).join(" ") + trail;
  return lead + localSizes(core, l) + trail;
}

/** Backend text in the language in use. */
export function tb(text: string | null | undefined): string {
  if (text == null) return "";
  const l = lang();
  return l === "en" ? text : tbIn(text, l, 0);
}

/** Does the catalog translate this backend text, as a whole, lowercased
 * inside another sentence, or sentence by sentence? (tests) */
export function tbKnown(text: string, l: Exclude<Lang, "en">): boolean {
  const core = text.trim();
  if (translate(core, l, 0) !== null) return true;
  if (/^[a-z]/.test(core) && translate(core[0].toUpperCase() + core.slice(1), l, 0) !== null) return true;
  const sentences = core.split(/(?<=[.!?])\s+(?=[A-Z"“(])/);
  return sentences.length > 1 && sentences.every((x) => tbKnown(x, l));
}

/** An item's name in the language in use: the prototype's descriptive names
 * by item id (`n.<id>`), else the detector's title through the catalog.
 * Product names, paths and repository names stay as they are. */
export const itemName = (s: { id: string; title: string }) => tf("n." + s.id, tb(s.title));
