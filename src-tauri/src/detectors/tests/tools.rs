use super::*;
use crate::detectors::tools::*;
use crate::detectors::{probe, Probe};

fn node(fx: &Fx, v: &str) {
    fx.write(&format!(".nvm/versions/node/{v}/bin/node"), &[1u8; 30_000]);
}

#[test]
fn node_versions_keep_default_newest_and_every_requested_version() {
    let fx = Fx::new();
    for v in ["v16.20.2", "v18.19.0", "v20.18.1", "v21.7.3", "v22.8.0", "v24.18.0"] {
        node(&fx, v);
    }
    fx.write(".nvm/alias/default", b"22.8.0\n");
    // .nvmrc pins 18; engines asks for 20.x; volta pins nothing installed.
    fx.write("projects/a/.git/HEAD", b"ref: refs/heads/main\n");
    fx.write("projects/a/.nvmrc", b"lts/hydrogen\n");
    fx.write("projects/b/.git/HEAD", b"ref: refs/heads/main\n");
    fx.write("projects/b/package.json", br#"{"engines":{"node":"^20.0.0"},"volta":{"node":"19.0.0"}}"#);
    let o = fx.scan(ScanKind::Deep, &[Box::new(NodeVersions)]);
    let f = find(&o, NODE_VERSIONS);
    let labels: Vec<&str> = f.suggestion.parts.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels.len(), 2, "{labels:?}");
    assert!(labels.contains(&"Node 16.20.2") && labels.contains(&"Node 21.7.3"));
    assert_eq!((f.suggestion.risk, f.suggestion.action, f.suggestion.group), (Risk::Review, Action::Trash, Group::OldToolsApps));
    let p16 = f.parts.iter().find(|p| p.suggestion.title == "Node 16.20.2").unwrap();
    fx.with_ctx(ScanKind::Deep, |ctx| assert_eq!(probe(p16, ctx), Probe::Unchanged));
    // The default moves to 16 after the scan.
    fx.write(".nvm/alias/default", b"16\n");
    fx.with_ctx(ScanKind::Deep, |ctx| assert!(matches!(probe(p16, ctx), Probe::Changed(_))));
    // Quick scans do not know the projects.
    assert!(fx.scan(ScanKind::Quick, &[Box::new(NodeVersions)]).findings.is_empty());
}

#[test]
fn node_versions_unreadable_request_keeps_everything() {
    let fx = Fx::new();
    for v in ["v18.19.0", "v20.18.1"] {
        node(&fx, v);
    }
    fx.write("projects/a/.git/HEAD", b"ref: refs/heads/main\n");
    fx.write("projects/a/package.json", br#"{"engines":{"node":"something weird!"}}"#);
    let o = fx.scan(ScanKind::Deep, &[Box::new(NodeVersions)]);
    assert!(o.findings.is_empty());
    assert_eq!((o.errors[0].detector.as_str(), o.errors[0].kind.as_str()), (NODE_VERSIONS, "malformed"));
}

#[test]
fn rust_toolchains_keep_default_and_pinned() {
    let fx = Fx::new();
    let host = "aarch64-apple-darwin";
    let names = [format!("stable-{host}"), format!("nightly-2024-01-01-{host}"), format!("1.75.0-{host}"), format!("1.70.0-{host}")];
    for n in &names {
        fx.write(&format!(".rustup/toolchains/{n}/bin/rustc"), &[1u8; 40_000]);
    }
    let list = fx.root.join("toolchains.txt");
    std::fs::write(&list, format!("{} (active, default)\n{}\n{}\n{}\n", names[0], names[1], names[2], names[3])).unwrap();
    fx.tool("rustup", &format!("[ \"$1 $2\" = 'toolchain list' ] || exit 9; cat '{}'", list.display()));
    fx.write("projects/r/.git/HEAD", b"ref: refs/heads/main\n");
    fx.write("projects/r/rust-toolchain.toml", b"[toolchain]\nchannel = \"1.75\"\ncomponents = [\"clippy\"]\n");
    fx.write("projects/n/.git/HEAD", b"ref: refs/heads/main\n");
    fx.write("projects/n/rust-toolchain", b"nightly-2024-01-01\n");
    let o = fx.scan(ScanKind::Deep, &[Box::new(RustToolchains)]);
    let f = find(&o, RUST_TOOLCHAINS);
    assert_eq!(f.suggestion.title, format!("Rust toolchain 1.70.0-{host}"));
    let Target::Native { program, args, .. } = &f.target else { panic!() };
    assert_eq!((program.as_str(), args.as_slice()), ("rustup", &["toolchain".to_string(), "uninstall".to_string(), format!("1.70.0-{host}")][..]));
    fx.with_ctx(ScanKind::Deep, |ctx| assert_eq!(probe(f, ctx), Probe::Unchanged));
    // It became the default after the scan.
    std::fs::write(&list, format!("{}\n{} (default)\n", names[0], names[3])).unwrap();
    fx.with_ctx(ScanKind::Deep, |ctx| assert!(matches!(probe(f, ctx), Probe::Changed(_))));
}

#[test]
fn rustup_actions_are_bounded() {
    use crate::runner::{check, Cmd, Confirmed, Policy};
    let ok = Confirmed::new("p");
    assert!(check(&Cmd::new("rustup", ["toolchain", "uninstall", "1.70.0-x86_64-unknown-linux-gnu"]), Policy::Action, Some(&ok)).is_ok());
    for bad in [&["toolchain", "uninstall"][..], &["toolchain", "uninstall", "a", "b"], &["self", "uninstall"], &["toolchain", "uninstall", "-y"], &["update"]] {
        assert!(check(&Cmd::new("rustup", bad.iter().copied()), Policy::Action, Some(&ok)).is_err(), "{bad:?}");
    }
    assert!(check(&Cmd::new("rustup", ["toolchain", "list"]), Policy::ReadOnly, None).is_ok());
    assert!(check(&Cmd::new("rustup", ["toolchain", "uninstall", "x"]), Policy::ReadOnly, None).is_err());
}
