use super::*;
use crate::detectors::ai::*;
use crate::detectors::{probe, Probe};

fn digest(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

/// An Ollama model: manifest plus blobs of the given sizes.
fn ollama_model(fx: &Fx, path: &str, layers: &[(char, usize)]) {
    let models = fx.home.join(".ollama/models");
    let ls: Vec<String> = layers.iter().map(|(c, n)| format!(r#"{{"digest":"{}","size":{n}}}"#, digest(*c))).collect();
    fx.write(&format!(".ollama/models/manifests/{path}"), format!(r#"{{"schemaVersion":2,"layers":[{}]}}"#, ls.join(",")).as_bytes());
    for (c, n) in layers {
        let b = models.join("blobs").join(digest(*c).replace(':', "-"));
        if !b.exists() {
            std::fs::create_dir_all(b.parent().unwrap()).unwrap();
            std::fs::write(b, vec![0u8; *n]).unwrap();
        }
    }
}

#[test]
fn ollama_offers_idle_models_per_model_counting_only_their_own_layers() {
    let fx = Fx::new();
    ollama_model(&fx, "registry.ollama.ai/library/llama3/8b", &[('a', 50_000), ('d', 10_000)]);
    ollama_model(&fx, "registry.ollama.ai/library/mistral/latest", &[('b', 40_000), ('d', 10_000)]);
    ollama_model(&fx, "registry.ollama.ai/library/qwen/7b", &[('c', 30_000)]);
    age(&fx.home.join(".ollama"), 90);
    // qwen ran recently: its blob was read today.
    let qwen = fx.home.join(".ollama/models/blobs").join(digest('c').replace(':', "-"));
    let now = UNIX_EPOCH + Duration::from_secs(NOW as u64);
    std::fs::File::options().write(true).open(&qwen).unwrap().set_times(std::fs::FileTimes::new().set_accessed(now).set_modified(now)).unwrap();
    fx.tool("ollama", "exit 0");
    let o = fx.scan(ScanKind::Quick, &[Box::new(Ollama)]);
    let f = find(&o, OLLAMA);
    let names: Vec<&str> = f.suggestion.parts.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(names, ["llama3:8b", "mistral:latest"]);
    // The shared layer stays while another model uses it.
    assert_eq!(f.parts[0].suggestion.size_bytes, 50_000);
    assert!(f.parts[0].suggestion.size_is_estimate);
    let Target::Native { program, args, .. } = &f.parts[0].target else { panic!() };
    assert_eq!((program.as_str(), args.as_slice()), ("ollama", &["rm".to_string(), "llama3:8b".to_string()][..]));
    assert_eq!(f.suggestion.risk, Risk::Review);
    assert!(f.suggestion.blocked_reason.is_none());
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(&f.parts[0], ctx), Probe::Unchanged));
    // Run after the scan: changed.
    let blob = fx.home.join(".ollama/models/blobs").join(digest('a').replace(':', "-"));
    std::fs::File::options().write(true).open(&blob).unwrap().set_times(std::fs::FileTimes::new().set_accessed(now)).unwrap();
    fx.with_ctx(ScanKind::Quick, |ctx| assert!(matches!(probe(&f.parts[0], ctx), Probe::Changed(_))));
}

#[test]
fn ollama_malformed_manifest_marks_it_unavailable_and_missing_cli_blocks() {
    let fx = Fx::new();
    ollama_model(&fx, "registry.ollama.ai/library/llama3/8b", &[('a', 50_000)]);
    age(&fx.home.join(".ollama"), 90);
    // Models on disk but no `ollama` to remove them with.
    let o = fx.scan(ScanKind::Quick, &[Box::new(Ollama)]);
    assert!(find(&o, OLLAMA).suggestion.blocked_reason.is_some());
    fx.write(".ollama/models/manifests/registry.ollama.ai/library/bad/latest", b"{not json");
    let o = fx.scan(ScanKind::Quick, &[Box::new(Ollama)]);
    assert!(o.findings.is_empty());
    assert_eq!((o.errors[0].detector.as_str(), o.errors[0].kind.as_str()), (OLLAMA, "malformed"));
}

#[test]
fn ollama_rm_is_the_only_allowed_action() {
    use crate::runner::{check, Cmd, Confirmed, Policy};
    let ok = Confirmed::new("p");
    assert!(check(&Cmd::new("ollama", ["rm", "llama3:8b"]), Policy::Action, Some(&ok)).is_ok());
    for bad in [&["rm", "-f"][..], &["rm", "a", "b"], &["pull", "x"], &["rm", "a;b"]] {
        assert!(check(&Cmd::new("ollama", bad.iter().copied()), Policy::Action, Some(&ok)).is_err(), "{bad:?}");
    }
    assert!(check(&Cmd::new("ollama", ["rm", "x"]), Policy::ReadOnly, None).is_err());
}

#[test]
fn huggingface_repositories_are_parts_and_go_to_the_trash() {
    let fx = Fx::new();
    fx.write(".cache/huggingface/hub/models--org--bert/blobs/aaa", &[1u8; 40_000]);
    fx.write(".cache/huggingface/hub/datasets--squad/blobs/bbb", &[1u8; 20_000]);
    // Hub bookkeeping is not a repository.
    fx.write(".cache/huggingface/hub/.locks/models--org--bert/x.lock", &[1u8; 10]);
    fx.write(".cache/huggingface/hub/CACHEDIR.TAG", b"Signature");
    let o = fx.scan(ScanKind::Quick, &[Box::new(HuggingFace)]);
    let f = find(&o, HUGGINGFACE);
    let labels: Vec<&str> = f.suggestion.parts.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["org/bert", "squad (dataset)"]);
    assert_eq!((f.suggestion.risk, f.suggestion.action, f.suggestion.group), (Risk::Review, Action::Trash, Group::AiModels));
    assert!(f.paths().iter().all(|p| !p.contains(".locks")));
}

#[test]
fn lm_studio_models_split_by_model_in_both_locations() {
    let fx = Fx::new();
    fx.write(".lmstudio/models/lmstudio-community/Qwen-7B-GGUF/q4.gguf", &[1u8; 60_000]);
    fx.write(".lmstudio/models/bartowski/Llama-3-8B-GGUF/q8.gguf", &[1u8; 30_000]);
    fx.write(".cache/lm-studio/models/TheBloke/Mistral-GGUF/m.gguf", &[1u8; 20_000]);
    let o = fx.scan(ScanKind::Quick, &[Box::new(LmStudio)]);
    let f = find(&o, LM_STUDIO);
    assert_eq!(f.parts.len(), 3);
    assert_eq!(f.suggestion.parts[0].label, "lmstudio-community/Qwen-7B-GGUF");
    assert_eq!(f.suggestion.action, Action::Trash);
    fx.with_ctx(ScanKind::Quick, |ctx| assert_eq!(probe(&f.parts[1], ctx), Probe::Unchanged));
    // Nothing installed: nothing found, no error.
    let empty = Fx::new();
    let o = empty.scan(ScanKind::Quick, &[Box::new(LmStudio), Box::new(HuggingFace), Box::new(Ollama)]);
    assert!(o.findings.is_empty() && o.errors.is_empty());
}
