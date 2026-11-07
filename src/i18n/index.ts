// Interface languages (spec "Languages"). One JSON file per language, keyed by
// stable ids; English is the fallback for any missing key. The prototype's
// `T(key, en, vars)` is `t(key, vars)` here, with English in en.json.
import en from "./en.json" with { type: "json" };
import nl from "./nl.json" with { type: "json" };
import de from "./de.json" with { type: "json" };
import fr from "./fr.json" with { type: "json" };
import es from "./es.json" with { type: "json" };

export type Lang = "en" | "nl" | "de" | "fr" | "es";
/** The saved choice: a language, or the system's. */
export type LanguagePref = "system" | Lang;

export const LANGS: Lang[] = ["en", "nl", "de", "fr", "es"];
/** Each language in its own name. */
export const LANG_NAMES: Record<Lang, string> = { en: "English", nl: "Nederlands", de: "Deutsch", fr: "Français", es: "Español" };

const STRINGS: Record<Lang, Record<string, string>> = { en, nl, de, fr, es };

let current: Lang = "en";

/** A supported language from a language tag ("de-AT" → "de"), else English. */
export function langOf(tag: string | null | undefined): Lang {
  const c = String(tag ?? "").toLowerCase().split(/[-_]/)[0] as Lang;
  return LANGS.includes(c) ? c : "en";
}

/** The first supported language in a preference list (the prototype's
 * `systemLang()`), else English. */
export function firstSupported(tags: readonly string[]): Lang {
  for (const tag of tags) {
    const c = String(tag).toLowerCase().split(/[-_]/)[0] as Lang;
    if (LANGS.includes(c)) return c;
  }
  return "en";
}

export function resolveLang(pref: LanguagePref | undefined, system: string | null | undefined): Lang {
  return !pref || pref === "system" ? langOf(system) : pref;
}

/** Set the language used by `t` and the number formats. */
export function setLang(l: Lang) {
  current = l;
  if (typeof document !== "undefined") document.documentElement.lang = l;
}
export const lang = () => current;

/** The string for `key` in the current language, with `{name}` filled in. */
export function t(key: string, vars?: Record<string, string | number>, l: Lang = current): string {
  let s = STRINGS[l][key] ?? STRINGS.en[key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.split("{" + k + "}").join(String(v));
  return s;
}

/** "1 crate" / "3 crates": `key1` for one, `keyN` otherwise (prototype pattern). */
export const tn = (one: string, many: string, n: number, vars?: Record<string, string | number>) => t(n === 1 ? one : many, { n, ...vars });

/** One decimal, with a decimal comma outside English. */
export const fx = (n: number, l: Lang = current) => (l === "en" ? n.toFixed(1) : n.toFixed(1).replace(".", ","));
/** French writes gigaoctets. */
export const unitGB = (l: Lang = current) => (l === "fr" ? "Go" : "GB");

/** `t`, or `fallback` when no language has `key` (item and area names). */
export function tf(key: string, fallback: string, vars?: Record<string, string | number>): string {
  return key in STRINGS.en || key in STRINGS[current] ? t(key, vars) : fallback;
}

/** An area or group name by its English label ("AI models"). */
export const areaName = (label: string) => tf("g." + label, label);

/** "18 min ago", "vor 18 Min.": minutes follow the prototype's `time.18`
 * pattern; hours and days use Intl.RelativeTimeFormat (spec "Languages"). */
export function ago(minutes: number): string {
  if (minutes < 1) return t("time.now");
  if (minutes < 60) return t("time.18").replace("18", String(minutes));
  const h = Math.floor(minutes / 60);
  const [n, unit] = h < 24 ? [h, "hour"] : [Math.floor(h / 24), "day"];
  if (current === "en") return `${n} ${unit}${n === 1 ? "" : "s"} ago`;
  return new Intl.RelativeTimeFormat(current, { numeric: "always" }).format(-n, unit as Intl.RelativeTimeFormatUnit);
}
