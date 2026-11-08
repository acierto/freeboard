//! AI models: Ollama models not run for 60+ days, Hugging Face downloads and
//! LM Studio models. One part per model (or Hugging Face repository).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::common::*;
use super::Probe;
use crate::guard::Guard;
use crate::model::*;
use crate::platform;
use crate::scan::{Detector, ScanContext};

pub const OLLAMA: &str = "ai.ollama";
pub const HUGGINGFACE: &str = "ai.huggingface";
pub const LM_STUDIO: &str = "ai.lm_studio";
pub const OLLAMA_IDLE_DAYS: i64 = 60;

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut v: Vec<PathBuf> = rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path()).collect();
    v.sort();
    v
}

fn name_of(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// Newest of modification and access time: reading a model counts as use.
fn touched(meta: &std::fs::Metadata) -> i64 {
    platform::modified_secs(meta).max(platform::accessed_secs(meta))
}

// ----- Ollama ----------------------------------------------------------------

pub struct Ollama;

pub fn ollama_models(ctx: &ScanContext) -> PathBuf {
    env_override(ctx, "OLLAMA_MODELS").unwrap_or_else(|| ctx.home.join(".ollama").join("models"))
}

#[derive(Debug, Clone, PartialEq)]
pub struct OllamaModel {
    /// The name `ollama rm` takes: "llama3:8b", "user/model:tag",
    /// "host/ns/model:tag" (default registry and library left out).
    pub name: String,
    pub manifest: PathBuf,
    /// sha256 digests of the model's layers and config, with sizes.
    pub layers: Vec<(String, u64)>,
}

/// Layers of one manifest: Docker distribution manifest v2 JSON with
/// `config` and `layers`, each `{digest, size}`. None when unreadable.
pub fn parse_manifest(json: &str) -> Option<Vec<(String, u64)>> {
    let v: Value = serde_json::from_str(json).ok()?;
    let mut out = vec![];
    let mut add = |l: &Value| -> Option<()> {
        let d = l.get("digest")?.as_str()?;
        let hex = d.strip_prefix("sha256:")?;
        if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        out.push((d.to_string(), l.get("size")?.as_u64()?));
        Some(())
    };
    if let Some(c) = v.get("config") {
        add(c)?;
    }
    for l in v.get("layers")?.as_array()? {
        add(l)?;
    }
    Some(out)
}

/// A model name is passed to `ollama rm` as one argument: letters, digits
/// and `._-:/` only, never starting with "-".
pub fn safe_model_name(n: &str) -> bool {
    !n.is_empty() && !n.starts_with('-') && n.chars().all(|c| c.is_ascii_alphanumeric() || "._-:/".contains(c))
}

/// Every manifest under `manifests/<host>/<namespace>/<model>/<tag>`.
pub fn list_ollama(models: &Path) -> Result<Vec<OllamaModel>, String> {
    let root = models.join("manifests");
    let mut out = vec![];
    for host in subdirs(&root) {
        for ns in subdirs(&host) {
            for model in subdirs(&ns) {
                let Ok(rd) = std::fs::read_dir(&model) else { continue };
                for tag in rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_file())) {
                    let p = tag.path();
                    let json = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
                    let layers = parse_manifest(&json).ok_or("Ollama has a model manifest Freeboard could not read.")?;
                    let (h, n, m, t) = (name_of(&host), name_of(&ns), name_of(&model), name_of(&p));
                    let name = match (h.as_str(), n.as_str()) {
                        ("registry.ollama.ai", "library") => format!("{m}:{t}"),
                        ("registry.ollama.ai", _) => format!("{n}/{m}:{t}"),
                        _ => format!("{h}/{n}/{m}:{t}"),
                    };
                    if !safe_model_name(&name) {
                        continue;
                    }
                    out.push(OllamaModel { name, manifest: p, layers });
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn blob_path(models: &Path, digest: &str) -> PathBuf {
    models.join("blobs").join(digest.replace(':', "-"))
}

/// When a model was last pulled or read: its manifest and its layers.
fn ollama_last_used(models: &Path, m: &OllamaModel) -> Option<i64> {
    let mut t = touched(&std::fs::metadata(&m.manifest).ok()?);
    for (d, _) in &m.layers {
        if let Ok(meta) = std::fs::metadata(blob_path(models, d)) {
            t = t.max(touched(&meta));
        }
    }
    Some(t)
}

fn ollama_identity(m: &OllamaModel) -> String {
    let digests: Vec<&str> = m.layers.iter().map(|(d, _)| d.as_str()).collect();
    fingerprint_of(&[&[m.name.as_str()][..], &digests].concat())
}

impl Detector for Ollama {
    fn id(&self) -> &'static str {
        OLLAMA
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let models = ollama_models(ctx);
        if !models.join("manifests").is_dir() {
            return Ok(vec![]);
        }
        let all = list_ollama(&models).map_err(|m| err(OLLAMA, "malformed", m))?;
        // A layer frees space only when no other model uses it.
        let mut users: BTreeMap<&str, usize> = BTreeMap::new();
        for m in &all {
            for (d, _) in &m.layers {
                *users.entry(d.as_str()).or_default() += 1;
            }
        }
        let mut parts = vec![];
        for m in &all {
            let Some(last) = ollama_last_used(&models, m) else { continue };
            if days_since(ctx.now, last) < OLLAMA_IDLE_DAYS {
                continue;
            }
            let own: u64 = m.layers.iter().filter(|(d, _)| users[d.as_str()] == 1).map(|(_, s)| *s).sum();
            if own == 0 {
                continue;
            }
            let shared = m.layers.iter().any(|(d, _)| users[d.as_str()] > 1);
            let mut s = base(OLLAMA, Group::AiModels, &m.name);
            s.title = m.name.clone();
            s.location = display_path(ctx, &models);
            s.size_bytes = own;
            s.full_size_bytes = own;
            s.size_is_estimate = shared;
            s.last_used_at = rfc3339(last);
            s.last_used_label = format!("Not run in {}+ days", days_since(ctx.now, last));
            s.risk = Risk::Review;
            s.action = Action::NativeCommand;
            s.action_label = format!("ollama rm {}", m.name);
            s.reason = OLLAMA_REASON.into();
            s.recovery = format!("Download it again with ollama pull {}.", m.name);
            s.recoverable = true;
            s.fingerprint = ollama_identity(m);
            parts.push(Finding::new(
                s,
                Target::Native { program: "ollama".into(), args: vec!["rm".into(), m.name.clone()], resources: vec![m.name.clone()], cwd: None },
            ));
        }
        Ok(native_item(ctx, parts, &models).into_iter().collect())
    }
}

const OLLAMA_REASON: &str = "Ollama models you have not run for 60 days or more.";

/// The Ollama models as one item with a part per model.
fn native_item(ctx: &ScanContext, mut parts: Vec<Finding>, models: &Path) -> Option<Finding> {
    if parts.is_empty() {
        return None;
    }
    let blocked = (!ctx.runner.available("ollama")).then(|| "Ollama is not installed, so Freeboard cannot remove its models.".to_string());
    if parts.len() == 1 {
        let mut f = parts.pop().unwrap();
        f.suggestion.title = format!("Ollama model {}", f.suggestion.title);
        f.suggestion.blocked_reason = blocked;
        return Some(f);
    }
    parts.sort_by_key(|f| std::cmp::Reverse(f.suggestion.size_bytes));
    let n = parts.len();
    let mut s = base(OLLAMA, Group::AiModels, "ollama-models");
    s.title = format!("{n} Ollama models not run for 60+ days");
    s.location = display_path(ctx, models);
    s.size_bytes = parts.iter().map(|f| f.suggestion.size_bytes).sum();
    s.full_size_bytes = s.size_bytes;
    s.size_is_estimate = parts.iter().any(|f| f.suggestion.size_is_estimate);
    s.last_used_at = parts.iter().filter_map(|f| f.suggestion.last_used_at.clone()).max();
    s.last_used_label = format!("Not run in {}+ days", OLLAMA_IDLE_DAYS);
    s.risk = Risk::Review;
    s.action = Action::NativeCommand;
    s.action_label = format!("ollama rm (the {n} models)");
    s.reason = OLLAMA_REASON.into();
    s.recovery = "Download them again with ollama pull.".into();
    s.recoverable = true;
    s.blocked_reason = blocked;
    s.parts = parts
        .iter()
        .map(|f| SuggestionPart {
            id: f.suggestion.id.clone(),
            label: f.suggestion.title.clone(),
            location: f.suggestion.location.clone(),
            size_bytes: f.suggestion.size_bytes,
            selected: true,
        })
        .collect();
    s.fingerprint = parts.iter().map(|f| f.suggestion.fingerprint.as_str()).collect::<Vec<_>>().join("|");
    let names: Vec<String> = parts.iter().map(|f| f.suggestion.title.clone()).collect();
    let mut args = vec!["rm".to_string()];
    args.extend(names.iter().cloned());
    Some(Finding { suggestion: s, target: Target::Native { program: "ollama".into(), args, resources: names, cwd: None }, parts })
}

// ----- Hugging Face ----------------------------------------------------------

pub struct HuggingFace;

pub fn hf_hub(ctx: &ScanContext) -> PathBuf {
    if let Some(p) = env_override(ctx, "HF_HUB_CACHE") {
        return p;
    }
    if let Some(p) = env_override(ctx, "HF_HOME") {
        return p.join("hub");
    }
    ctx.home.join(".cache").join("huggingface").join("hub")
}

/// "models--answerdotai--ModernBERT-base" → "answerdotai/ModernBERT-base";
/// datasets and spaces say so.
pub fn hf_repo_label(folder: &str) -> Option<String> {
    let (kind, rest) = folder.split_once("--")?;
    let name = rest.replace("--", "/");
    match kind {
        "models" => Some(name),
        "datasets" => Some(format!("{name} (dataset)")),
        "spaces" => Some(format!("{name} (space)")),
        _ => None,
    }
}

impl Detector for HuggingFace {
    fn id(&self) -> &'static str {
        HUGGINGFACE
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let hub = hf_hub(ctx);
        let entries = subdirs(&hub)
            .into_iter()
            .filter_map(|d| {
                let label = hf_repo_label(&name_of(&d))?;
                // Folder modification only: listing a folder (as this scan
                // does) updates its access time.
                let last = std::fs::metadata(&d).ok().map(|m| platform::modified_secs(&m));
                let last = last.map(|t| (Some(t), format!("Downloaded or used {}", ago(ctx.now, t))));
                Some((label, vec![d], last))
            })
            .collect();
        let tw = trash_word(ctx);
        let item = FolderItem {
            detector: HUGGINGFACE,
            group: Group::AiModels,
            identity: "huggingface-hub",
            title: "Hugging Face models and datasets".into(),
            location: display_path(ctx, &hub),
            risk: Risk::Review,
            action: Action::Trash,
            reason: "Models and datasets downloaded by Hugging Face libraries. A project may load one again at any time.".into(),
            recovery: format!("Restore them from the {tw}, or let the library download them again."),
        };
        Ok(split_finding(ctx, item, entries).into_iter().collect())
    }
}

// ----- LM Studio -------------------------------------------------------------

pub struct LmStudio;

/// LM Studio 0.3 keeps models in `~/.lmstudio/models`; older versions in
/// `~/.cache/lm-studio/models`. Both hold `<publisher>/<model>/` folders.
pub fn lm_studio_roots(ctx: &ScanContext) -> Vec<PathBuf> {
    vec![ctx.home.join(".lmstudio").join("models"), ctx.home.join(".cache").join("lm-studio").join("models")]
}

fn newest_touch(dir: &Path) -> Option<i64> {
    let mut t = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        if let Ok(m) = e.metadata() {
            t = Some(t.map_or(touched(&m), |x: i64| x.max(touched(&m))));
        }
    }
    t
}

impl Detector for LmStudio {
    fn id(&self) -> &'static str {
        LM_STUDIO
    }
    fn run(&self, ctx: &ScanContext) -> Result<Vec<Finding>, DetectorError> {
        let mut entries = vec![];
        let roots: Vec<PathBuf> = lm_studio_roots(ctx).into_iter().filter(|r| r.is_dir() && !platform::is_link(r)).collect();
        for root in &roots {
            for publisher in subdirs(root) {
                for model in subdirs(&publisher) {
                    let label = format!("{}/{}", name_of(&publisher), name_of(&model));
                    let last = newest_touch(&model).map(|t| (Some(t), format!("Last used {}", ago(ctx.now, t))));
                    entries.push((label, vec![model], last));
                }
            }
        }
        let Some(root) = roots.first() else { return Ok(vec![]) };
        let tw = trash_word(ctx);
        let item = FolderItem {
            detector: LM_STUDIO,
            group: Group::AiModels,
            identity: "lm-studio-models",
            title: "LM Studio models".into(),
            location: display_path(ctx, root),
            risk: Risk::Review,
            action: Action::Trash,
            reason: "Models downloaded in LM Studio.".into(),
            recovery: format!("Restore them from the {tw}, or download them again in LM Studio."),
        };
        Ok(split_finding(ctx, item, entries).into_iter().collect())
    }
}

// ----- pre-cleanup checks ----------------------------------------------------

pub fn probe(f: &Finding, ctx: &ScanContext, guard: &Guard) -> Probe {
    match (&f.target, f.suggestion.detector.as_str()) {
        (Target::Paths { .. }, _) => super::files::probe_paths(f, ctx, guard),
        (Target::Native { resources, .. }, OLLAMA) => {
            let models = ollama_models(ctx);
            let all = match list_ollama(&models) {
                Ok(a) => a,
                Err(m) => return Probe::Unknown(m),
            };
            if !ctx.runner.available("ollama") {
                return Probe::Unknown("Ollama is not installed, so Freeboard cannot remove its models.".into());
            }
            // One part per model; a combined item is never executed.
            let [name] = resources.as_slice() else { return Probe::Unknown("Unexpected cleanup target.".into()) };
            match all.iter().find(|m| &m.name == name) {
                None => Probe::Changed("This model is gone already.".into()),
                Some(m) if ollama_identity(m) != f.suggestion.fingerprint => Probe::Changed("This model was downloaded again after the scan.".into()),
                Some(m) => match ollama_last_used(&models, m) {
                    Some(t) if days_since(ctx.now, t) >= OLLAMA_IDLE_DAYS => Probe::Unchanged,
                    Some(_) => Probe::Changed("This model was run after the scan.".into()),
                    None => Probe::Unknown("Freeboard couldn't read when this model was last used.".into()),
                },
            }
        }
        _ => Probe::Unknown("Unknown AI model detector.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_and_names() {
        let d = format!("sha256:{}", "a".repeat(64));
        let j = format!(r#"{{"schemaVersion":2,"config":{{"digest":"{d}","size":485}},"layers":[{{"digest":"{d}","size":4661211808}}]}}"#);
        assert_eq!(parse_manifest(&j).unwrap().len(), 2);
        assert_eq!(parse_manifest(r#"{"layers":[{"digest":"sha256:zz","size":1}]}"#), None);
        assert_eq!(parse_manifest("not json"), None);
        assert!(safe_model_name("llama3.1:8b") && safe_model_name("user/m:latest"));
        assert!(!safe_model_name("-rf") && !safe_model_name("a b") && !safe_model_name("a;b"));
        assert_eq!(hf_repo_label("models--answerdotai--ModernBERT-base").unwrap(), "answerdotai/ModernBERT-base");
        assert_eq!(hf_repo_label("datasets--squad").unwrap(), "squad (dataset)");
        assert_eq!(hf_repo_label("blobs"), None);
        assert_eq!(hf_repo_label(".locks"), None);
    }
}
