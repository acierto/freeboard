//! Contract tests for the frontend boundary: capabilities, CSP and the
//! command list (task 1.2, invariants 3 and 12).

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(p: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root().join(p)).unwrap()).unwrap()
}

const FORBIDDEN_PERMISSION_PREFIXES: &[&str] = &[
    "fs:", "shell:", "dialog:", "http:", "opener:", "process:", "os:", "clipboard", "updater:", "websocket:", "upload:",
    "store:", "sql:", "core:webview:allow-create", "core:window:allow-create", "core:path:", "core:resources:",
];

#[test]
fn inv03_capabilities_grant_no_filesystem_shell_or_network_plugins() {
    let cap = read_json("capabilities/default.json");
    let perms: Vec<String> = cap["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().map(String::from).unwrap_or_else(|| p["identifier"].as_str().unwrap_or("").to_string()))
        .collect();
    for p in &perms {
        for bad in FORBIDDEN_PERMISSION_PREFIXES {
            assert!(!p.starts_with(bad), "capability grants forbidden permission {p}");
        }
    }
    assert!(perms.iter().any(|p| p == "allow-app-commands"));
    // Only our own window, and no remote URLs may use the capability.
    assert_eq!(cap["local"], serde_json::Value::Bool(true));
    assert!(cap.get("remote").is_none(), "no remote origins");
    // Every other capability file must be absent so nothing else is granted.
    let files: Vec<_> = std::fs::read_dir(root().join("capabilities")).unwrap().flatten().collect();
    assert_eq!(files.len(), 1, "only default.json is allowed in capabilities/");
}

#[test]
fn inv03_app_commands_take_ids_not_paths() {
    let toml = std::fs::read_to_string(root().join("permissions/app.toml")).unwrap();
    let expected = [
        "get_status", "start_scan", "get_suggestions", "create_plan", "create_permanent_plan", "get_plan", "acknowledge_item", "run_plan",
        "cancel_run", "get_run", "get_trash", "empty_freeboard_trash", "list_history", "get_settings",
        "update_settings", "add_folder", "remove_folder", "set_notifications_paused",
        // Rebuild command table (spec "Tauri commands").
        "get_disk_map", "get_blind_spots", "resolve_blind_spot", "get_places_checked", "get_growth", "get_pile",
        "set_pile_goal", "set_pile_areas", "add_to_pile", "take_out_of_pile", "set_item_decision", "set_item_parts",
        "set_tidy_rule", "create_disk_report", "list_logbook",
    ];
    for c in expected {
        assert!(toml.contains(&format!("\"{c}\"")), "{c} missing from allowlist");
    }
    // The handlers in commands/mod.rs must not accept free-form path or
    // command arguments.
    let src = std::fs::read_to_string(root().join("src/commands/mod.rs")).unwrap();
    // Every command the shell defines is on the allowlist, or the interface
    // cannot call it (a new command once shipped without it, 2026-11-05).
    let mut lines = src.lines();
    while let Some(l) = lines.next() {
        if l.trim() == "#[tauri::command]" {
            let f = lines.next().unwrap_or_default();
            let name = f.split("fn ").nth(1).and_then(|r| r.split('(').next()).unwrap_or_default().trim();
            assert!(!name.is_empty(), "unparsed command line: {f}");
            assert!(toml.contains(&format!("\"{name}\"")), "command {name} is missing from permissions/app.toml");
        }
    }
    for bad in ["path: String", "path: PathBuf", "command: String", "program: String", "args: Vec<String>", "cmd: String"] {
        assert!(!src.contains(bad), "command handler takes `{bad}`");
    }
}

#[test]
fn inv12_csp_forbids_remote_content() {
    let conf = read_json("tauri.conf.json");
    let csp = &conf["app"]["security"]["csp"];
    let all: String = csp.as_object().unwrap().iter().map(|(k, v)| format!("{k} {} ;", v.as_str().unwrap())).collect();
    for bad in ["http://", "https://", "*", "'unsafe-eval'", "data: 'self' https", "ws:", "wss:"] {
        if bad == "http://" {
            // The only allowed http origin is Tauri's IPC endpoint.
            let cleaned = all.replace("http://ipc.localhost", "");
            assert!(!cleaned.contains(bad), "CSP allows remote origin: {all}");
            continue;
        }
        assert!(!all.contains(bad), "CSP contains {bad}: {all}");
    }
    assert_eq!(csp["default-src"], "'self'");
    assert_eq!(csp["script-src"], "'self'");
    assert_eq!(csp["font-src"], "'self'");
    assert_eq!(csp["object-src"], "'none'");
    assert_eq!(conf["app"]["withGlobalTauri"], serde_json::Value::Bool(false));
}

#[test]
fn inv12_no_network_client_in_dependencies() {
    // Freeboard makes no network calls of its own: no HTTP or socket client
    // crates are direct dependencies, and the core never opens sockets.
    let cargo = std::fs::read_to_string(root().join("Cargo.toml")).unwrap();
    for bad in ["reqwest", "ureq", "hyper", "surf", "isahc", "tauri-plugin-http", "tauri-plugin-updater", "tungstenite", "sentry"] {
        assert!(!cargo.contains(bad), "network client dependency {bad}");
    }
    let mut stack = vec![root().join("src")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().map(|x| x == "rs").unwrap_or(false) && !p.ends_with("contract_tests.rs") {
                let s = std::fs::read_to_string(&p).unwrap();
                for bad in ["TcpStream", "UdpSocket", "std::net::", "TcpListener"] {
                    assert!(!s.contains(bad), "{} uses {bad}", p.display());
                }
            }
        }
    }
}

#[test]
fn inv12_frontend_has_no_remote_urls() {
    // The interface bundles its fonts and makes no remote requests.
    let src = root().join("../src");
    if !src.exists() {
        return;
    }
    let mut stack = vec![src];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
            if !["ts", "tsx", "css", "html"].contains(&ext) {
                continue;
            }
            let s = std::fs::read_to_string(&p).unwrap_or_default();
            for bad in ["fonts.googleapis.com", "fonts.gstatic.com", "fetch(\"http", "fetch('http", "new WebSocket", "XMLHttpRequest"] {
                assert!(!s.contains(bad), "{} contains {bad}", p.display());
            }
        }
    }
}

/// Every command argument must be something that cannot carry a path or a
/// command line: a framework handle, an ID (a `String` named `id`, `*_id` or
/// `token`, validated by `app_core::validate_id`), a list of IDs (`*_ids`), a
/// number, a boolean, or a typed enum or patch defined in Rust. A free-form
/// string under any other name fails this test (invariant 3).
#[test]
fn inv03_command_arguments_are_ids_enums_numbers_or_bools() {
    let src = std::fs::read_to_string(root().join("src/commands/mod.rs")).unwrap();
    // Typed, non-string inputs defined in Rust.
    const TYPED: &[&str] = &[
        "ScanKind", "Option<SuggestionFilter>", "FolderList", "UiSettingsPatch", "Goal", "Vec<Group>", "ItemDecision",
        "BlindSpotAction", "PlanSource", "TidyRuleId", "VolumeId",
    ];
    const FRAMEWORK: &[&str] = &["Core<'_>", "tauri::AppHandle", "AppHandle"];
    const SCALARS: &[&str] = &["u32", "u64", "bool", "Option<u32>", "Option<u64>"];
    let mut checked = 0;
    let mut lines = src.lines();
    while let Some(l) = lines.next() {
        if l.trim() != "#[tauri::command]" {
            continue;
        }
        let mut sig = String::new();
        for next in lines.by_ref() {
            sig.push_str(next.trim());
            if next.contains('{') {
                break;
            }
        }
        let args = sig.split_once('(').and_then(|(_, r)| r.rsplit_once(") ->").or_else(|| r.rsplit_once(')'))).map(|(a, _)| a).unwrap_or("");
        for arg in split_args(args) {
            let (name, ty) = arg.split_once(':').map(|(n, t)| (n.trim(), t.trim())).unwrap_or(("", ""));
            if name.is_empty() {
                continue;
            }
            let ok = FRAMEWORK.contains(&ty)
                || SCALARS.contains(&ty)
                || TYPED.contains(&ty)
                || (ty == "String" && (name == "id" || name.ends_with("_id") || name == "token"))
                || (ty == "Vec<String>" && name.ends_with("_ids"));
            assert!(ok, "command argument `{name}: {ty}` in `{sig}` could carry a path or command");
            checked += 1;
        }
    }
    assert!(checked > 10, "parsed too few command arguments ({checked})");
}

fn split_args(s: &str) -> Vec<String> {
    // Split on commas that are not inside <...> generics.
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    for ch in s.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

#[test]
fn inv03_argument_rule_rejects_a_path_argument() {
    // The rule above is only useful if it fails on a bad signature.
    let bad = ["path: String", "folder: String", "command: String", "paths: Vec<String>"];
    for arg in bad {
        let (name, ty) = arg.split_once(':').map(|(n, t)| (n.trim(), t.trim())).unwrap();
        let id_like = (ty == "String" && (name == "id" || name.ends_with("_id") || name == "token")) || (ty == "Vec<String>" && name.ends_with("_ids"));
        assert!(!id_like, "{arg} would have been accepted");
    }
}
