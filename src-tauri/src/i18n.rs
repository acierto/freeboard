//! Languages on the native side (spec "Languages"): the menu bar text, the
//! tray menu and notifications. The strings are the interface's own JSON
//! files, so there is one source per language.

use crate::settings::Language;
use std::collections::HashMap;
use std::sync::OnceLock;

pub const LANGS: [&str; 5] = ["en", "nl", "de", "fr", "es"];

const FILES: [(&str, &str); 5] = [
    ("en", include_str!("../../src/i18n/en.json")),
    ("nl", include_str!("../../src/i18n/nl.json")),
    ("de", include_str!("../../src/i18n/de.json")),
    ("fr", include_str!("../../src/i18n/fr.json")),
    ("es", include_str!("../../src/i18n/es.json")),
];

fn tables() -> &'static HashMap<&'static str, HashMap<String, String>> {
    static T: OnceLock<HashMap<&'static str, HashMap<String, String>>> = OnceLock::new();
    T.get_or_init(|| FILES.iter().map(|(l, src)| (*l, serde_json::from_str(src).expect("valid language file"))).collect())
}

/// A supported language from a locale tag ("de-AT", "pt_BR"), if any.
pub fn supported(tag: &str) -> Option<&'static str> {
    let code = tag.split(['-', '_']).next()?.to_ascii_lowercase();
    LANGS.iter().copied().find(|l| *l == code)
}

/// The first preferred system language Freeboard supports, else English.
pub fn system_language() -> &'static str {
    sys_locale::get_locales().find_map(|l| supported(&l)).unwrap_or("en")
}

/// The language in use: the saved choice, else the system's.
pub fn resolve(pref: Language) -> &'static str {
    pref.code().unwrap_or_else(system_language)
}

/// The string for `key`, English when the language lacks it.
pub fn tr(lang: &str, key: &str) -> String {
    let t = tables();
    t.get(lang).and_then(|m| m.get(key)).or_else(|| t["en"].get(key)).cloned().unwrap_or_else(|| key.to_string())
}

/// `tr` with `{name}` placeholders filled in.
pub fn trv(lang: &str, key: &str, vars: &[(&str, String)]) -> String {
    vars.iter().fold(tr(lang, key), |s, (k, v)| s.replace(&format!("{{{k}}}"), v))
}

/// "12.4 GB", "12,4 GB", "12,4 Go".
pub fn gb(lang: &str, bytes: u64) -> String {
    let n = format!("{:.1}", bytes as f64 / crate::model::GB as f64);
    let n = if lang == "en" { n } else { n.replace('.', ",") };
    format!("{n} {}", if lang == "fr" { "Go" } else { "GB" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_every_english_key() {
        let t = tables();
        for l in LANGS {
            let missing: Vec<&String> = t["en"].keys().filter(|k| !t[l].contains_key(*k)).collect();
            assert!(missing.is_empty(), "{l} lacks {missing:?}");
        }
    }

    #[test]
    fn sizes_follow_the_language() {
        assert_eq!(gb("en", 12_400_000_000), "12.4 GB");
        assert_eq!(gb("de", 12_400_000_000), "12,4 GB");
        assert_eq!(gb("fr", 12_400_000_000), "12,4 Go");
    }

    #[test]
    fn locale_tags_map_to_languages() {
        assert_eq!(supported("de-AT"), Some("de"));
        assert_eq!(supported("pt_BR"), None);
        assert_eq!(supported("FR"), Some("fr"));
        assert_eq!(resolve(Language::Es), "es");
    }

    #[test]
    fn missing_keys_fall_back_to_english() {
        assert_eq!(tr("de", "no.such.key"), "no.such.key");
        assert_eq!(trv("de", "nt.none", &[("f", gb("de", 4_000_000_000))]), "Noch 4,0 GB frei. Öffne Freeboard, um zu sehen, was du entfernen kannst.");
    }
}
